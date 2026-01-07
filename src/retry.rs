//! Retry logic with exponential backoff and jitter for HTTP requests.
//!
//! This module provides robust retry functionality for transient network failures
//! and rate limiting (HTTP 429) responses. It uses exponential backoff with
//! configurable jitter to prevent thundering herd problems.
//!
//! # Example
//!
//! ```ignore
//! use discrakt::retry::{execute_with_retry, RetryConfig};
//!
//! let config = RetryConfig::default();
//! let result: Result<MyResponse, RetryError> = execute_with_retry(
//!     || agent.get("https://api.example.com/data").call(),
//!     &config,
//! );
//! ```

use serde::de::DeserializeOwned;
use std::thread;
use std::time::Duration;
use thiserror::Error;

/// Configuration for retry behavior.
///
/// Uses exponential backoff with jitter to space out retry attempts.
/// The delay doubles with each attempt (up to `max_delay`), and random
/// jitter prevents synchronized retries from multiple clients.
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts before giving up.
    pub max_retries: u32,
    /// Initial delay between retries (doubles with each attempt).
    pub base_delay: Duration,
    /// Maximum delay cap to prevent excessively long waits.
    pub max_delay: Duration,
    /// Random jitter factor (0.0 to 1.0) to add/subtract from delay.
    /// A value of 0.3 means the delay can vary by +/-30%.
    pub jitter_factor: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(30),
            jitter_factor: 0.3,
        }
    }
}

/// Errors that can occur during retry execution.
#[derive(Error, Debug)]
pub enum RetryError {
    /// All retry attempts have been exhausted.
    #[error("max retries exceeded after {0} attempts")]
    MaxRetriesExceeded(u32),
    /// HTTP status code indicates a non-retryable error (e.g., 400, 401, 404).
    #[error("non-retryable HTTP status code: {0}")]
    NonRetryableError(u16),
    /// Network-level error (connection refused, timeout, DNS failure, etc.).
    #[error("network error: {0}")]
    NetworkError(String),
    /// Failed to parse the response body as JSON.
    #[error("failed to parse response: {0}")]
    ParseError(String),
}

/// Calculates the delay for a retry attempt with exponential backoff and jitter.
///
/// The base formula is: `base_delay * 2^attempt`, capped at `max_delay`.
/// Random jitter is then applied to prevent synchronized retries.
///
/// # Arguments
///
/// * `attempt` - The current attempt number (0-indexed).
/// * `config` - Retry configuration with delay parameters.
///
/// # Returns
///
/// The calculated delay duration including jitter.
///
/// # Note for Python developers
///
/// This function uses Rust's saturating arithmetic (`saturating_mul`) to prevent
/// integer overflow when calculating exponential backoff. In Python, integers
/// have arbitrary precision, but Rust's fixed-size integers can overflow.
pub fn calculate_delay_with_jitter(attempt: u32, config: &RetryConfig) -> Duration {
    // Calculate exponential backoff: base_delay * 2^attempt
    // Use saturating_pow to prevent overflow for large attempt values
    let multiplier = 2u64.saturating_pow(attempt);
    let base_millis = config.base_delay.as_millis() as u64;
    let exponential_millis = base_millis.saturating_mul(multiplier);

    // Cap at max_delay
    let max_millis = config.max_delay.as_millis() as u64;
    let capped_millis = exponential_millis.min(max_millis);

    // Apply jitter: random value in range [1 - jitter_factor, 1 + jitter_factor]
    // Using a simple deterministic approach based on attempt number for reproducibility
    // in tests, but with enough variation in practice due to timing.
    let jitter_range = config.jitter_factor * 2.0;
    let jitter_offset = pseudo_random_factor() * jitter_range - config.jitter_factor;
    let jitter_multiplier = 1.0 + jitter_offset;

    // Apply jitter and ensure non-negative result
    let jittered_millis = (capped_millis as f64 * jitter_multiplier).max(0.0) as u64;

    Duration::from_millis(jittered_millis)
}

/// Generates a pseudo-random factor between 0.0 and 1.0 based on timing.
///
/// This uses system time nanoseconds for randomness without requiring
/// an external random number generator dependency. While not cryptographically
/// secure, it provides sufficient variation for retry jitter purposes.
fn pseudo_random_factor() -> f64 {
    // Use system time nanoseconds for cheap pseudo-randomness
    // The nanosecond component varies enough between calls to provide jitter
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .subsec_nanos();

    // Normalize to 0.0 - 1.0 range
    (nanos as f64) / (1_000_000_000.0)
}

/// Parses the `Retry-After` HTTP header value as seconds.
///
/// The `Retry-After` header can contain either:
/// - A number of seconds to wait (this function handles this case)
/// - An HTTP-date (not handled by this function)
///
/// # Note
///
/// This function is currently not used in `execute_with_retry` because
/// `ureq::Error::StatusCode` doesn't provide access to response headers.
/// It's kept for potential future use when we might intercept responses
/// before they become errors, or if ureq's API changes.
///
/// # Arguments
///
/// * `value` - The raw header value string.
///
/// # Returns
///
/// The parsed duration, or `None` if the value cannot be parsed as seconds.
#[allow(dead_code)]
pub fn parse_retry_after_header(value: &str) -> Option<Duration> {
    value.trim().parse::<u64>().ok().map(Duration::from_secs)
}

/// Determines if an HTTP status code indicates a retryable error.
///
/// Retryable status codes include:
/// - 429 (Too Many Requests): Rate limiting, should retry after backoff
/// - 5xx (Server Errors): Temporary server issues that may resolve
///
/// Non-retryable codes include:
/// - 2xx (Success): Not an error
/// - 3xx (Redirects): Not typically retried
/// - 4xx (Client Errors): Except 429, these indicate request problems
///
/// # Arguments
///
/// * `status` - The HTTP status code.
///
/// # Returns
///
/// `true` if the request should be retried, `false` otherwise.
pub fn should_retry_status_code(status: u16) -> bool {
    // 429: Rate limited - definitely retry
    // 5xx: Server errors - may be transient
    status == 429 || (500..600).contains(&status)
}

/// Executes an HTTP request with automatic retry on transient failures.
///
/// This function wraps a request-producing closure and handles:
/// - Automatic retries for rate limiting (HTTP 429) and server errors (5xx)
/// - Exponential backoff with jitter between attempts
/// - JSON deserialization of successful responses
///
/// # Type Parameters
///
/// * `F` - A closure that produces the HTTP request. Called once per attempt.
/// * `T` - The expected response type, must implement `DeserializeOwned`.
///
/// # Arguments
///
/// * `request_fn` - Closure that executes the HTTP request.
/// * `config` - Retry configuration (delays, max attempts, jitter).
///
/// # Returns
///
/// - `Ok(T)` - Successfully parsed response body.
/// - `Err(RetryError)` - If all retries exhausted or non-retryable error occurred.
///
/// # Note for Python developers
///
/// The `where` clause specifies trait bounds (similar to Python's `Protocol` or
/// type constraints). The `for<'de>` syntax is a "higher-ranked trait bound"
/// (HRTB) meaning the type must be deserializable for any lifetime - this is
/// required because the JSON deserializer needs to work with borrowed data.
///
/// # Example
///
/// ```ignore
/// use discrakt::retry::{execute_with_retry, RetryConfig, RetryError};
///
/// let config = RetryConfig::default();
///
/// let result: Result<ApiResponse, RetryError> = execute_with_retry(
///     || agent.get("https://api.example.com/data").call(),
///     &config,
/// );
///
/// match result {
///     Ok(response) => println!("Got data: {:?}", response),
///     Err(RetryError::MaxRetriesExceeded(attempts)) => {
///         eprintln!("Failed after {} attempts", attempts);
///     }
///     Err(e) => eprintln!("Error: {}", e),
/// }
/// ```
pub fn execute_with_retry<F, T>(request_fn: F, config: &RetryConfig) -> Result<T, RetryError>
where
    F: Fn() -> Result<ureq::http::Response<ureq::Body>, ureq::Error>,
    T: DeserializeOwned,
{
    let mut attempt = 0;

    loop {
        match request_fn() {
            Ok(mut response) => {
                // Success - parse the JSON response
                return response
                    .body_mut()
                    .read_json::<T>()
                    .map_err(|e| RetryError::ParseError(e.to_string()));
            }
            Err(ureq::Error::StatusCode(status)) => {
                if !should_retry_status_code(status) {
                    return Err(RetryError::NonRetryableError(status));
                }

                attempt += 1;
                if attempt > config.max_retries {
                    return Err(RetryError::MaxRetriesExceeded(attempt));
                }

                // Calculate delay - would check Retry-After header if available
                // but ureq::Error::StatusCode doesn't give us access to headers
                let delay = calculate_delay_with_jitter(attempt - 1, config);

                tracing::warn!(
                    status = status,
                    attempt = attempt,
                    max_retries = config.max_retries,
                    delay_ms = delay.as_millis() as u64,
                    "Retryable HTTP error, backing off"
                );

                thread::sleep(delay);
            }
            Err(e) => {
                // Network errors (connection refused, timeout, DNS failure, etc.)
                attempt += 1;
                if attempt > config.max_retries {
                    return Err(RetryError::NetworkError(e.to_string()));
                }

                let delay = calculate_delay_with_jitter(attempt - 1, config);

                tracing::warn!(
                    error = %e,
                    attempt = attempt,
                    max_retries = config.max_retries,
                    delay_ms = delay.as_millis() as u64,
                    "Network error, retrying"
                );

                thread::sleep(delay);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retry_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.base_delay, Duration::from_secs(1));
        assert_eq!(config.max_delay, Duration::from_secs(30));
        assert!((config.jitter_factor - 0.3).abs() < f64::EPSILON);
    }

    #[test]
    fn test_should_retry_status_code() {
        // Retryable: 429 and 5xx
        assert!(should_retry_status_code(429));
        assert!(should_retry_status_code(500));
        assert!(should_retry_status_code(502));
        assert!(should_retry_status_code(503));
        assert!(should_retry_status_code(504));
        assert!(should_retry_status_code(599));

        // Not retryable: 2xx, 3xx, 4xx (except 429)
        assert!(!should_retry_status_code(200));
        assert!(!should_retry_status_code(201));
        assert!(!should_retry_status_code(301));
        assert!(!should_retry_status_code(302));
        assert!(!should_retry_status_code(400));
        assert!(!should_retry_status_code(401));
        assert!(!should_retry_status_code(403));
        assert!(!should_retry_status_code(404));
        assert!(!should_retry_status_code(422));
    }

    #[test]
    fn test_parse_retry_after_header_valid() {
        assert_eq!(
            parse_retry_after_header("120"),
            Some(Duration::from_secs(120))
        );
        assert_eq!(parse_retry_after_header("0"), Some(Duration::from_secs(0)));
        assert_eq!(
            parse_retry_after_header("  60  "),
            Some(Duration::from_secs(60))
        );
    }

    #[test]
    fn test_parse_retry_after_header_invalid() {
        assert_eq!(parse_retry_after_header("invalid"), None);
        assert_eq!(parse_retry_after_header(""), None);
        assert_eq!(parse_retry_after_header("-1"), None);
        // HTTP-date format is not supported
        assert_eq!(
            parse_retry_after_header("Wed, 21 Oct 2024 07:28:00 GMT"),
            None
        );
    }

    #[test]
    fn test_calculate_delay_exponential_backoff() {
        let config = RetryConfig {
            max_retries: 5,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            jitter_factor: 0.0, // No jitter for predictable testing
        };

        // Without jitter, delays should follow 2^attempt pattern
        let delay_0 = calculate_delay_with_jitter(0, &config);
        let delay_1 = calculate_delay_with_jitter(1, &config);
        let delay_2 = calculate_delay_with_jitter(2, &config);

        // With 0 jitter, should be exactly 100ms, 200ms, 400ms
        assert_eq!(delay_0.as_millis(), 100);
        assert_eq!(delay_1.as_millis(), 200);
        assert_eq!(delay_2.as_millis(), 400);
    }

    #[test]
    fn test_calculate_delay_caps_at_max() {
        let config = RetryConfig {
            max_retries: 10,
            base_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(5),
            jitter_factor: 0.0,
        };

        // Attempt 10 would be 1024 seconds without cap
        // Should be capped at 5 seconds
        let delay = calculate_delay_with_jitter(10, &config);
        assert_eq!(delay.as_secs(), 5);
    }

    #[test]
    fn test_calculate_delay_with_jitter_bounds() {
        let config = RetryConfig {
            max_retries: 3,
            base_delay: Duration::from_millis(1000),
            max_delay: Duration::from_secs(30),
            jitter_factor: 0.3,
        };

        // Run multiple times to verify jitter stays within bounds
        for _ in 0..10 {
            let delay = calculate_delay_with_jitter(0, &config);
            // Base is 1000ms, jitter +/-30% = 700ms to 1300ms
            assert!(delay.as_millis() >= 700);
            assert!(delay.as_millis() <= 1300);
        }
    }

    #[test]
    fn test_retry_error_display() {
        let err = RetryError::MaxRetriesExceeded(3);
        assert!(err.to_string().contains("3"));

        let err = RetryError::NonRetryableError(404);
        assert!(err.to_string().contains("404"));

        let err = RetryError::NetworkError("connection refused".to_string());
        assert!(err.to_string().contains("connection refused"));

        let err = RetryError::ParseError("invalid json".to_string());
        assert!(err.to_string().contains("invalid json"));
    }
}

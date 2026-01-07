//! Retry logic with exponential backoff and jitter for HTTP requests.
//!
//! This module provides robust retry functionality for transient network failures
//! and rate limiting (HTTP 429) responses using the `backon` crate for exponential
//! backoff with jitter.
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

use backon::{BlockingRetryable, ExponentialBuilder};
use serde::de::DeserializeOwned;
use std::time::Duration;
use thiserror::Error;

/// Configuration for retry behavior.
///
/// Uses exponential backoff with jitter to space out retry attempts.
/// The delay doubles with each attempt (up to `max_delay`), and random
/// jitter prevents synchronized retries from multiple clients.
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts before giving up (not counting the initial attempt).
    pub max_retries: u32,
    /// Initial delay between retries (doubles with each attempt).
    pub base_delay: Duration,
    /// Maximum delay cap to prevent excessively long waits.
    pub max_delay: Duration,
    /// Whether to add random jitter to backoff delays. When enabled, `backon`
    /// applies full jitter (0 to 100% of calculated delay) to prevent
    /// synchronized retries from multiple clients (thundering herd).
    pub enable_jitter: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(30),
            enable_jitter: true,
        }
    }
}

impl RetryConfig {
    /// Convert this config to a backon ExponentialBuilder.
    ///
    /// Note: `backon`'s `with_max_times(n)` sets the maximum number of retries
    /// (not including the initial attempt). So if `max_retries = 3`, backon will
    /// retry up to 3 times after the initial attempt, giving 4 total attempts.
    fn to_backoff_builder(&self) -> ExponentialBuilder {
        let mut builder = ExponentialBuilder::default()
            .with_min_delay(self.base_delay)
            .with_max_delay(self.max_delay)
            .with_max_times(self.max_retries as usize);

        if self.enable_jitter {
            builder = builder.with_jitter();
        }

        builder
    }
}

/// Errors that can occur during retry execution.
#[derive(Error, Debug)]
pub enum RetryError {
    /// All retry attempts have been exhausted.
    #[error("max retries exceeded after {attempts} attempts: {last_error}")]
    MaxRetriesExceeded {
        /// Total number of attempts made (initial + retries).
        attempts: u32,
        /// The last error message before giving up.
        last_error: String,
    },
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

/// Determines if an HTTP status code indicates a retryable error.
///
/// Retryable status codes include:
/// - 408 (Request Timeout): Server timeout, may succeed on retry
/// - 429 (Too Many Requests): Rate limiting, should retry after backoff
/// - 5xx (Server Errors): Temporary server issues that may resolve
///
/// Non-retryable codes include:
/// - 2xx (Success): Not an error
/// - 3xx (Redirects): Not typically retried
/// - 4xx (Client Errors): Except 408 and 429, these indicate request problems
///
/// # Arguments
///
/// * `status` - The HTTP status code.
///
/// # Returns
///
/// `true` if the request should be retried, `false` otherwise.
pub fn should_retry_status_code(status: u16) -> bool {
    // 408: Request timeout - server took too long, may succeed on retry
    // 429: Rate limited - definitely retry after backoff
    // 5xx: Server errors - may be transient
    status == 408 || status == 429 || (500..600).contains(&status)
}

/// Internal result type for the retry operation.
/// This wraps the actual result to distinguish between retryable and non-retryable errors.
enum RetryableResult<T> {
    Success(T),
    /// Non-retryable error - should not be retried
    NonRetryable(RetryError),
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
    let backoff = config.to_backoff_builder();
    let max_retries = config.max_retries;

    // Wrap the request function to return a Result that backon can retry on
    let retryable_fn = || -> Result<RetryableResult<T>, String> {
        match request_fn() {
            Ok(mut response) => {
                // Success - parse the JSON response
                match response.body_mut().read_json::<T>() {
                    Ok(parsed) => Ok(RetryableResult::Success(parsed)),
                    Err(e) => Ok(RetryableResult::NonRetryable(RetryError::ParseError(
                        e.to_string(),
                    ))),
                }
            }
            Err(ureq::Error::StatusCode(status)) => {
                if should_retry_status_code(status) {
                    tracing::warn!(status = status, "Retryable HTTP error, backing off");
                    // Return an Err so backon will retry
                    Err(format!("HTTP {}", status))
                } else {
                    // Non-retryable status code - return success with error wrapped
                    Ok(RetryableResult::NonRetryable(
                        RetryError::NonRetryableError(status),
                    ))
                }
            }
            Err(e) => {
                // Network errors (connection refused, timeout, DNS failure, etc.)
                tracing::warn!(
                    error = %e,
                    "Network error, retrying"
                );
                // Return an Err so backon will retry
                Err(e.to_string())
            }
        }
    };

    // Execute with retry using backon
    let result = retryable_fn.retry(backoff).sleep(std::thread::sleep).call();

    match result {
        Ok(RetryableResult::Success(value)) => Ok(value),
        Ok(RetryableResult::NonRetryable(err)) => Err(err),
        Err(last_error) => {
            // All retries exhausted - return MaxRetriesExceeded with context.
            // Total attempts = 1 initial attempt + max_retries retry attempts.
            let total_attempts = max_retries.saturating_add(1);
            Err(RetryError::MaxRetriesExceeded {
                attempts: total_attempts,
                last_error,
            })
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
        assert!(config.enable_jitter);
    }

    #[test]
    fn test_should_retry_status_code() {
        // Retryable: 408, 429 and 5xx
        assert!(should_retry_status_code(408)); // Request Timeout
        assert!(should_retry_status_code(429)); // Too Many Requests
        assert!(should_retry_status_code(500)); // Internal Server Error
        assert!(should_retry_status_code(502)); // Bad Gateway
        assert!(should_retry_status_code(503)); // Service Unavailable
        assert!(should_retry_status_code(504)); // Gateway Timeout
        assert!(should_retry_status_code(599)); // Network Connect Timeout

        // Not retryable: 2xx, 3xx, 4xx (except 408, 429)
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
    fn test_retry_error_display() {
        let err = RetryError::MaxRetriesExceeded {
            attempts: 3,
            last_error: "HTTP 503".to_string(),
        };
        assert!(err.to_string().contains("3"));
        assert!(err.to_string().contains("HTTP 503"));

        let err = RetryError::NonRetryableError(404);
        assert!(err.to_string().contains("404"));

        let err = RetryError::NetworkError("connection refused".to_string());
        assert!(err.to_string().contains("connection refused"));

        let err = RetryError::ParseError("invalid json".to_string());
        assert!(err.to_string().contains("invalid json"));
    }

    #[test]
    fn test_retry_config_to_backoff_builder_with_jitter() {
        let config = RetryConfig {
            max_retries: 5,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            enable_jitter: true,
        };

        // This test just verifies the builder is created without panic
        let _builder = config.to_backoff_builder();
    }

    #[test]
    fn test_retry_config_to_backoff_builder_without_jitter() {
        let config = RetryConfig {
            max_retries: 5,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            enable_jitter: false,
        };

        // This test just verifies the builder is created without panic
        let _builder = config.to_backoff_builder();
    }
}

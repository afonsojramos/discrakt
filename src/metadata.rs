use lru::LruCache;
use serde_json::Value;
use std::{num::NonZeroUsize, time::Duration};
use ureq::Agent;

use crate::retry::{execute_with_retry, RetryConfig, RetryError};
use crate::utils::{user_agent, MediaType};

/// Sanitizes a URL by removing sensitive query parameters (e.g., `api_key`).
/// This prevents credential leaks in log messages.
fn sanitize_url_for_logging(url: &str) -> String {
    // Find the query string start
    if let Some(query_start) = url.find('?') {
        let base = &url[..query_start];
        let query = &url[query_start + 1..];

        // Filter out sensitive parameters
        let sanitized_params: Vec<&str> = query
            .split('&')
            .filter(|param| {
                !param.starts_with("api_key=")
                    && !param.starts_with("access_token=")
                    && !param.starts_with("token=")
            })
            .collect();

        // Use consistent formatting - show non-sensitive params or just [REDACTED]
        let sanitized_query = if sanitized_params.is_empty() {
            "[REDACTED]".to_string()
        } else {
            sanitized_params.join("&")
        };

        format!("{}?{}", base, sanitized_query)
    } else {
        url.to_string()
    }
}

/// Maximum number of entries to store in each cache.
///
/// This prevents unbounded memory growth for long-running instances.
/// 500 entries is generous for typical usage (watching ~10 movies/shows per day
/// would take 50 days to fill the cache).
pub const MAX_CACHE_SIZE: usize = 500;

/// Default TMDB API base URL.
pub const DEFAULT_TMDB_BASE_URL: &str = "https://api.themoviedb.org";

/// Client for fetching artwork and localized titles from TMDB.
///
/// TMDB metadata is source-agnostic: any tracking source (Trakt, Plex, ...) that
/// can provide a TMDB id can reuse this enricher to resolve posters and localized
/// titles. Results are cached to minimize API calls.
pub struct Tmdb {
    /// LRU cache for poster image URLs, keyed by TMDB ID.
    image_cache: LruCache<String, String>,
    /// LRU cache for localized titles, keyed by language + TMDB ID.
    title_cache: LruCache<String, String>,
    agent: Agent,
    tmdb_base_url: String,
    language: String,
    /// Configuration for retry behavior on transient network failures.
    retry_config: RetryConfig,
}

impl Tmdb {
    /// Create a new TMDB client.
    ///
    /// `tmdb_base_url` defaults to <https://api.themoviedb.org> and `language`
    /// defaults to `en-US` when not provided.
    pub fn new(tmdb_base_url: Option<String>, language: Option<String>) -> Tmdb {
        let agent_config = Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(10)))
            .user_agent(user_agent())
            .build();

        // SAFETY: MAX_CACHE_SIZE is a non-zero constant
        let cache_size =
            NonZeroUsize::new(MAX_CACHE_SIZE).expect("MAX_CACHE_SIZE must be non-zero");

        Tmdb {
            image_cache: LruCache::new(cache_size),
            title_cache: LruCache::new(cache_size),
            agent: agent_config.into(),
            tmdb_base_url: tmdb_base_url.unwrap_or_else(|| DEFAULT_TMDB_BASE_URL.to_string()),
            language: language.unwrap_or_else(|| "en-US".to_string()),
            retry_config: RetryConfig::default(),
        }
    }

    /// Sets the retry configuration for API requests.
    ///
    /// This is primarily useful for testing, allowing tests to use
    /// shorter delays to speed up test execution.
    pub fn set_retry_config(&mut self, config: RetryConfig) {
        self.retry_config = config;
    }

    /// Sets the preferred language for TMDB title lookups.
    ///
    /// When the language changes, the title cache is cleared to ensure
    /// fresh translations are fetched on next request.
    pub fn set_language(&mut self, language: String) {
        if self.language != language {
            tracing::info!("Changing TMDB client language to: {}", language);
            self.language = language;
            self.title_cache.clear();
        }
    }

    /// Fetches the poster image URL from TMDB for the given media.
    ///
    /// Results are cached to minimize API calls. Uses retry logic with
    /// exponential backoff for transient network failures.
    ///
    /// # Returns
    ///
    /// The poster image URL, or `None` if unavailable or on error
    pub fn get_poster(
        &mut self,
        media_type: MediaType,
        tmdb_id: String,
        tmdb_token: &str,
        season_id: u16,
    ) -> Option<String> {
        // Posters are season-specific for shows, so the cache key must include
        // the season to avoid returning season 1's art for every later season.
        let cache_key = match media_type {
            MediaType::Movie => tmdb_id.clone(),
            MediaType::Show => format!("{tmdb_id}_S{season_id}"),
        };

        // Check cache first - this should happen BEFORE any retry logic
        if let Some(image_url) = self.image_cache.get(&cache_key) {
            return Some(image_url.clone());
        }

        let endpoint = match media_type {
            MediaType::Movie => format!(
                "{}/3/movie/{tmdb_id}/images?api_key={tmdb_token}",
                self.tmdb_base_url
            ),
            MediaType::Show => format!(
                "{}/3/tv/{tmdb_id}/season/{season_id}/images?api_key={tmdb_token}",
                self.tmdb_base_url
            ),
        };

        let agent = &self.agent;

        let result: Result<Value, RetryError> =
            execute_with_retry(|| agent.get(&endpoint).call(), &self.retry_config);

        match result {
            Ok(body) => {
                // Extract poster URL from TMDB response
                let posters = body["posters"].as_array();
                if posters.is_none_or(|p| p.is_empty()) {
                    tracing::warn!(
                        media_type = %media_type.as_str(),
                        "Image not found in TMDB response"
                    );
                    return None;
                }

                // Extract the file_path from the first poster
                let file_path = body["posters"][0].get("file_path").and_then(|v| v.as_str());

                match file_path {
                    Some(path) => {
                        let image_url =
                            format!("https://image.tmdb.org/t/p/w600_and_h600_bestv2{}", path);
                        // Cache the image URL (LRU will evict oldest if full)
                        self.image_cache.put(cache_key, image_url.clone());
                        Some(image_url)
                    }
                    None => {
                        tracing::warn!(
                            media_type = %media_type.as_str(),
                            "Poster missing file_path in TMDB response"
                        );
                        None
                    }
                }
            }
            Err(RetryError::NonRetryableError(401)) => {
                tracing::error!(
                    endpoint = %sanitize_url_for_logging(&endpoint),
                    "TMDB API key expired or invalid"
                );
                None
            }
            Err(RetryError::MaxRetriesExceeded {
                attempts,
                last_error,
            }) => {
                tracing::error!(
                    endpoint = %sanitize_url_for_logging(&endpoint),
                    media_type = %media_type.as_str(),
                    attempts = attempts,
                    last_error = %last_error,
                    "Failed to fetch poster after retries"
                );
                None
            }
            Err(RetryError::NetworkError(msg)) => {
                tracing::error!(
                    endpoint = %sanitize_url_for_logging(&endpoint),
                    media_type = %media_type.as_str(),
                    error = %msg,
                    "Network error fetching image"
                );
                None
            }
            Err(RetryError::ParseError(msg)) => {
                tracing::error!(
                    endpoint = %sanitize_url_for_logging(&endpoint),
                    media_type = %media_type.as_str(),
                    error = %msg,
                    "Failed to parse image response"
                );
                None
            }
            Err(RetryError::NonRetryableError(code)) => {
                tracing::error!(
                    endpoint = %sanitize_url_for_logging(&endpoint),
                    media_type = %media_type.as_str(),
                    status = code,
                    "Unexpected HTTP error from TMDB API"
                );
                None
            }
        }
    }

    /// Fetches a localized title from TMDB for the given media.
    ///
    /// Results are cached to minimize API calls. Returns an empty string
    /// if no translation is available (which is also cached to avoid
    /// repeated lookups for untranslated content). Uses retry logic with
    /// exponential backoff for transient network failures.
    ///
    /// # Returns
    ///
    /// The localized title, or empty string if unavailable or on error
    pub fn get_title(
        &mut self,
        media_type: MediaType,
        tmdb_id: String,
        tmdb_token: &str,
        season: Option<u16>,
        episode: Option<u16>,
    ) -> String {
        // Include language in cache key to ensure correct translations when language changes
        let cache_key = if let (Some(s), Some(e)) = (season, episode) {
            format!("{}_{tmdb_id}_S{s}E{e}", self.language)
        } else {
            format!("{}_{tmdb_id}", self.language)
        };

        // Check cache first - this should happen BEFORE any retry logic
        if let Some(title) = self.title_cache.get(&cache_key) {
            return title.clone();
        }

        let endpoint = match media_type {
            MediaType::Movie => format!(
                "{}/3/movie/{}?api_key={}&language={}",
                self.tmdb_base_url, tmdb_id, tmdb_token, self.language
            ),
            MediaType::Show => {
                if let (Some(s), Some(e)) = (season, episode) {
                    format!(
                        "{}/3/tv/{}/season/{}/episode/{}?api_key={}&language={}",
                        self.tmdb_base_url, tmdb_id, s, e, tmdb_token, self.language
                    )
                } else {
                    format!(
                        "{}/3/tv/{}?api_key={}&language={}",
                        self.tmdb_base_url, tmdb_id, tmdb_token, self.language
                    )
                }
            }
        };

        let agent = &self.agent;

        // Use serde_json::Value for flexible JSON parsing since we need to
        // extract different keys ("title" vs "name") based on media type
        let result: Result<Value, RetryError> =
            execute_with_retry(|| agent.get(&endpoint).call(), &self.retry_config);

        let title = match result {
            Ok(json) => {
                // Movies use "title", TV shows/episodes use "name"
                let key = if matches!(media_type, MediaType::Movie) {
                    "title"
                } else {
                    "name"
                };
                json[key].as_str().unwrap_or("").to_string()
            }
            Err(RetryError::NonRetryableError(401)) => {
                tracing::debug!(
                    endpoint = %sanitize_url_for_logging(&endpoint),
                    media_type = %media_type.as_str(),
                    tmdb_id = %tmdb_id,
                    "TMDB API key expired or invalid"
                );
                String::new()
            }
            Err(RetryError::MaxRetriesExceeded {
                attempts,
                last_error,
            }) => {
                tracing::debug!(
                    endpoint = %sanitize_url_for_logging(&endpoint),
                    media_type = %media_type.as_str(),
                    tmdb_id = %tmdb_id,
                    attempts = attempts,
                    last_error = %last_error,
                    "Failed to fetch localized title after retries"
                );
                String::new()
            }
            Err(RetryError::NetworkError(msg)) => {
                tracing::debug!(
                    endpoint = %sanitize_url_for_logging(&endpoint),
                    error = %msg,
                    media_type = %media_type.as_str(),
                    tmdb_id = %tmdb_id,
                    "Failed to fetch localized title from TMDB"
                );
                String::new()
            }
            Err(RetryError::ParseError(msg)) => {
                tracing::debug!(
                    endpoint = %sanitize_url_for_logging(&endpoint),
                    error = %msg,
                    media_type = %media_type.as_str(),
                    tmdb_id = %tmdb_id,
                    "Failed to parse TMDB title response"
                );
                String::new()
            }
            Err(RetryError::NonRetryableError(code)) => {
                tracing::debug!(
                    endpoint = %sanitize_url_for_logging(&endpoint),
                    media_type = %media_type.as_str(),
                    tmdb_id = %tmdb_id,
                    status = code,
                    "Unexpected HTTP error fetching localized title"
                );
                String::new()
            }
        };

        // Cache both successful and empty results (LRU will evict oldest if full)
        self.title_cache.put(cache_key, title.clone());
        title
    }
}

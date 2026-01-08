use lru::LruCache;
use serde::Deserialize;
use serde_json::Value;
use std::{collections::HashMap, num::NonZeroUsize, time::Duration};
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

/// Default Trakt API base URL.
pub const DEFAULT_TRAKT_BASE_URL: &str = "https://api.trakt.tv";

/// Default TMDB API base URL.
pub const DEFAULT_TMDB_BASE_URL: &str = "https://api.themoviedb.org";

/// Configuration for creating a Trakt client.
#[derive(Clone, Default)]
pub struct TraktConfig {
    pub client_id: String,
    pub username: String,
    pub oauth_access_token: Option<String>,
    /// Base URL for Trakt API (defaults to https://api.trakt.tv)
    pub trakt_base_url: Option<String>,
    /// Base URL for TMDB API (defaults to https://api.themoviedb.org)
    pub tmdb_base_url: Option<String>,
    pub language: Option<String>,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct TraktMovie {
    pub title: String,
    pub year: u16,
    pub ids: TraktIds,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct TraktShow {
    pub title: String,
    pub year: u16,
    pub ids: TraktIds,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct TraktEpisode {
    pub season: u8,
    pub number: u8,
    pub title: String,
    pub ids: TraktIds,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct TraktIds {
    pub trakt: u32,
    pub slug: Option<String>,
    pub tvdb: Option<u32>,
    pub imdb: Option<String>,
    pub tmdb: Option<u32>,
    pub tvrage: Option<u32>,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct TraktWatchingResponse {
    pub expires_at: String,
    pub started_at: String,
    pub action: String,
    pub r#type: String,
    pub movie: Option<TraktMovie>,
    pub show: Option<TraktShow>,
    pub episode: Option<TraktEpisode>,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct TraktRatingsResponse {
    pub rating: f64,
    pub votes: u32,
    pub distribution: HashMap<String, u16>,
}

pub struct Trakt {
    /// LRU cache for movie ratings, keyed by movie slug.
    rating_cache: LruCache<String, f64>,
    /// LRU cache for poster image URLs, keyed by TMDB ID.
    image_cache: LruCache<String, String>,
    /// LRU cache for localized titles, keyed by language + TMDB ID.
    title_cache: LruCache<String, String>,
    agent: Agent,
    client_id: String,
    username: String,
    oauth_access_token: Option<String>,
    trakt_base_url: String,
    tmdb_base_url: String,
    language: String,
    /// Configuration for retry behavior on transient network failures.
    retry_config: RetryConfig,
}

impl Trakt {
    /// Create a new Trakt client with default API URLs.
    pub fn new(client_id: String, username: String, oauth_access_token: Option<String>) -> Trakt {
        Self::with_config(TraktConfig {
            client_id,
            username,
            oauth_access_token,
            ..Default::default()
        })
    }

    /// Create a new Trakt client with custom configuration.
    ///
    /// This constructor allows overriding the API base URLs, which is useful for testing.
    pub fn with_config(config: TraktConfig) -> Trakt {
        let agent_config = Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(10)))
            .user_agent(user_agent())
            .build();

        // SAFETY: MAX_CACHE_SIZE is a non-zero constant
        let cache_size =
            NonZeroUsize::new(MAX_CACHE_SIZE).expect("MAX_CACHE_SIZE must be non-zero");

        Trakt {
            rating_cache: LruCache::new(cache_size),
            image_cache: LruCache::new(cache_size),
            title_cache: LruCache::new(cache_size),
            agent: agent_config.into(),
            client_id: config.client_id,
            username: config.username,
            oauth_access_token: config.oauth_access_token,
            trakt_base_url: config
                .trakt_base_url
                .unwrap_or_else(|| DEFAULT_TRAKT_BASE_URL.to_string()),
            tmdb_base_url: config
                .tmdb_base_url
                .unwrap_or_else(|| DEFAULT_TMDB_BASE_URL.to_string()),
            language: config.language.unwrap_or_else(|| "en-US".to_string()),
            retry_config: RetryConfig::default(),
        }
    }

    fn handle_auth_error(&self, status_code: u16, endpoint: &str) {
        match status_code {
            401 => {
                if self.oauth_access_token.is_some() {
                    tracing::error!(
                        endpoint = %endpoint,
                        "OAuth token expired or invalid"
                    );
                    tracing::warn!(
                        "Please refresh your OAuth token to continue using authenticated endpoints"
                    );
                } else {
                    tracing::error!(
                        endpoint = %endpoint,
                        "Authentication required"
                    );
                }
            }
            403 => {
                tracing::error!(
                    endpoint = %endpoint,
                    "Access forbidden - check token permissions"
                );
            }
            _ => {}
        }
    }

    /// Fetches the current watching status from Trakt.tv.
    ///
    /// This method uses retry logic with exponential backoff for transient
    /// network failures and rate limiting. Auth errors (401, 403) are not
    /// retried and are logged appropriately.
    ///
    /// # Returns
    ///
    /// - `Some(TraktWatchingResponse)` if the user is currently watching something
    /// - `None` if not watching anything, or if an error occurred
    pub fn get_watching(&self) -> Option<TraktWatchingResponse> {
        let endpoint = format!("{}/users/{}/watching", self.trakt_base_url, self.username);

        // Build authorization header outside closure to avoid lifetime issues.
        // In Rust, closures capture variables by reference by default, but the
        // `execute_with_retry` function requires `Fn()` which means the closure
        // may be called multiple times. We need owned data or references that
        // outlive the closure.
        let authorization = self.oauth_access_token.as_ref().and_then(|token| {
            if token.is_empty() {
                None
            } else {
                Some(format!("Bearer {}", token))
            }
        });

        // Clone values needed inside the closure to avoid borrowing `self`
        // multiple times. The closure captures these by value (via `move` semantics
        // inferred from the Clone).
        let agent = &self.agent;
        let client_id = &self.client_id;

        let result: Result<Option<TraktWatchingResponse>, RetryError> = execute_with_retry(
            || {
                let mut request = agent
                    .get(&endpoint)
                    .header("Content-Type", "application/json")
                    .header("trakt-api-version", "2")
                    .header("trakt-api-key", client_id);

                if let Some(auth) = &authorization {
                    request = request.header("Authorization", auth);
                }

                request.call()
            },
            &self.retry_config,
        );

        match result {
            // The API returns `null` (parsed as `None`) when not watching anything
            Ok(response) => response,
            Err(RetryError::NonRetryableError(code @ (401 | 403))) => {
                self.handle_auth_error(code, &endpoint);
                None
            }
            Err(RetryError::NonRetryableError(204)) => {
                // HTTP 204 No Content - user is not watching anything.
                // Trakt returns 204 (not 200 with empty body) when nothing is playing.
                // This is expected API behavior, not an error condition.
                None
            }
            Err(RetryError::MaxRetriesExceeded {
                attempts,
                last_error,
            }) => {
                tracing::error!(
                    endpoint = %endpoint,
                    attempts = attempts,
                    last_error = %last_error,
                    "Failed to fetch watching status after retries"
                );
                None
            }
            Err(RetryError::NetworkError(msg)) => {
                tracing::error!(
                    endpoint = %endpoint,
                    error = %msg,
                    "Network error calling Trakt API"
                );
                None
            }
            Err(RetryError::ParseError(msg)) => {
                tracing::debug!(
                    endpoint = %endpoint,
                    error = %msg,
                    "Failed to parse watching response (may be empty)"
                );
                None
            }
            Err(RetryError::NonRetryableError(code)) => {
                tracing::error!(
                    endpoint = %endpoint,
                    status = code,
                    "Unexpected HTTP error from Trakt API"
                );
                None
            }
        }
    }

    /// Fetches the poster image URL from TMDB for the given media.
    ///
    /// Results are cached to minimize API calls. Uses retry logic with
    /// exponential backoff for transient network failures.
    ///
    /// # Arguments
    ///
    /// * `media_type` - Whether this is a Movie or Show
    /// * `tmdb_id` - The TMDB ID for the media
    /// * `tmdb_token` - API token for TMDB requests
    /// * `season_id` - Season number (used for TV shows)
    ///
    /// # Returns
    ///
    /// The poster image URL, or `None` if unavailable or on error
    pub fn get_poster(
        &mut self,
        media_type: MediaType,
        tmdb_id: String,
        tmdb_token: String,
        season_id: u8,
    ) -> Option<String> {
        // Check cache first - this should happen BEFORE any retry logic
        if let Some(image_url) = self.image_cache.get(&tmdb_id) {
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
                        self.image_cache.put(tmdb_id, image_url.clone());
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

    /// Fetches the rating for a movie from Trakt.tv.
    ///
    /// Results are cached to minimize API calls. Uses retry logic with
    /// exponential backoff for transient network failures.
    ///
    /// # Arguments
    ///
    /// * `movie_slug` - The Trakt slug identifier for the movie
    ///
    /// # Returns
    ///
    /// The movie rating (0.0 to 10.0), or 0.0 if unavailable or on error
    pub fn get_movie_rating(&mut self, movie_slug: String) -> f64 {
        // Check cache first - this should happen BEFORE any retry logic
        if let Some(rating) = self.rating_cache.get(&movie_slug) {
            return *rating;
        }

        let endpoint = format!("{}/movies/{movie_slug}/ratings", self.trakt_base_url);

        let agent = &self.agent;
        let client_id = &self.client_id;

        let result: Result<TraktRatingsResponse, RetryError> = execute_with_retry(
            || {
                agent
                    .get(&endpoint)
                    .header("Content-Type", "application/json")
                    .header("trakt-api-version", "2")
                    .header("trakt-api-key", client_id)
                    .call()
            },
            &self.retry_config,
        );

        match result {
            Ok(body) => {
                // Cache the rating (LRU will evict oldest if full)
                self.rating_cache.put(movie_slug, body.rating);
                body.rating
            }
            Err(RetryError::NonRetryableError(code @ (401 | 403))) => {
                self.handle_auth_error(code, &endpoint);
                0.0
            }
            Err(RetryError::MaxRetriesExceeded {
                attempts,
                last_error,
            }) => {
                tracing::error!(
                    endpoint = %endpoint,
                    attempts = attempts,
                    last_error = %last_error,
                    "Failed to fetch movie rating after retries"
                );
                0.0
            }
            Err(RetryError::NetworkError(msg)) => {
                tracing::error!(error = %msg, "Network error fetching movie rating");
                0.0
            }
            Err(RetryError::ParseError(msg)) => {
                tracing::error!(error = %msg, "Failed to parse movie rating response");
                0.0
            }
            Err(RetryError::NonRetryableError(code)) => {
                tracing::error!(
                    endpoint = %endpoint,
                    status = code,
                    "Unexpected HTTP error fetching movie rating"
                );
                0.0
            }
        }
    }

    /// Sets the retry configuration for API requests.
    ///
    /// This is primarily useful for testing, allowing tests to use
    /// shorter delays to speed up test execution.
    ///
    /// # Arguments
    ///
    /// * `config` - The retry configuration to use
    ///
    /// # Example
    ///
    /// ```ignore
    /// use std::time::Duration;
    /// use discrakt::retry::RetryConfig;
    ///
    /// let mut trakt = Trakt::new("client_id".to_string(), "user".to_string(), None);
    /// trakt.set_retry_config(RetryConfig {
    ///     max_retries: 2,
    ///     base_delay: Duration::from_millis(10),
    ///     max_delay: Duration::from_millis(100),
    ///     enable_jitter: false,
    /// });
    /// ```
    pub fn set_retry_config(&mut self, config: RetryConfig) {
        self.retry_config = config;
    }

    /// Sets the preferred language for TMDB title lookups.
    ///
    /// When the language changes, the title cache is cleared to ensure
    /// fresh translations are fetched on next request.
    ///
    /// # Arguments
    /// * `language` - TMDB language code (e.g., "en-US", "fr-FR")
    pub fn set_language(&mut self, language: String) {
        if self.language != language {
            tracing::info!("Changing Trakt client language to: {}", language);
            self.language = language;
            self.title_cache.clear();
        }
    }

    /// Fetches a localized title from TMDB for the given media.
    ///
    /// Results are cached to minimize API calls. Returns an empty string
    /// if no translation is available (which is also cached to avoid
    /// repeated lookups for untranslated content). Uses retry logic with
    /// exponential backoff for transient network failures.
    ///
    /// # Arguments
    ///
    /// * `media_type` - Whether this is a Movie or Show
    /// * `tmdb_id` - The TMDB ID for the media
    /// * `tmdb_token` - API token for TMDB requests
    /// * `season` - Season number (required for episode lookups)
    /// * `episode` - Episode number (required for episode lookups)
    ///
    /// # Returns
    ///
    /// The localized title, or empty string if unavailable or on error
    pub fn get_title(
        &mut self,
        media_type: MediaType,
        tmdb_id: String,
        tmdb_token: &str,
        season: Option<u8>,
        episode: Option<u8>,
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

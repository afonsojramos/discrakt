use lru::LruCache;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use serde::Deserialize;
use std::{collections::HashMap, num::NonZeroUsize, time::Duration};
use ureq::Agent;

use crate::metadata::Tmdb;
use crate::retry::{execute_with_retry, RetryConfig, RetryError};
use crate::utils::{user_agent, MediaType};

// Re-exported for backwards compatibility: these used to live in this module
// before TMDB enrichment was extracted into [`crate::metadata`].
pub use crate::metadata::{DEFAULT_TMDB_BASE_URL, MAX_CACHE_SIZE};

/// Default Trakt API base URL.
pub const DEFAULT_TRAKT_BASE_URL: &str = "https://api.trakt.tv";

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
    pub runtime: Option<u16>,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct TraktShow {
    pub title: String,
    pub year: u16,
    pub ids: TraktIds,
    pub runtime: Option<u16>,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct TraktEpisode {
    pub season: u8,
    pub number: u8,
    pub title: String,
    pub ids: TraktIds,
    pub runtime: Option<u16>,
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
    /// TMDB enricher for posters and localized titles (shared, source-agnostic).
    tmdb: Tmdb,
    agent: Agent,
    client_id: String,
    username: String,
    oauth_access_token: Option<String>,
    trakt_base_url: String,
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
            tmdb: Tmdb::new(config.tmdb_base_url, config.language),
            agent: agent_config.into(),
            client_id: config.client_id,
            username: config.username,
            oauth_access_token: config.oauth_access_token,
            trakt_base_url: config
                .trakt_base_url
                .unwrap_or_else(|| DEFAULT_TRAKT_BASE_URL.to_string()),
            retry_config: RetryConfig::default(),
        }
    }

    /// Returns a mutable reference to the TMDB enricher.
    pub fn tmdb_mut(&mut self) -> &mut Tmdb {
        &mut self.tmdb
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
        let has_oauth = self
            .oauth_access_token
            .as_ref()
            .is_some_and(|t| !t.is_empty());

        let endpoint = if has_oauth {
            format!("{}/users/me/watching?extended=full", self.trakt_base_url)
        } else {
            let encoded = utf8_percent_encode(&self.username, NON_ALPHANUMERIC).to_string();
            format!(
                "{}/users/{}/watching?extended=full",
                self.trakt_base_url, encoded
            )
        };

        let authorization = self.oauth_access_token.as_ref().and_then(|token| {
            if token.is_empty() {
                None
            } else {
                Some(format!("Bearer {}", token))
            }
        });

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
            Ok(response) => response,
            Err(RetryError::NonRetryableError(code @ (401 | 403))) => {
                self.handle_auth_error(code, &endpoint);
                None
            }
            Err(RetryError::NonRetryableError(204)) => None,
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
    /// Delegates to the internal [`Tmdb`] enricher. Retained for backwards
    /// compatibility; new callers can use [`Trakt::tmdb_mut`].
    pub fn get_poster(
        &mut self,
        media_type: MediaType,
        tmdb_id: String,
        tmdb_token: String,
        season_id: u8,
    ) -> Option<String> {
        self.tmdb
            .get_poster(media_type, tmdb_id, &tmdb_token, season_id)
    }

    /// Fetches the rating for a movie from Trakt.tv.
    ///
    /// Results are cached to minimize API calls. Uses retry logic with
    /// exponential backoff for transient network failures.
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
    pub fn set_retry_config(&mut self, config: RetryConfig) {
        self.retry_config = config.clone();
        self.tmdb.set_retry_config(config);
    }

    /// Sets the preferred language for TMDB title lookups.
    ///
    /// Delegates to the internal [`Tmdb`] enricher, which clears its title
    /// cache when the language changes.
    pub fn set_language(&mut self, language: String) {
        self.tmdb.set_language(language);
    }

    /// Fetches a localized title from TMDB for the given media.
    ///
    /// Delegates to the internal [`Tmdb`] enricher. Retained for backwards
    /// compatibility; new callers can use [`Trakt::tmdb_mut`].
    pub fn get_title(
        &mut self,
        media_type: MediaType,
        tmdb_id: String,
        tmdb_token: &str,
        season: Option<u8>,
        episode: Option<u8>,
    ) -> String {
        self.tmdb
            .get_title(media_type, tmdb_id, tmdb_token, season, episode)
    }
}

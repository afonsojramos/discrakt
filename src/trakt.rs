use serde::Deserialize;
use serde_json::Value;
use std::{collections::HashMap, time::Duration};
use ureq::Agent;

use crate::utils::{user_agent, MediaType};

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
    rating_cache: HashMap<String, f64>,
    image_cache: HashMap<String, String>,
    agent: Agent,
    client_id: String,
    username: String,
    oauth_access_token: Option<String>,
    trakt_base_url: String,
    tmdb_base_url: String,
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

        Trakt {
            rating_cache: HashMap::default(),
            image_cache: HashMap::default(),
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

    pub fn get_watching(&self) -> Option<TraktWatchingResponse> {
        let endpoint = format!("{}/users/{}/watching", self.trakt_base_url, self.username);

        let mut request = self
            .agent
            .get(&endpoint)
            .header("Content-Type", "application/json")
            .header("trakt-api-version", "2")
            .header("trakt-api-key", &self.client_id);

        // add Authorization header if there is a (valid) OAuth access token
        if self.oauth_access_token.is_some()
            && !self.oauth_access_token.as_ref().unwrap().is_empty()
        {
            let authorization = format!("Bearer {}", self.oauth_access_token.as_ref().unwrap());
            request = request.header("Authorization", &authorization);
        }

        let mut response = match request.call() {
            Ok(response) => response,
            Err(ureq::Error::StatusCode(code)) => {
                self.handle_auth_error(code, &endpoint);
                return None;
            }
            Err(e) => {
                tracing::error!(endpoint = %endpoint, error = %e, "Network error calling Trakt API");
                return None;
            }
        };

        response.body_mut().read_json().unwrap_or_default()
    }

    pub fn get_poster(
        &mut self,
        media_type: MediaType,
        tmdb_id: String,
        tmdb_token: String,
        season_id: u8,
    ) -> Option<String> {
        match self.image_cache.get(&tmdb_id) {
            Some(image_url) => Some(image_url.to_string()),
            None => {
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

                let mut response = match self.agent.get(&endpoint).call() {
                    Ok(response) => response,
                    Err(ureq::Error::StatusCode(401)) => {
                        tracing::error!(
                            endpoint = %endpoint,
                            "TMDB API key expired or invalid"
                        );
                        return None;
                    }
                    Err(e) => {
                        tracing::error!(
                            media_type = %media_type.as_str(),
                            error = %e,
                            "Error fetching image"
                        );
                        return None;
                    }
                };

                match response.body_mut().read_json::<Value>() {
                    Ok(body) => {
                        if body["posters"].as_array().unwrap_or(&vec![]).is_empty() {
                            tracing::warn!(
                                media_type = %media_type.as_str(),
                                "Image not found in TMDB response"
                            );
                            return None;
                        }

                        let image_url = format!(
                            "https://image.tmdb.org/t/p/w600_and_h600_bestv2{}",
                            body["posters"][0]
                                .clone()
                                .get("file_path")
                                .unwrap()
                                .as_str()
                                .unwrap()
                        );

                        // Cache the image URL
                        self.image_cache.insert(tmdb_id, image_url.clone());
                        Some(image_url)
                    }
                    Err(e) => {
                        tracing::error!(
                            media_type = %media_type.as_str(),
                            error = %e,
                            "Failed to parse image response"
                        );
                        None
                    }
                }
            }
        }
    }

    pub fn get_movie_rating(&mut self, movie_slug: String) -> f64 {
        match self.rating_cache.get(&movie_slug) {
            Some(rating) => *rating,
            None => {
                let endpoint = format!("{}/movies/{movie_slug}/ratings", self.trakt_base_url);

                let mut response = match self
                    .agent
                    .get(&endpoint)
                    .header("Content-Type", "application/json")
                    .header("trakt-api-version", "2")
                    .header("trakt-api-key", &self.client_id)
                    .call()
                {
                    Ok(response) => response,
                    Err(ureq::Error::StatusCode(code)) => {
                        self.handle_auth_error(code, &endpoint);
                        return 0.0;
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "Network error fetching movie rating");
                        return 0.0;
                    }
                };

                match response.body_mut().read_json::<TraktRatingsResponse>() {
                    Ok(body) => {
                        self.rating_cache
                            .insert(movie_slug.to_string(), body.rating);
                        body.rating
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "Failed to parse movie rating response");
                        0.0
                    }
                }
            }
        }
    }
}

use serde::Deserialize;
use std::{collections::HashMap, time::Duration};
use ureq::{serde_json, Agent, AgentBuilder};

use crate::utils::{log, user_agent, MediaType};

#[derive(Deserialize)]
pub struct TraktMovie {
    pub title: String,
    pub year: u16,
    pub ids: TraktIds,
}

#[derive(Deserialize)]
pub struct TraktShow {
    pub title: String,
    pub year: u16,
    pub ids: TraktIds,
}

#[derive(Deserialize)]
pub struct TraktEpisode {
    pub season: u8,
    pub number: u8,
    pub title: String,
    pub ids: TraktIds,
}

#[derive(Deserialize)]
pub struct TraktIds {
    pub trakt: u32,
    pub slug: Option<String>,
    pub tvdb: Option<u32>,
    pub imdb: Option<String>,
    pub tmdb: Option<u32>,
    pub tvrage: Option<u32>,
}

#[derive(Deserialize)]
pub struct TraktWatchingResponse {
    pub expires_at: String,
    pub started_at: String,
    pub action: String,
    pub r#type: String,
    pub movie: Option<TraktMovie>,
    pub show: Option<TraktShow>,
    pub episode: Option<TraktEpisode>,
}

#[derive(Deserialize)]
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
}

impl Trakt {
    pub fn new(client_id: String, username: String, oauth_access_token: Option<String>) -> Trakt {
        Trakt {
            rating_cache: HashMap::default(),
            image_cache: HashMap::default(),
            agent: AgentBuilder::new()
                .timeout_read(Duration::from_secs(5))
                .timeout_write(Duration::from_secs(5))
                .user_agent(&user_agent())
                .build(),
            client_id,
            username,
            oauth_access_token,
        }
    }

    fn handle_auth_error(&self, status_code: u16, endpoint: &str) {
        match status_code {
            401 => {
                if self.oauth_access_token.is_some() {
                    log(&format!(
                        "OAuth token expired or invalid for endpoint: {}",
                        endpoint
                    ));
                    log(
                        "Please refresh your OAuth token to continue using authenticated endpoints",
                    );
                } else {
                    log(&format!(
                        "Authentication required for endpoint: {}",
                        endpoint
                    ));
                }
            }
            403 => {
                log(&format!(
                    "Access forbidden for endpoint: {} - check token permissions",
                    endpoint
                ));
            }
            _ => {}
        }
    }

    pub fn get_watching(&self) -> Option<TraktWatchingResponse> {
        let endpoint = format!("https://api.trakt.tv/users/{}/watching", self.username);

        let request = self
            .agent
            .get(&endpoint)
            .set("Content-Type", "application/json")
            .set("trakt-api-version", "2")
            .set("trakt-api-key", &self.client_id)
            .set("User-Agent", &user_agent());

        // add Authorization header if there is a (valid) OAuth access token
        let request = if self.oauth_access_token.is_some()
            && !self.oauth_access_token.as_ref().unwrap().is_empty()
        {
            let authorization = format!("Bearer {}", self.oauth_access_token.as_ref().unwrap());
            request.set("Authorization", &authorization)
        } else {
            request
        };

        let response = match request.call() {
            Ok(response) => response,
            Err(ureq::Error::Status(code, _)) => {
                self.handle_auth_error(code, &endpoint);
                return None;
            }
            Err(e) => {
                log(&format!("Network error calling {}: {}", endpoint, e));
                return None;
            }
        };

        response.into_json().unwrap_or_default()
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
                    MediaType::Movie => format!("https://api.themoviedb.org/3/movie/{tmdb_id}/images?api_key={tmdb_token}"),
                    MediaType::Show => format!("https://api.themoviedb.org/3/tv/{tmdb_id}/season/{season_id}/images?api_key={tmdb_token}")
                };

                let response = match self.agent.get(&endpoint).call() {
                    Ok(response) => response,
                    Err(ureq::Error::Status(401, _)) => {
                        log(&format!(
                            "TMDB API key expired or invalid for endpoint: {}",
                            endpoint
                        ));
                        return None;
                    }
                    Err(e) => {
                        log(&format!(
                            "Error fetching {} image: {}",
                            media_type.as_str(),
                            e
                        ));
                        return None;
                    }
                };

                match response.into_json::<serde_json::Value>() {
                    Ok(body) => {
                        if body["posters"].as_array().unwrap_or(&vec![]).is_empty() {
                            log(&format!(
                                "{} image not found in TMDB response",
                                media_type.as_str()
                            ));
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
                        log(&format!(
                            "Failed to parse {} image response: {}",
                            media_type.as_str(),
                            e
                        ));
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
                let endpoint = format!("https://api.trakt.tv/movies/{movie_slug}/ratings");

                let response = match self
                    .agent
                    .get(&endpoint)
                    .set("Content-Type", "application/json")
                    .set("trakt-api-version", "2")
                    .set("trakt-api-key", &self.client_id)
                    .call()
                {
                    Ok(response) => response,
                    Err(ureq::Error::Status(code, _)) => {
                        self.handle_auth_error(code, &endpoint);
                        return 0.0;
                    }
                    Err(e) => {
                        log(&format!("Network error fetching movie rating: {}", e));
                        return 0.0;
                    }
                };

                match response.into_json::<TraktRatingsResponse>() {
                    Ok(body) => {
                        self.rating_cache
                            .insert(movie_slug.to_string(), body.rating);
                        body.rating
                    }
                    Err(e) => {
                        log(&format!("Failed to parse movie rating response: {}", e));
                        0.0
                    }
                }
            }
        }
    }
}

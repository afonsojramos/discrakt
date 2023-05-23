use serde::Deserialize;
use std::{collections::HashMap, time::Duration};
use ureq::{serde_json, Agent, AgentBuilder};

use crate::utils::log;

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
                .build(),
            client_id,
            username,
            oauth_access_token,
        }
    }

    pub fn get_watching(&self) -> Option<TraktWatchingResponse> {
        let endpoint = format!("https://api.trakt.tv/users/{}/watching", self.username);

        let request = self
            .agent
            .get(&endpoint)
            .set("Content-Type", "application/json")
            .set("trakt-api-version", "2")
            .set("trakt-api-key", &self.client_id);
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
            Err(_) => return None,
        };

        match response.into_json() {
            Ok(body) => body,
            Err(_) => None,
        }
    }

    pub fn get_show_image(
        &mut self,
        tmdb_id: String,
        tmdb_token: String,
        season_id: u8,
    ) -> Option<String> {
        match self.image_cache.get(&tmdb_id) {
            Some(image_url) => Some(image_url.to_string()),
            None => {
                let endpoint = format!("https://api.themoviedb.org/3/tv/{tmdb_id}/season/{season_id}/images?api_key={tmdb_token}");

                let response = match self.agent.get(&endpoint).call() {
                    Ok(response) => response,
                    Err(_) => {
                        log("Failed to get image from tmdb-api");
                        return None;
                    }
                };

                match response.into_json::<serde_json::Value>() {
                    Ok(body) => {
                        let image_url = format!(
                            "https://image.tmdb.org/t/p/w600_and_h600_bestv2{}",
                            body["posters"][0]
                                .clone()
                                .get("file_path")
                                .unwrap()
                                .as_str()
                                .unwrap()
                        );
                        Some(image_url)
                    }
                    Err(_) => {
                        log("Show image not correctly found");
                        None
                    }
                }
            }
        }
    }

    pub fn get_movie_image(&mut self, tmdb_id: String, tmdb_token: String) -> Option<String> {
        match self.image_cache.get(&tmdb_id) {
            Some(image_url) => Some(image_url.to_string()),
            None => {
                let endpoint = format!(
                    "https://api.themoviedb.org/3/movie/{tmdb_id}/images?api_key={tmdb_token}"
                );

                let response = match self.agent.get(&endpoint).call() {
                    Ok(response) => response,
                    Err(_) => {
                        log("Failed to get image from tmdb-api");
                        return None;
                    }
                };

                match response.into_json::<serde_json::Value>() {
                    Ok(body) => {
                        let image_url = format!(
                            "https://image.tmdb.org/t/p/w600_and_h600_bestv2{}",
                            body["posters"][0]
                                .clone()
                                .get("file_path")
                                .unwrap()
                                .as_str()
                                .unwrap()
                        );
                        Some(image_url)
                    }
                    Err(_) => {
                        log("Movie image not correctly found");
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
                    Err(_) => return 0.0,
                };

                match response.into_json::<TraktRatingsResponse>() {
                    Ok(body) => {
                        self.rating_cache
                            .insert(movie_slug.to_string(), body.rating);
                        body.rating
                    }
                    Err(_) => 0.0,
                }
            }
        }
    }
}

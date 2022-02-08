use serde::{Deserialize, Serialize};
use std::{collections::HashMap, time::Duration};
use ureq::{Agent, AgentBuilder};

#[derive(Serialize, Deserialize)]
pub struct TraktMovie {
    pub title: String,
    pub year: u16,
    pub ids: TraktIds,
}

#[derive(Serialize, Deserialize)]
pub struct TraktShow {
    pub title: String,
    pub year: u16,
    pub ids: TraktIds,
}

#[derive(Serialize, Deserialize)]
pub struct TraktEpisode {
    pub season: u8,
    pub number: u8,
    pub title: String,
    pub ids: TraktIds,
}

#[derive(Serialize, Deserialize)]
pub struct TraktIds {
    pub trakt: u32,
    pub slug: Option<String>,
    pub tvdb: Option<u32>,
    pub imdb: Option<String>,
    pub tmdb: Option<u32>,
    pub tvrage: Option<u32>,
}

#[derive(Serialize, Deserialize)]
pub struct TraktWatchingResponse {
    pub expires_at: String,
    pub started_at: String,
    pub action: String,
    pub r#type: String,
    pub movie: Option<TraktMovie>,
    pub show: Option<TraktShow>,
    pub episode: Option<TraktEpisode>,
}

#[derive(Serialize, Deserialize)]
pub struct TraktRatingsResponse {
    pub rating: f64,
    pub votes: u32,
    pub distribution: HashMap<String, u16>,
}

pub struct Trakt {
    cache: HashMap<String, f64>,
    agent: Agent,
    client_id: String,
    username: String,
}

impl Trakt {
    pub fn new(client_id: String, username: String) -> Trakt {
        Trakt {
            cache: HashMap::default(),
            agent: AgentBuilder::new()
                .timeout_read(Duration::from_secs(5))
                .timeout_write(Duration::from_secs(5))
                .build(),
            client_id,
            username,
        }
    }

    pub fn get_watching(&self) -> Option<TraktWatchingResponse> {
        let endpoint = format!("https://api.trakt.tv/users/{}/watching", self.username);

        let response: String = self
            .agent
            .get(&endpoint)
            .set("Content-Type", "application/json")
            .set("trakt-api-version", "2")
            .set("trakt-api-key", &self.client_id)
            .call()
            .unwrap()
            .into_string()
            .unwrap();

        match serde_json::from_str(&response) {
            Ok(response) => Some(response),
            Err(_) => None,
        }
    }

    pub fn get_movie_rating(&mut self, movie_slug: String) -> Option<f64> {
        match self.cache.get(&movie_slug) {
            Some(rating) => Some(*rating),
            None => {
                let endpoint = format!("https://api.trakt.tv/movies/{}/ratings", movie_slug);

                let response: String = self
                    .agent
                    .get(&endpoint)
                    .set("Content-Type", "application/json")
                    .set("trakt-api-version", "2")
                    .set("trakt-api-key", &self.client_id)
                    .call()
                    .unwrap()
                    .into_string()
                    .unwrap();

                println!("{:?}", response);

                match serde_json::from_str(&response) as Result<TraktRatingsResponse, _> {
                    Ok(response) => {
                        self.cache.insert(movie_slug.to_string(), response.rating);
                        Some(response.rating)
                    }
                    Err(_) => Some(0.0),
                }
            }
        }
    }
}

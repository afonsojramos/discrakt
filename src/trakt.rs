use discord_rich_presence::activity::{Activity, Assets};
use serde::{Deserialize, Serialize};
use ureq::Agent;

#[derive(Serialize, Deserialize)]
pub struct TraktMovie {
    pub title: String,
    pub year: String,
    pub ids: TraktIds,
}

#[derive(Serialize, Deserialize)]
pub struct TraktShow {
    pub title: String,
    pub year: String,
    pub ids: TraktIds,
}

#[derive(Serialize, Deserialize)]
pub struct TraktEpisode {
    pub season: String,
    pub number: String,
    pub title: String,
    pub ids: TraktIds,
}

#[derive(Serialize, Deserialize)]
pub struct TraktIds {
    pub trakt: String,
    pub slug: String,
    pub tvdb: Option<String>,
    pub imdb: Option<String>,
    pub tmdb: Option<String>,
    pub tvrage: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct TraktBodyResponse {
    pub expires_at: String,
    pub started_at: String,
    pub action: String,
    pub type_: String,
    pub movie: Option<TraktMovie>,
    pub show: Option<TraktShow>,
    pub episode: Option<TraktEpisode>,
}

pub fn get_watching(
    agent: &Agent,
    username: &String,
    client_id: &String,
) -> Option<TraktBodyResponse> {
    let endpoint = format!("https://api.trakt.tv/users/{}/watching", username);

    let response: String = agent
        .get(&endpoint)
        .set("Content-Type", "application/json")
        .set("trakt-api-version", "2")
        .set("trakt-api-key", client_id)
        .call()
        .unwrap()
        .into_string()
        .unwrap();

    let deserialized: TraktBodyResponse = match serde_json::from_str(&response) {
        Ok(response) => response,
        Err(e) => {
            println!("{}", e);
            return None;
        }
    };

    Some(deserialized)
}

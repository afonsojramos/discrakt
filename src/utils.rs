use chrono::{DateTime, FixedOffset, SecondsFormat, Utc};
use configparser::ini::Ini;

use crate::trakt::TraktWatchingResponse;

pub struct Env {
    pub discord_token: String,
    pub trakt_username: String,
    pub trakt_client_id: String,
    pub tmdb_token: String,
}

pub struct WatchStats {
    pub watch_percentage: String,
    pub start_date: DateTime<FixedOffset>,
    pub end_date: DateTime<FixedOffset>,
}

pub fn load_config() -> Env {
    let mut config = Ini::new();
    config.load("credentials.ini").unwrap();

    Env {
        discord_token: config
            .get("Discord", "discordClientID")
            .expect("discordClientID not found"),
        trakt_username: config
            .get("Trakt API", "traktUser")
            .expect("traktUser not found"),
        trakt_client_id: config
            .get("Trakt API", "traktClientID")
            .expect("traktClientID not found"),
        tmdb_token: config
            .get("TMDB API", "tmdbToken")
            .expect("tmdbToken not found"),
    }
}

pub fn log(message: &str) {
    println!(
        "{} : {message}",
        Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
    );
}

pub fn get_watch_stats(trakt_response: &TraktWatchingResponse) -> WatchStats {
    let start_date = DateTime::parse_from_rfc3339(&trakt_response.started_at).unwrap();
    let end_date = DateTime::parse_from_rfc3339(&trakt_response.expires_at).unwrap();
    let percentage = Utc::now().signed_duration_since(start_date).num_seconds() as f32
        / end_date.signed_duration_since(start_date).num_seconds() as f32;
    let watch_percentage = format!("{:.2}%", percentage * 100.0);

    WatchStats {
        watch_percentage,
        start_date,
        end_date,
    }
}

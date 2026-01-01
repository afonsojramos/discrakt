use discord_rich_presence::{
    activity::{Activity, ActivityType, Assets, Button, StatusDisplayType, Timestamps},
    DiscordIpc, DiscordIpcClient,
};
use std::{thread::sleep, time::Duration};

use crate::{
    trakt::{Trakt, TraktWatchingResponse},
    utils::{
        get_watch_stats, MediaType, DEFAULT_DISCORD_APP_ID_MOVIE, DEFAULT_DISCORD_APP_ID_SHOW,
    },
};

pub struct Discord {
    client: DiscordIpcClient,
    current_app_id: String,
}

/// Payload data for Discord Rich Presence.
#[derive(Default, Debug, Clone, PartialEq)]
pub struct Payload {
    pub details: String,
    pub state: String,
    pub media: String,
    pub link_imdb: String,
    pub link_trakt: String,
    pub img_url: String,
    pub watch_percentage: String,
}

/// Build payload data from a Trakt watching response.
///
/// This function extracts the display information from a Trakt response
/// and formats it for Discord Rich Presence.
///
/// # Arguments
/// * `trakt_response` - The Trakt API response for what's currently being watched
/// * `movie_rating` - The movie rating (only used for movie type)
///
/// # Returns
/// * `Some(Payload)` - The payload data if the media type is recognized
/// * `None` - If the media type is unknown
pub fn build_payload(trakt_response: &TraktWatchingResponse, movie_rating: f64) -> Option<Payload> {
    let mut payload = Payload::default();

    match trakt_response.r#type.as_str() {
        "movie" => {
            let movie = trakt_response.movie.as_ref()?;
            payload.details = format!("{} ({})", movie.title, movie.year);
            payload.state = format!("{:.1} stars", movie_rating);
            payload.media = String::from("movies");
            payload.link_imdb = format!("https://www.imdb.com/title/{}", movie.ids.imdb.as_ref()?);
            payload.link_trakt = format!(
                "https://trakt.tv/{}/{}",
                payload.media,
                movie.ids.slug.as_ref()?
            );
            Some(payload)
        }
        "episode" => {
            let episode = trakt_response.episode.as_ref()?;
            let show = trakt_response.show.as_ref()?;
            payload.details = show.title.to_string();
            payload.state = format!(
                "S{:02}E{:02} - {}",
                episode.season, episode.number, episode.title
            );
            payload.media = String::from("shows");
            payload.link_imdb = format!("https://www.imdb.com/title/{}", show.ids.imdb.as_ref()?);
            payload.link_trakt = format!(
                "https://trakt.tv/{}/{}",
                payload.media,
                show.ids.slug.as_ref()?
            );
            Some(payload)
        }
        _ => None,
    }
}

/// Get the appropriate Discord app ID for a media type.
pub fn get_app_id_for_media_type(media_type: &str) -> &'static str {
    match media_type {
        "episode" => DEFAULT_DISCORD_APP_ID_SHOW,
        _ => DEFAULT_DISCORD_APP_ID_MOVIE,
    }
}

impl Discord {
    pub fn new(discord_client_id: String) -> Discord {
        Discord {
            client: DiscordIpcClient::new(&discord_client_id),
            current_app_id: discord_client_id,
        }
    }

    /// Switch to a different Discord application ID if needed.
    fn switch_app_id(&mut self, new_app_id: &str) {
        if self.current_app_id == new_app_id {
            return;
        }

        tracing::info!(
            "Switching Discord app ID from {} to {}",
            self.current_app_id,
            new_app_id
        );

        // Close existing connection
        let _ = self.client.close();

        // Create new client with new app ID
        self.client = DiscordIpcClient::new(new_app_id);
        self.current_app_id = new_app_id.to_string();
        self.connect();
    }

    pub fn connect(&mut self) {
        loop {
            if self.client.connect().is_ok() {
                break;
            }
            tracing::warn!("Failed to connect to Discord, retrying in 15 seconds");
            sleep(Duration::from_secs(15));
        }
    }

    pub fn close(&mut self) {
        let _ = self.client.close();
    }

    pub fn set_activity(
        &mut self,
        trakt_response: &TraktWatchingResponse,
        trakt: &mut Trakt,
        tmdb_token: String,
    ) {
        let mut payload_data = Payload::default();

        // Switch to appropriate Discord app ID based on media type
        let target_app_id = match trakt_response.r#type.as_str() {
            "episode" => DEFAULT_DISCORD_APP_ID_SHOW,
            _ => DEFAULT_DISCORD_APP_ID_MOVIE, // Default to movie for unknown types (including "movie")
        };
        self.switch_app_id(target_app_id);

        let img_url = match trakt_response.r#type.as_str() {
            "movie" => {
                let movie = trakt_response.movie.as_ref().unwrap();
                payload_data.details = format!("{} ({})", movie.title, movie.year);
                payload_data.state = format!(
                    "{:.1} ⭐️",
                    Trakt::get_movie_rating(trakt, movie.ids.slug.as_ref().unwrap().to_string())
                );
                payload_data.media = String::from("movies");
                payload_data.link_imdb = format!(
                    "https://www.imdb.com/title/{}",
                    movie.ids.imdb.as_ref().unwrap()
                );
                payload_data.link_trakt = format!(
                    "https://trakt.tv/{}/{}",
                    payload_data.media,
                    movie.ids.slug.as_ref().unwrap()
                );
                let id_tmdb = movie.ids.tmdb.as_ref().unwrap();

                trakt.get_poster(MediaType::Movie, id_tmdb.to_string(), tmdb_token.clone(), 0)
            }
            "episode" if trakt_response.episode.is_some() => {
                let episode = trakt_response.episode.as_ref().unwrap();
                let show = trakt_response.show.as_ref().unwrap();
                payload_data.details = show.title.to_string();
                payload_data.state = format!(
                    "S{:02}E{:02} - {}",
                    episode.season, episode.number, episode.title
                );
                payload_data.media = String::from("shows");
                payload_data.link_imdb = format!(
                    "https://www.imdb.com/title/{}",
                    show.ids.imdb.as_ref().unwrap()
                );
                payload_data.link_trakt = format!(
                    "https://trakt.tv/{}/{}",
                    payload_data.media,
                    show.ids.slug.as_ref().unwrap()
                );
                let id_tmdb = show.ids.tmdb.as_ref().unwrap();

                trakt.get_poster(
                    MediaType::Show,
                    id_tmdb.to_string(),
                    tmdb_token.clone(),
                    episode.season,
                )
            }
            _ => {
                tracing::warn!("Unknown media type: {}", trakt_response.r#type);
                return;
            }
        };

        if let Some(movie) = &trakt_response.movie {
            if let Some(tmdb_id) = movie.ids.tmdb {
                let translated = trakt.get_title(
                    MediaType::Movie,
                    tmdb_id.to_string(),
                    tmdb_token.clone(),
                    None,
                    None,
                );
                if !translated.is_empty() {
                    payload_data.details = format!("{} ({})", translated, movie.year);
                }
            }
        } else if let Some(show) = &trakt_response.show {
            if let Some(tmdb_id) = show.ids.tmdb {
                let show_title = trakt.get_title(
                    MediaType::Show,
                    tmdb_id.to_string(),
                    tmdb_token.clone(),
                    None,
                    None,
                );
                if !show_title.is_empty() {
                    payload_data.details = show_title;
                }

                if let Some(episode) = &trakt_response.episode {
                    let ep_title = trakt.get_title(
                        MediaType::Show,
                        tmdb_id.to_string(),
                        tmdb_token.clone(),
                        Some(episode.season),
                        Some(episode.number),
                    );

                    if !ep_title.is_empty() {
                        payload_data.state = format!(
                            "S{:02}E{:02} - {}",
                            episode.season, episode.number, ep_title
                        );
                    }
                }
            }
        }

        let img = match img_url {
            Some(img) => img,
            None => payload_data.media.to_string(),
        };

        let watch_time = get_watch_stats(trakt_response);

        let payload = Activity::new()
            .details(&payload_data.details)
            .state(&payload_data.state)
            .activity_type(ActivityType::Watching)
            .status_display_type(StatusDisplayType::Details)
            .assets(
                Assets::new()
                    .large_image(&img)
                    .small_image("trakt")
                    .small_text("Discrakt"),
            )
            .timestamps(
                Timestamps::new()
                    .start(watch_time.start_date.timestamp())
                    .end(watch_time.end_date.timestamp()),
            )
            .buttons(vec![
                Button::new("IMDB", &payload_data.link_imdb),
                Button::new("Trakt", &payload_data.link_trakt),
            ]);

        tracing::info!(
            details = %payload_data.details,
            state = %payload_data.state,
            progress = %watch_time.watch_percentage,
            "Now playing"
        );

        if self.client.set_activity(payload).is_err() {
            self.connect();
        }
    }
}

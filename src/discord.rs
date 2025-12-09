use discord_rich_presence::{
    activity::{Activity, ActivityType, Assets, Button, Timestamps},
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

#[derive(Default)]
pub struct Payload {
    pub details: String,
    pub state: String,
    pub media: String,
    pub link_imdb: String,
    pub link_trakt: String,
    pub img_url: String,
    pub watch_percentage: String,
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

                trakt.get_poster(MediaType::Movie, id_tmdb.to_string(), tmdb_token, 0)
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
                    tmdb_token,
                    episode.season,
                )
            }
            _ => {
                tracing::warn!("Unknown media type: {}", trakt_response.r#type);
                return;
            }
        };

        let img = match img_url {
            Some(img) => img,
            None => payload_data.media.to_string(),
        };

        let watch_time = get_watch_stats(trakt_response);

        let payload = Activity::new()
            .details(&payload_data.details)
            .state(&payload_data.state)
            .activity_type(ActivityType::Watching)
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

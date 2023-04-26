use discord_rich_presence::{
    activity::{Activity, Assets, Button, Timestamps},
    DiscordIpc, DiscordIpcClient,
};
use std::{thread::sleep, time::Duration};

use crate::{
    discord::payload::Payload,
    trakt::{Trakt, TraktWatchingResponse},
    utils::{get_watch_stats, log},
};

pub struct Discord {
    client: DiscordIpcClient,
}

impl Discord {
    pub fn new(discord_client_id: String) -> Discord {
        Discord {
            client: match DiscordIpcClient::new(&discord_client_id) {
                Ok(client) => client,
                Err(e) => {
                    log(&format!("Couldn't connect to Discord: {e}"));
                    panic!("Couldn't connect to Discord");
                }
            },
        }
    }

    pub fn connect(&mut self) {
        loop {
            if self.client.connect().is_ok() {
                break;
            } else {
                log("Failed to connect to Discord, retrying in 15 seconds");
                sleep(Duration::from_secs(15));
            }
        }
    }

    pub fn close(&mut self) {
        self.client.close().unwrap();
    }

    pub fn set_activity(
        &mut self,
        trakt_response: &TraktWatchingResponse,
        trakt: &mut Trakt,
        tmdb_token: String,
    ) {
        let mut payload_data = Payload::default();

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
                trakt.get_movie_image(id_tmdb.to_string(), tmdb_token)
            }
            "episode" if trakt_response.episode.is_some() => {
                let episode = trakt_response.episode.as_ref().unwrap();
                let show = trakt_response.show.as_ref().unwrap();
                payload_data.details = show.title.to_string();
                payload_data.state =
                    format!("S{}E{} - {}", episode.season, episode.number, episode.title);
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
                trakt.get_show_image(id_tmdb.to_string(), tmdb_token, episode.season)
            }
            _ => {
                log(&format!("Unknown media type: {}", trakt_response.r#type));
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
            .assets(
                Assets::new()
                    .large_image(&img)
                    .small_image("trakt")
                    .small_text("Trakt.tv"),
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

        log(&format!(
            "{} - {} | {}",
            payload_data.details, payload_data.state, watch_time.watch_percentage
        ));

        if self.client.set_activity(payload).is_err() {
            self.connect();
        }
    }
}

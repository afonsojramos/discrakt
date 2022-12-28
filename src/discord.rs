use chrono::{DateTime, Utc};
use discord_rich_presence::{
    activity::{Activity, Assets, Button, Timestamps},
    DiscordIpc, DiscordIpcClient,
};
use std::{thread::sleep, time::Duration};
use tmdb::model::*;
use tmdb::themoviedb::{Executable, Fetch, TMDbApi};

use crate::{
    trakt::{Trakt, TraktWatchingResponse},
    utils::log,
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

    pub fn set_activity(&mut self, trakt_response: &TraktWatchingResponse, trakt: &mut Trakt, tmdb: &dyn TMDbApi) {
        let details;
        let state;
        let media;
        let link_imdb;
        let link_trakt;
        let tmdb_id;
        let start_date = DateTime::parse_from_rfc3339(&trakt_response.started_at).unwrap();
        let end_date = DateTime::parse_from_rfc3339(&trakt_response.expires_at).unwrap();
        let now = Utc::now();
        let percentage = now.signed_duration_since(start_date).num_seconds() as f32
            / end_date.signed_duration_since(start_date).num_seconds() as f32;
        let watch_percentage = format!("{:.2}%", percentage * 100.0);

        match trakt_response.r#type.as_str() {
            "movie" => {
                let movie = trakt_response.movie.as_ref().unwrap();
                details = format!("{} ({})", movie.title, movie.year);
                state = format!(
                    "{:.1} ⭐️",
                    Trakt::get_movie_rating(trakt, movie.ids.slug.as_ref().unwrap().to_string())
                        .as_ref()
                        .unwrap()
                );
                media = "movies";
                link_imdb = format!(
                    "https://www.imdb.com/title/{}",
                    movie.ids.imdb.as_ref().unwrap()
                );
                link_trakt = format!(
                    "https://trakt.tv/{}/{}",
                    media,
                    movie.ids.slug.as_ref().unwrap()
                );
                tmdb_id = movie.ids.tmdb
            }
            "episode" if trakt_response.episode.is_some() => {
                let episode = trakt_response.episode.as_ref().unwrap();
                let show = trakt_response.show.as_ref().unwrap();
                details = show.title.to_string();
                state = format!("S{}E{} - {}", episode.season, episode.number, episode.title);
                media = "shows";
                link_imdb = format!(
                    "https://www.imdb.com/title/{}",
                    show.ids.imdb.as_ref().unwrap()
                );
                link_trakt = format!(
                    "https://trakt.tv/{}/{}",
                    media,
                    show.ids.slug.as_ref().unwrap()
                );
                tmdb_id = show.ids.tmdb
            }
            _ => {
                log(&format!("Unknown media type: {}", trakt_response.r#type));
                return;
            }
        }

        let poster_path: Option<String> = match tmdb_id {
            Some(id) => match media {
                "movies" => {
                    let movie = (tmdb
                        .fetch()
                        .id(id as u64)
                        as &dyn Executable<Movie>)
                        .execute();

                    if movie.is_ok()  {
                        movie.unwrap().poster_path
                    } else { None }
                },
                "shows" => {
                    let tv = (tmdb
                        .fetch()
                        .id(id as u64)
                        as &dyn Executable<TV>).execute();

                    if tv.is_ok()  {
                        tv.unwrap().poster_path
                    } else { None }
                },
                _ => None
            },
            None => None
        };

        let formatted: String;
        let asset_media_link = match &poster_path {
            None => media,
            Some(poster_path) => {
                formatted = format!("https://image.tmdb.org/t/p/original{}", poster_path);
                &formatted[..]
            }
        };

        log(&format!("{details} - {state} | {watch_percentage}"));

        let payload = Activity::new()
            .details(&details)
            .state(&state)
            .assets(
                Assets::new()
                    .large_image(asset_media_link)
                    .large_text(&watch_percentage)
                    .small_image("trakt")
                    .small_text("Discrakt"),
            )
            .timestamps(
                Timestamps::new()
                    .start(start_date.timestamp())
                    .end(end_date.timestamp()),
            )
            .buttons(vec![
                Button::new("IMDB", &link_imdb),
                Button::new("Trakt", &link_trakt),
            ]);

        if self.client.set_activity(payload).is_err() {
            self.connect();
        }
    }
}

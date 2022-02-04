use discord_rich_presence::{
    activity::{Activity, Assets},
    new_client, DiscordIpc,
};
use std::{thread::sleep, time::Duration};

use crate::trakt::TraktBodyResponse;

pub fn new(discord_token: String) -> Result<impl DiscordIpc, Box<dyn std::error::Error>> {
    new_client(&discord_token)
}

pub fn connect(discord_client: &mut impl DiscordIpc) {
    loop {
        if discord_client.connect().is_ok() {
            break;
        }

        println!("Discord Client not connected! Attempting to reconnect...");

        sleep(Duration::from_secs(15));
    }
}

pub fn set_activity(discord_client: &mut impl DiscordIpc, trakt_response: &TraktBodyResponse) {
    let details;
    let state;
    let media;

    match trakt_response.r#type.as_str() {
        "movie" => {
            let movie = trakt_response.movie.as_ref().unwrap();
            details = movie.title.to_owned();
            state = movie.year.to_owned();
            media = "movie";
        }
        "episode" if trakt_response.episode.is_some() => {
            let episode = trakt_response.episode.as_ref().unwrap();
            details = episode.title.to_owned();
            state = format!("S{}E{} - {}", episode.season, episode.number, episode.title,);
            media = "tv";
        }
        _ => {
            println!("Unknown media type: {}", trakt_response.r#type);
            return;
        }
    }

    let payload = Activity::new().details(&details).state(&state).assets(
        Assets::new()
            .large_image(&media)
            .large_text("hello")
            .small_image("movie")
            .small_text("movie"),
    );

    if discord_client.set_activity(payload).is_err() && discord_client.reconnect().is_ok() {
        return;
    }
}

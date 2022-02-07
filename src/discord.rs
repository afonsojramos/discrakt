use chrono::{DateTime, SecondsFormat, Utc};
use discord_rich_presence::{
    activity::{Activity, Assets, Timestamps},
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
    let start_date = DateTime::parse_from_rfc3339(&trakt_response.started_at).unwrap();
    let end_date = DateTime::parse_from_rfc3339(&trakt_response.expires_at).unwrap();
    let now = Utc::now();
    let percentage = now.signed_duration_since(start_date).num_seconds() as f32
        / end_date.signed_duration_since(start_date).num_seconds() as f32;
    let watch_percentage = format!("{:.2}%", percentage * 100.0);

    match trakt_response.r#type.as_str() {
        "movie" => {
            let movie = trakt_response.movie.as_ref().unwrap();
            details = movie.title.to_string();
            state = movie.year.to_string();
            media = "movies";
        }
        "episode" if trakt_response.episode.is_some() => {
            let episode = trakt_response.episode.as_ref().unwrap();
            let show = trakt_response.show.as_ref().unwrap();
            details = show.title.to_string();
            state = format!("S{}E{} - {}", episode.season, episode.number, episode.title);
            media = "shows";
        }
        _ => {
            println!("Unknown media type: {}", trakt_response.r#type);
            return;
        }
    }

    println!(
        "{} : {} - {} | {}",
        now.to_rfc3339_opts(SecondsFormat::Secs, true),
        details,
        state,
        watch_percentage
    );

    let payload = Activity::new()
        .details(&details)
        .state(&state)
        .assets(
            Assets::new()
                .large_image(&media)
                .large_text(&watch_percentage)
                .small_image("trakt")
                .small_text("Discrakt"),
        )
        .timestamps(
            Timestamps::new()
                .start(start_date.timestamp())
                .end(end_date.timestamp()),
        );

    if discord_client.set_activity(payload).is_err() && discord_client.reconnect().is_ok() {
        return;
    }
}

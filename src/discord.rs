use discord_rich_presence::{
    activity::{Activity, ActivityType, Assets, Button, StatusDisplayType, Timestamps},
    DiscordIpc, DiscordIpcClient,
};
use std::{thread::sleep, time::Duration};

use crate::source::{MediaKind, Watching};
use crate::utils::{get_watch_stats, DEFAULT_DISCORD_APP_ID_MOVIE, DEFAULT_DISCORD_APP_ID_SHOW};

pub struct Discord {
    client: DiscordIpcClient,
    current_app_id: String,
}

/// Display data for Discord Rich Presence, derived purely from a [`Watching`].
#[derive(Default, Debug, Clone, PartialEq)]
pub struct Payload {
    pub details: String,
    pub state: String,
    /// Media category: `movies` or `shows`.
    pub media: String,
    /// Large image: a resolved poster URL, or the media category as a fallback.
    pub large_image: String,
    /// Rich Presence buttons as `(label, url)` pairs.
    pub buttons: Vec<(String, String)>,
}

/// Builds Discord display data from a source-agnostic [`Watching`].
///
/// All localization and artwork resolution has already been done by the source,
/// so this is pure formatting: no network calls, no source-specific logic.
pub fn build_payload(watching: &Watching) -> Payload {
    let media = match watching.kind {
        MediaKind::Movie => "movies",
        MediaKind::Episode => "shows",
    }
    .to_string();

    let (details, state) = match watching.kind {
        MediaKind::Movie => {
            let details = match watching.year {
                Some(year) => format!("{} ({})", watching.title, year),
                None => watching.title.clone(),
            };
            let state = match watching.rating {
                Some(rating) => format!("{:.1} ⭐️", rating),
                None => String::new(),
            };
            (details, state)
        }
        MediaKind::Episode => {
            let details = watching.title.clone();
            let state = format!(
                "S{:02}E{:02} - {}",
                watching.season.unwrap_or(0),
                watching.episode_number.unwrap_or(0),
                watching.episode_title.as_deref().unwrap_or("")
            );
            (details, state)
        }
    };

    let large_image = watching.poster_url.clone().unwrap_or_else(|| media.clone());

    let mut buttons = Vec::new();
    // Primary: the movie/show page. Prefer TMDB (its id is resolved for every
    // source), falling back to IMDB when no TMDB id is available.
    if let Some(tmdb) = watching.ids.tmdb {
        let path = match watching.kind {
            MediaKind::Movie => "movie",
            MediaKind::Episode => "tv",
        };
        buttons.push((
            "TMDB".to_string(),
            format!("https://www.themoviedb.org/{path}/{tmdb}"),
        ));
    } else if let Some(imdb_url) = &watching.imdb_url {
        buttons.push(("IMDB".to_string(), imdb_url.clone()));
    }
    // Secondary: always link back to the project.
    buttons.push((
        "Discrakt".to_string(),
        env!("CARGO_PKG_REPOSITORY").to_string(),
    ));

    Payload {
        details,
        state,
        media,
        large_image,
        buttons,
    }
}

/// Returns the appropriate Discord application ID for a media kind.
pub fn app_id_for_kind(kind: MediaKind) -> &'static str {
    match kind {
        MediaKind::Episode => DEFAULT_DISCORD_APP_ID_SHOW,
        MediaKind::Movie => DEFAULT_DISCORD_APP_ID_MOVIE,
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

    pub fn clear_activity(&mut self) {
        let _ = self.client.clear_activity();
    }

    pub fn close(&mut self) {
        let _ = self.client.close();
    }

    pub fn set_activity(&mut self, watching: &Watching) {
        // Switch to the appropriate Discord app ID based on media kind.
        self.switch_app_id(app_id_for_kind(watching.kind));

        let payload = build_payload(watching);
        let watch_time = get_watch_stats(watching);

        let buttons: Vec<Button> = payload
            .buttons
            .iter()
            .map(|(label, url)| Button::new(label, url))
            .collect();

        let mut activity = Activity::new()
            .details(&payload.details)
            .state(&payload.state)
            .activity_type(ActivityType::Watching)
            .status_display_type(StatusDisplayType::Details)
            .assets(
                Assets::new()
                    .large_image(&payload.large_image)
                    .small_image("trakt")
                    .small_text("Discrakt"),
            )
            .timestamps(
                Timestamps::new()
                    .start(watch_time.start_date.timestamp())
                    .end(watch_time.end_date.timestamp()),
            );

        if !buttons.is_empty() {
            activity = activity.buttons(buttons);
        }

        tracing::info!(
            details = %payload.details,
            state = %payload.state,
            progress = %watch_time.watch_percentage,
            "Now playing"
        );

        if self.client.set_activity(activity).is_err() {
            self.connect();
        }
    }
}

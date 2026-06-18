use std::time::Duration;

use chrono::Utc;
use serde::Deserialize;
use ureq::Agent;

use crate::metadata::Tmdb;
use crate::retry::{execute_with_retry, RetryConfig, RetryError};
use crate::source::{MediaIds, MediaKind, Source, Watching};
use crate::utils::{user_agent, MediaType};

/// Configuration for a [`PlexSource`].
#[derive(Clone, Default)]
pub struct PlexConfig {
    /// Base URL of the Plex Media Server, e.g. `http://192.168.1.10:32400`.
    pub server_url: String,
    /// A Plex authentication token (`X-Plex-Token`).
    pub token: String,
    /// The Plex account username whose session should be mirrored. When empty,
    /// the first playing video session is used (handy for single-user servers).
    pub username: String,
    pub tmdb_token: String,
    /// Base URL for TMDB (defaults to the public API). Primarily for testing.
    pub tmdb_base_url: Option<String>,
    pub language: Option<String>,
}

#[derive(Deserialize)]
struct SessionsResponse {
    #[serde(rename = "MediaContainer")]
    media_container: MediaContainer,
}

#[derive(Deserialize)]
struct MediaContainer {
    #[serde(rename = "Metadata", default)]
    metadata: Vec<PlexMetadata>,
}

#[derive(Deserialize)]
struct PlexMetadata {
    #[serde(rename = "type")]
    kind: String,
    title: Option<String>,
    #[serde(rename = "grandparentTitle")]
    grandparent_title: Option<String>,
    #[serde(rename = "parentIndex")]
    parent_index: Option<u8>,
    index: Option<u8>,
    year: Option<u16>,
    /// Total runtime in milliseconds.
    duration: Option<i64>,
    /// Current playback position in milliseconds.
    #[serde(rename = "viewOffset")]
    view_offset: Option<i64>,
    #[serde(rename = "Guid", default)]
    guids: Vec<PlexGuid>,
    #[serde(rename = "grandparentGuid")]
    grandparent_guid: Option<String>,
    #[serde(rename = "User")]
    user: Option<PlexUser>,
    #[serde(rename = "Player")]
    player: Option<PlexPlayer>,
}

#[derive(Deserialize)]
struct PlexGuid {
    id: String,
}

#[derive(Deserialize)]
struct PlexUser {
    title: Option<String>,
}

#[derive(Deserialize)]
struct PlexPlayer {
    state: Option<String>,
}

/// A [`Source`] backed by a Plex Media Server's live sessions.
///
/// Polls `GET /status/sessions`, selects the configured user's actively-playing
/// video session, and maps it into an enriched [`Watching`]. TMDB ids exposed by
/// Plex's `Guid` metadata are used to resolve artwork and localized titles on a
/// best-effort basis; when they are absent, Plex's own title is used and the
/// default media artwork is shown.
pub struct PlexSource {
    agent: Agent,
    server_url: String,
    token: String,
    username: String,
    tmdb: Tmdb,
    tmdb_token: String,
    retry_config: RetryConfig,
}

impl PlexSource {
    pub fn new(config: PlexConfig) -> Self {
        let agent_config = Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(10)))
            .user_agent(user_agent())
            .build();

        PlexSource {
            agent: agent_config.into(),
            server_url: config.server_url.trim_end_matches('/').to_string(),
            token: config.token,
            username: config.username,
            tmdb: Tmdb::new(config.tmdb_base_url, config.language),
            tmdb_token: config.tmdb_token,
            retry_config: RetryConfig::default(),
        }
    }

    /// Overrides the retry configuration (primarily for tests).
    pub fn set_retry_config(&mut self, config: RetryConfig) {
        self.retry_config = config;
    }

    fn fetch_sessions(&self) -> Option<SessionsResponse> {
        let endpoint = format!("{}/status/sessions", self.server_url);
        let agent = &self.agent;
        let token = &self.token;

        let result: Result<SessionsResponse, RetryError> = execute_with_retry(
            || {
                agent
                    .get(&endpoint)
                    .header("Accept", "application/json")
                    .header("X-Plex-Token", token)
                    .call()
            },
            &self.retry_config,
        );

        match result {
            Ok(response) => Some(response),
            Err(RetryError::NonRetryableError(code @ (401 | 403))) => {
                tracing::error!(status = code, "Plex token rejected; check X-Plex-Token");
                None
            }
            Err(err) => {
                tracing::error!(error = %err, "Failed to fetch Plex sessions");
                None
            }
        }
    }

    /// Returns true if this session belongs to the configured user (or any user
    /// when no username is configured).
    fn matches_user(&self, meta: &PlexMetadata) -> bool {
        if self.username.is_empty() {
            return true;
        }
        meta.user
            .as_ref()
            .and_then(|user| user.title.as_deref())
            .is_some_and(|title| title.eq_ignore_ascii_case(&self.username))
    }

    fn is_playing(meta: &PlexMetadata) -> bool {
        meta.player
            .as_ref()
            .and_then(|player| player.state.as_deref())
            .is_some_and(|state| state == "playing")
    }

    fn enrich(&mut self, meta: &PlexMetadata) -> Option<Watching> {
        // Plex reports the current position; derive a precise progress window.
        let now = Utc::now();
        let offset_ms = meta.view_offset.unwrap_or(0).max(0);
        let duration_ms = meta.duration.unwrap_or(0).max(0);
        let remaining_ms = (duration_ms - offset_ms).max(0);
        let started_at = (now - chrono::Duration::milliseconds(offset_ms)).fixed_offset();
        let expires_at = (now + chrono::Duration::milliseconds(remaining_ms)).fixed_offset();

        let token = self.tmdb_token.clone();
        let imdb = extract_guid(&meta.guids, "imdb://");
        let imdb_url = imdb
            .as_ref()
            .map(|imdb| format!("https://www.imdb.com/title/{}", imdb));

        match meta.kind.as_str() {
            "movie" => {
                let mut title = meta.title.clone().unwrap_or_default();
                let tmdb_id =
                    extract_guid(&meta.guids, "tmdb://").and_then(|id| id.parse::<u32>().ok());
                let mut poster_url = None;

                if let Some(id) = tmdb_id {
                    poster_url =
                        self.tmdb
                            .get_poster(MediaType::Movie, id.to_string(), token.clone(), 0);
                    let localized =
                        self.tmdb
                            .get_title(MediaType::Movie, id.to_string(), &token, None, None);
                    if !localized.is_empty() {
                        title = localized;
                    }
                }

                Some(Watching {
                    kind: MediaKind::Movie,
                    title,
                    year: meta.year,
                    season: None,
                    episode_number: None,
                    episode_title: None,
                    ids: MediaIds {
                        imdb,
                        tmdb: tmdb_id,
                        slug: None,
                    },
                    rating: None,
                    poster_url,
                    imdb_url,
                    source_link: None,
                    started_at,
                    expires_at,
                    runtime_minutes: None,
                })
            }
            "episode" => {
                let mut title = meta.grandparent_title.clone().unwrap_or_default();
                let mut episode_title = meta.title.clone().unwrap_or_default();
                let season = meta.parent_index;
                let number = meta.index;
                // The show's TMDB id (used for artwork) lives on the grandparent guid.
                let show_tmdb = meta
                    .grandparent_guid
                    .as_deref()
                    .and_then(extract_tmdb_from_str);
                let mut poster_url = None;

                if let (Some(id), Some(s)) = (show_tmdb, season) {
                    poster_url =
                        self.tmdb
                            .get_poster(MediaType::Show, id.to_string(), token.clone(), s);
                    let localized_show =
                        self.tmdb
                            .get_title(MediaType::Show, id.to_string(), &token, None, None);
                    if !localized_show.is_empty() {
                        title = localized_show;
                    }
                    if let Some(n) = number {
                        let localized_episode = self.tmdb.get_title(
                            MediaType::Show,
                            id.to_string(),
                            &token,
                            Some(s),
                            Some(n),
                        );
                        if !localized_episode.is_empty() {
                            episode_title = localized_episode;
                        }
                    }
                }

                Some(Watching {
                    kind: MediaKind::Episode,
                    title,
                    year: meta.year,
                    season,
                    episode_number: number,
                    episode_title: Some(episode_title),
                    ids: MediaIds {
                        imdb,
                        tmdb: show_tmdb,
                        slug: None,
                    },
                    rating: None,
                    poster_url,
                    imdb_url,
                    source_link: None,
                    started_at,
                    expires_at,
                    runtime_minutes: None,
                })
            }
            other => {
                tracing::debug!("Ignoring non-video Plex session type: {}", other);
                None
            }
        }
    }
}

impl Source for PlexSource {
    fn get_watching(&mut self) -> Option<Watching> {
        let sessions = self.fetch_sessions()?;

        // Find the configured user's actively-playing video session. We collect
        // an owned copy of the matched metadata to release the borrow on
        // `sessions` before the mutable `enrich` call.
        let index = sessions
            .media_container
            .metadata
            .iter()
            .position(|meta| Self::is_playing(meta) && self.matches_user(meta))?;
        let meta = sessions.media_container.metadata.into_iter().nth(index)?;

        self.enrich(&meta)
    }

    fn set_language(&mut self, language: String) {
        self.tmdb.set_language(language);
    }
}

/// Extracts the value of a Plex `Guid` with the given scheme prefix
/// (e.g. `"tmdb://"`), returning the part after the prefix.
fn extract_guid(guids: &[PlexGuid], scheme: &str) -> Option<String> {
    guids
        .iter()
        .find_map(|guid| guid.id.strip_prefix(scheme).map(str::to_string))
}

/// Extracts a TMDB numeric id embedded anywhere in a guid string, tolerating the
/// various agent formats Plex uses (e.g. `tmdb://1396`).
fn extract_tmdb_from_str(value: &str) -> Option<u32> {
    let start = value.find("tmdb://")? + "tmdb://".len();
    let digits: String = value[start..]
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse().ok()
}

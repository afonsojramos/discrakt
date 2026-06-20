use std::num::NonZeroUsize;
use std::time::Duration;

use chrono::{DateTime, FixedOffset, Utc};
use lru::LruCache;
use serde::Deserialize;
use ureq::Agent;

use crate::jellyfin_auth::auth_header;
use crate::metadata::Tmdb;
use crate::retry::{execute_with_retry, RetryConfig, RetryError};
use crate::source::{MediaIds, MediaKind, Source, Watching};
use crate::utils::{user_agent, MediaType};

/// One Jellyfin tick is 100 nanoseconds; 10,000 ticks make a millisecond.
const TICKS_PER_MS: i64 = 10_000;
/// Runtime assumed when Jellyfin reports none, so the session still displays.
const DEFAULT_RUNTIME_MS: i64 = 2 * 60 * 60 * 1000;
/// Upper bound on cached series-to-TMDB-id mappings.
const SERIES_CACHE_SIZE: usize = 512;

/// Configuration for a [`JellyfinSource`].
#[derive(Clone, Default)]
pub struct JellyfinConfig {
    /// Base URL of the Jellyfin server, e.g. `http://192.168.1.10:8096`.
    pub server_url: String,
    /// Access token (from Quick Connect or a manually supplied API key).
    pub access_token: String,
    /// A stable device identifier, sent in the `Authorization` header.
    pub device_id: String,
    /// The Jellyfin user id whose session should be mirrored (Quick Connect).
    pub user_id: String,
    /// The Jellyfin username, used to filter sessions when no user id is set.
    pub username: String,
    pub tmdb_token: String,
    pub tmdb_base_url: Option<String>,
    pub language: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Session {
    #[serde(default)]
    user_id: String,
    #[serde(default)]
    user_name: String,
    now_playing_item: Option<NowPlaying>,
    play_state: Option<PlayState>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct NowPlaying {
    name: Option<String>,
    #[serde(rename = "Type")]
    kind: Option<String>,
    series_name: Option<String>,
    series_id: Option<String>,
    index_number: Option<u16>,
    parent_index_number: Option<u16>,
    production_year: Option<u16>,
    run_time_ticks: Option<i64>,
    provider_ids: Option<ProviderIds>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
struct ProviderIds {
    tmdb: Option<String>,
    imdb: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct PlayState {
    position_ticks: Option<i64>,
    is_paused: Option<bool>,
}

/// Minimal shape of `/Items?ids=...`, used to resolve a series' external ids.
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ItemsResponse {
    #[serde(default)]
    items: Vec<ItemProviderIds>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ItemProviderIds {
    provider_ids: Option<ProviderIds>,
}

/// A [`Source`] backed by a Jellyfin server's live sessions.
pub struct JellyfinSource {
    agent: Agent,
    server_url: String,
    access_token: String,
    device_id: String,
    user_id: String,
    username: String,
    tmdb: Tmdb,
    tmdb_token: String,
    retry_config: RetryConfig,
    /// Caches the TMDB id resolved for a series id, so the lookup happens once.
    series_tmdb_cache: LruCache<String, Option<u32>>,
}

impl JellyfinSource {
    pub fn new(config: JellyfinConfig) -> Self {
        let agent_config = Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(10)))
            .user_agent(user_agent())
            .build();

        JellyfinSource {
            agent: agent_config.into(),
            server_url: config.server_url.trim_end_matches('/').to_string(),
            access_token: config.access_token,
            device_id: config.device_id,
            user_id: config.user_id,
            username: config.username,
            tmdb: Tmdb::new(config.tmdb_base_url, config.language),
            tmdb_token: config.tmdb_token,
            retry_config: RetryConfig::default(),
            series_tmdb_cache: LruCache::new(
                NonZeroUsize::new(SERIES_CACHE_SIZE).expect("SERIES_CACHE_SIZE must be non-zero"),
            ),
        }
    }

    pub fn set_retry_config(&mut self, config: RetryConfig) {
        self.retry_config = config;
    }

    fn fetch_sessions(&self) -> Option<Vec<Session>> {
        let endpoint = format!("{}/Sessions", self.server_url);
        let agent = &self.agent;
        let header = auth_header(&self.device_id, Some(&self.access_token));

        let result: Result<Vec<Session>, RetryError> = execute_with_retry(
            || {
                agent
                    .get(&endpoint)
                    .header("Accept", "application/json")
                    .header("Authorization", &header)
                    .call()
            },
            &self.retry_config,
        );

        match result {
            Ok(sessions) => Some(sessions),
            Err(RetryError::NonRetryableError(code @ (401 | 403))) => {
                tracing::error!(status = code, "Jellyfin token rejected");
                None
            }
            Err(err) => {
                tracing::error!(error = %err, "Failed to fetch Jellyfin sessions");
                None
            }
        }
    }

    fn matches_user(&self, session: &Session) -> bool {
        if !self.user_id.is_empty() {
            return session.user_id == self.user_id;
        }
        if !self.username.is_empty() {
            return session.user_name.eq_ignore_ascii_case(&self.username);
        }
        true
    }

    fn is_playing(session: &Session) -> bool {
        // Require play state: a session with a now-playing item but no play state
        // isn't actively playing, and treating it as such would surface stale data
        // from position 0. Mirrors the Plex source's stricter default.
        let Some(play_state) = session.play_state.as_ref() else {
            return false;
        };
        session.now_playing_item.is_some() && !play_state.is_paused.unwrap_or(false)
    }

    /// Resolves a series' TMDB id (cached), since a session exposes only the
    /// episode's provider ids but artwork needs the show's id.
    fn resolve_series_tmdb(&mut self, series_id: &str) -> Option<u32> {
        if let Some(cached) = self.series_tmdb_cache.get(series_id) {
            return *cached;
        }

        let endpoint = format!("{}/Items", self.server_url);
        let agent = &self.agent;
        let header = auth_header(&self.device_id, Some(&self.access_token));

        // Jellyfin omits ProviderIds from the slim item DTO unless explicitly
        // requested, so without `Fields=ProviderIds` the TMDB id is never found
        // and the episode loses its artwork and localized title.
        let result: Result<ItemsResponse, RetryError> = execute_with_retry(
            || {
                agent
                    .get(&endpoint)
                    .query("ids", series_id)
                    .query("Fields", "ProviderIds")
                    .header("Accept", "application/json")
                    .header("Authorization", &header)
                    .call()
            },
            &self.retry_config,
        );

        match result {
            Ok(response) => {
                let tmdb_id = response
                    .items
                    .first()
                    .and_then(|item| item.provider_ids.as_ref())
                    .and_then(|ids| ids.tmdb.as_ref())
                    .and_then(|id| id.parse().ok());
                self.series_tmdb_cache.put(series_id.to_string(), tmdb_id);
                tmdb_id
            }
            // Don't cache transient failures, so the next poll retries instead of
            // pinning the series to "no artwork" for the rest of the process.
            Err(err) => {
                tracing::debug!(error = %err, series_id, "Failed to resolve Jellyfin series ids");
                None
            }
        }
    }

    fn enrich(&mut self, session: &Session) -> Option<Watching> {
        let item = session.now_playing_item.as_ref()?;
        let position_ms = session
            .play_state
            .as_ref()
            .and_then(|p| p.position_ticks)
            .unwrap_or(0)
            .max(0)
            / TICKS_PER_MS;
        let provider = item.provider_ids.as_ref();
        let imdb = provider.and_then(|p| p.imdb.clone());
        let imdb_url = imdb
            .as_ref()
            .map(|imdb| format!("https://www.imdb.com/title/{}", imdb));
        let token = &self.tmdb_token;

        let watching = match item.kind.as_deref() {
            Some("Movie") => {
                let mut title = item.name.clone().unwrap_or_default();
                let tmdb_id = provider
                    .and_then(|p| p.tmdb.as_deref())
                    .and_then(parse_tmdb);
                let mut poster_url = None;

                if let Some(id) = tmdb_id {
                    poster_url = self
                        .tmdb
                        .get_poster(MediaType::Movie, id.to_string(), token, 0);
                    let localized =
                        self.tmdb
                            .get_title(MediaType::Movie, id.to_string(), token, None, None);
                    if !localized.is_empty() {
                        title = localized;
                    }
                }

                let (started_at, expires_at) = window(position_ms, item.run_time_ticks);
                Watching {
                    kind: MediaKind::Movie,
                    title,
                    year: item.production_year,
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
                    started_at,
                    expires_at,
                    runtime_minutes: None,
                }
            }
            Some("Episode") => {
                let mut title = item.series_name.clone().unwrap_or_default();
                let mut episode_title = item.name.clone().unwrap_or_default();
                let season = item.parent_index_number;
                let number = item.index_number;
                let show_tmdb = item
                    .series_id
                    .as_deref()
                    .and_then(|id| self.resolve_series_tmdb(id));
                let token = &self.tmdb_token;
                let mut poster_url = None;

                if let Some(id) = show_tmdb {
                    let localized_show =
                        self.tmdb
                            .get_title(MediaType::Show, id.to_string(), token, None, None);
                    if !localized_show.is_empty() {
                        title = localized_show;
                    }
                    // Posters are season-specific; fall back to season 0 when the
                    // library didn't tag the episode with a season, so the episode
                    // still gets artwork instead of silently rendering with none.
                    poster_url = self.tmdb.get_poster(
                        MediaType::Show,
                        id.to_string(),
                        token,
                        season.unwrap_or(0),
                    );
                    if let (Some(s), Some(n)) = (season, number) {
                        let localized_episode = self.tmdb.get_title(
                            MediaType::Show,
                            id.to_string(),
                            token,
                            Some(s),
                            Some(n),
                        );
                        if !localized_episode.is_empty() {
                            episode_title = localized_episode;
                        }
                    }
                }

                let (started_at, expires_at) = window(position_ms, item.run_time_ticks);
                Watching {
                    kind: MediaKind::Episode,
                    title,
                    year: item.production_year,
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
                    started_at,
                    expires_at,
                    runtime_minutes: None,
                }
            }
            other => {
                tracing::debug!("Ignoring non-video Jellyfin item: {:?}", other);
                return None;
            }
        };

        Some(watching)
    }
}

impl Source for JellyfinSource {
    fn get_watching(&mut self) -> Option<Watching> {
        let sessions = self.fetch_sessions()?;
        let session = sessions
            .into_iter()
            .find(|s| Self::is_playing(s) && self.matches_user(s))?;
        self.enrich(&session)
    }

    fn set_language(&mut self, language: String) {
        self.tmdb.set_language(language);
    }
}

fn parse_tmdb(value: &str) -> Option<u32> {
    value.parse().ok()
}

/// Derives a progress window from the current position (ms) and runtime (ticks),
/// sampled now so it reflects the moment the result is built.
fn window(
    position_ms: i64,
    run_time_ticks: Option<i64>,
) -> (DateTime<FixedOffset>, DateTime<FixedOffset>) {
    let now = Utc::now();
    let duration_ms = match run_time_ticks.unwrap_or(0).max(0) / TICKS_PER_MS {
        0 => DEFAULT_RUNTIME_MS,
        ms => ms,
    };
    let offset_ms = position_ms.clamp(0, duration_ms);
    let remaining_ms = (duration_ms - offset_ms).max(0);
    let started_at = (now - chrono::Duration::milliseconds(offset_ms)).fixed_offset();
    let expires_at = (now + chrono::Duration::milliseconds(remaining_ms)).fixed_offset();
    (started_at, expires_at)
}

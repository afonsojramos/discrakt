use chrono::{DateTime, FixedOffset};

pub mod jellyfin;
pub mod plex;
pub mod trakt;

/// The kind of media being watched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKind {
    Movie,
    Episode,
}

/// External identifiers for a media item, used for artwork lookups and links.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MediaIds {
    pub imdb: Option<String>,
    pub tmdb: Option<u32>,
    /// Source-specific slug (e.g. a Trakt slug). `None` for sources without one.
    pub slug: Option<String>,
}

/// A source-agnostic, render-ready snapshot of what a user is watching right now.
///
/// Every [`Source`] produces this; [`crate::discord`] consumes only this. Titles
/// are already localized and artwork already resolved by the source, so Discord
/// is responsible purely for formatting and presentation, not for any API calls.
#[derive(Debug, Clone, PartialEq)]
pub struct Watching {
    pub kind: MediaKind,
    /// Movie or show title, already localized when a translation was available.
    pub title: String,
    pub year: Option<u16>,
    pub season: Option<u8>,
    pub episode_number: Option<u8>,
    /// Episode title, already localized when available (episodes only).
    pub episode_title: Option<String>,
    pub ids: MediaIds,
    /// Rating on a 0.0-10.0 scale (movies only); `None` hides the rating line.
    pub rating: Option<f64>,
    /// Resolved poster image URL; `None` falls back to the default media artwork.
    pub poster_url: Option<String>,
    /// IMDB page URL for the title, when available.
    pub imdb_url: Option<String>,
    /// A source-specific deep link rendered as a button, e.g. `("Trakt", url)`.
    pub source_link: Option<(String, String)>,
    pub started_at: DateTime<FixedOffset>,
    pub expires_at: DateTime<FixedOffset>,
    /// Runtime in minutes when known; used to derive a precise progress window.
    pub runtime_minutes: Option<u16>,
}

/// A tracking source that can report what the user is watching right now.
///
/// Implementors poll their backend and return a fully-enriched [`Watching`], or
/// `None` when nothing is playing or an error occurred.
/// `Send` is required because the polling loop owns a `Box<dyn Source>` inside a
/// spawned thread (see `main`).
pub trait Source: Send {
    fn get_watching(&mut self) -> Option<Watching>;

    /// Updates the preferred language for localized titles. No-op by default.
    fn set_language(&mut self, _language: String) {}
}

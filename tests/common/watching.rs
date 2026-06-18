// Shared builders for the source-agnostic `Watching` type used across tests.
#![allow(dead_code)]

use chrono::DateTime;
use discrakt::source::{MediaIds, MediaKind, Watching};

/// A movie `Watching` mirroring the Inception fixture, enriched as a source would.
pub fn movie_watching() -> Watching {
    Watching {
        kind: MediaKind::Movie,
        title: "Inception".to_string(),
        year: Some(2010),
        season: None,
        episode_number: None,
        episode_title: None,
        ids: MediaIds {
            imdb: Some("tt1375666".to_string()),
            tmdb: Some(27205),
            slug: Some("inception-2010".to_string()),
        },
        rating: Some(8.5),
        poster_url: Some("https://image.tmdb.org/t/p/w600_and_h600_bestv2/poster.jpg".to_string()),
        imdb_url: Some("https://www.imdb.com/title/tt1375666".to_string()),
        source_link: Some((
            "Trakt".to_string(),
            "https://trakt.tv/movies/inception-2010".to_string(),
        )),
        started_at: DateTime::parse_from_rfc3339("2024-01-15T10:00:00.000Z").unwrap(),
        expires_at: DateTime::parse_from_rfc3339("2024-01-15T12:30:00.000Z").unwrap(),
        runtime_minutes: None,
    }
}

/// An episode `Watching` mirroring the Breaking Bad / Felina fixture.
pub fn episode_watching() -> Watching {
    Watching {
        kind: MediaKind::Episode,
        title: "Breaking Bad".to_string(),
        year: Some(2008),
        season: Some(5),
        episode_number: Some(16),
        episode_title: Some("Felina".to_string()),
        ids: MediaIds {
            imdb: Some("tt0903747".to_string()),
            tmdb: Some(1396),
            slug: Some("breaking-bad".to_string()),
        },
        rating: None,
        poster_url: None,
        imdb_url: Some("https://www.imdb.com/title/tt0903747".to_string()),
        source_link: Some((
            "Trakt".to_string(),
            "https://trakt.tv/shows/breaking-bad".to_string(),
        )),
        started_at: DateTime::parse_from_rfc3339("2024-01-15T10:00:00.000Z").unwrap(),
        expires_at: DateTime::parse_from_rfc3339("2024-01-15T11:00:00.000Z").unwrap(),
        runtime_minutes: None,
    }
}

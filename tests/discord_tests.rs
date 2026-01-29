// Tests for Discord module in src/discord.rs

mod common;

use discrakt::discord::{build_payload, get_app_id_for_media_type, Payload};
use discrakt::trakt::{TraktEpisode, TraktIds, TraktMovie, TraktShow, TraktWatchingResponse};
use discrakt::utils::{DEFAULT_DISCORD_APP_ID_MOVIE, DEFAULT_DISCORD_APP_ID_SHOW};

// ============================================================================
// Payload Default Tests
// ============================================================================

#[test]
fn test_payload_default() {
    let payload = Payload::default();

    assert_eq!(payload.details, "");
    assert_eq!(payload.state, "");
    assert_eq!(payload.media, "");
    assert_eq!(payload.link_imdb, "");
    assert_eq!(payload.link_trakt, "");
    assert_eq!(payload.img_url, "");
    assert_eq!(payload.watch_percentage, "");
}

#[test]
fn test_payload_clone() {
    let payload = Payload {
        details: "Test Details".to_string(),
        state: "Test State".to_string(),
        media: "movies".to_string(),
        link_imdb: "https://imdb.com/test".to_string(),
        link_trakt: "https://trakt.tv/test".to_string(),
        img_url: "https://image.test/img.jpg".to_string(),
        watch_percentage: "50%".to_string(),
    };

    let cloned = payload.clone();
    assert_eq!(cloned.details, payload.details);
    assert_eq!(cloned.state, payload.state);
    assert_eq!(cloned.media, payload.media);
}

#[test]
fn test_payload_debug() {
    let payload = Payload::default();
    let debug_str = format!("{:?}", payload);

    assert!(debug_str.contains("Payload"));
    assert!(debug_str.contains("details"));
}

// ============================================================================
// Build Payload Tests - Movie
// ============================================================================

fn create_movie_response() -> TraktWatchingResponse {
    TraktWatchingResponse {
        expires_at: "2024-01-15T12:30:00.000Z".to_string(),
        started_at: "2024-01-15T10:00:00.000Z".to_string(),
        action: "watching".to_string(),
        r#type: "movie".to_string(),
        movie: Some(TraktMovie {
            title: "Inception".to_string(),
            year: 2010,
            ids: TraktIds {
                trakt: 16662,
                slug: Some("inception-2010".to_string()),
                tvdb: None,
                imdb: Some("tt1375666".to_string()),
                tmdb: Some(27205),
                tvrage: None,
            },
            runtime: None,
        }),
        show: None,
        episode: None,
    }
}

#[test]
fn test_build_payload_movie() {
    let response = create_movie_response();

    let payload = build_payload(&response, 8.5);

    assert!(payload.is_some());
    let p = payload.unwrap();
    assert_eq!(p.details, "Inception (2010)");
    assert_eq!(p.state, "8.5 stars");
    assert_eq!(p.media, "movies");
}

#[test]
fn test_build_payload_movie_imdb_link() {
    let response = create_movie_response();

    let payload = build_payload(&response, 8.5).unwrap();

    assert_eq!(payload.link_imdb, "https://www.imdb.com/title/tt1375666");
}

#[test]
fn test_build_payload_movie_trakt_link() {
    let response = create_movie_response();

    let payload = build_payload(&response, 8.5).unwrap();

    assert_eq!(payload.link_trakt, "https://trakt.tv/movies/inception-2010");
}

#[test]
fn test_build_payload_movie_rating_formatting() {
    let response = create_movie_response();

    // Test various rating values
    let payload1 = build_payload(&response, 9.12345).unwrap();
    assert_eq!(payload1.state, "9.1 stars");

    let payload2 = build_payload(&response, 7.0).unwrap();
    assert_eq!(payload2.state, "7.0 stars");
}

// ============================================================================
// Build Payload Tests - Episode
// ============================================================================

fn create_episode_response() -> TraktWatchingResponse {
    TraktWatchingResponse {
        expires_at: "2024-01-15T11:00:00.000Z".to_string(),
        started_at: "2024-01-15T10:00:00.000Z".to_string(),
        action: "watching".to_string(),
        r#type: "episode".to_string(),
        movie: None,
        show: Some(TraktShow {
            title: "Breaking Bad".to_string(),
            year: 2008,
            ids: TraktIds {
                trakt: 1388,
                slug: Some("breaking-bad".to_string()),
                tvdb: Some(81189),
                imdb: Some("tt0903747".to_string()),
                tmdb: Some(1396),
                tvrage: Some(18164),
            },
            runtime: None,
        }),
        episode: Some(TraktEpisode {
            season: 5,
            number: 16,
            title: "Felina".to_string(),
            ids: TraktIds {
                trakt: 62155,
                slug: None,
                tvdb: Some(4639461),
                imdb: Some("tt2301451".to_string()),
                tmdb: Some(62161),
                tvrage: None,
            },
            runtime: None,
        }),
    }
}

#[test]
fn test_build_payload_episode() {
    let response = create_episode_response();

    let payload = build_payload(&response, 0.0); // Rating ignored for episodes

    assert!(payload.is_some());
    let p = payload.unwrap();
    assert_eq!(p.details, "Breaking Bad");
    assert_eq!(p.state, "S05E16 - Felina");
    assert_eq!(p.media, "shows");
}

#[test]
fn test_build_payload_episode_imdb_link() {
    let response = create_episode_response();

    let payload = build_payload(&response, 0.0).unwrap();

    // Should link to show, not episode
    assert_eq!(payload.link_imdb, "https://www.imdb.com/title/tt0903747");
}

#[test]
fn test_build_payload_episode_trakt_link() {
    let response = create_episode_response();

    let payload = build_payload(&response, 0.0).unwrap();

    assert_eq!(payload.link_trakt, "https://trakt.tv/shows/breaking-bad");
}

#[test]
fn test_build_payload_episode_formatting() {
    // Test episode number formatting (leading zeros)
    let mut response = create_episode_response();
    response.episode.as_mut().unwrap().season = 1;
    response.episode.as_mut().unwrap().number = 1;
    response.episode.as_mut().unwrap().title = "Pilot".to_string();

    let payload = build_payload(&response, 0.0).unwrap();

    assert_eq!(payload.state, "S01E01 - Pilot");
}

// ============================================================================
// Build Payload Tests - Unknown Type
// ============================================================================

#[test]
fn test_build_payload_unknown_type() {
    let response = TraktWatchingResponse {
        expires_at: "2024-01-15T11:00:00.000Z".to_string(),
        started_at: "2024-01-15T10:00:00.000Z".to_string(),
        action: "watching".to_string(),
        r#type: "unknown".to_string(),
        movie: None,
        show: None,
        episode: None,
    };

    let payload = build_payload(&response, 0.0);

    assert!(payload.is_none());
}

#[test]
fn test_build_payload_movie_missing_ids() {
    let response = TraktWatchingResponse {
        expires_at: "2024-01-15T11:00:00.000Z".to_string(),
        started_at: "2024-01-15T10:00:00.000Z".to_string(),
        action: "watching".to_string(),
        r#type: "movie".to_string(),
        movie: Some(TraktMovie {
            title: "Test".to_string(),
            year: 2020,
            ids: TraktIds {
                trakt: 12345,
                slug: None, // Missing slug
                tvdb: None,
                imdb: None, // Missing imdb
                tmdb: None,
                tvrage: None,
            },
            runtime: None,
        }),
        show: None,
        episode: None,
    };

    let payload = build_payload(&response, 8.0);

    // Should return None because required IDs are missing
    assert!(payload.is_none());
}

// ============================================================================
// App ID Selection Tests
// ============================================================================

#[test]
fn test_get_app_id_for_movie() {
    let app_id = get_app_id_for_media_type("movie");
    assert_eq!(app_id, DEFAULT_DISCORD_APP_ID_MOVIE);
}

#[test]
fn test_get_app_id_for_episode() {
    let app_id = get_app_id_for_media_type("episode");
    assert_eq!(app_id, DEFAULT_DISCORD_APP_ID_SHOW);
}

#[test]
fn test_get_app_id_for_unknown_defaults_to_movie() {
    let app_id = get_app_id_for_media_type("unknown");
    assert_eq!(app_id, DEFAULT_DISCORD_APP_ID_MOVIE);
}

#[test]
fn test_get_app_id_for_empty_string_defaults_to_movie() {
    let app_id = get_app_id_for_media_type("");
    assert_eq!(app_id, DEFAULT_DISCORD_APP_ID_MOVIE);
}

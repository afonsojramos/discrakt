// Tests for Discord module in src/discord.rs

mod common;

use common::watching::{episode_watching, movie_watching};
use discrakt::discord::{app_id_for_kind, build_payload, Payload};
use discrakt::source::MediaKind;
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
    assert_eq!(payload.large_image, "");
    assert!(payload.buttons.is_empty());
}

#[test]
fn test_payload_clone() {
    let payload = Payload {
        details: "Test Details".to_string(),
        state: "Test State".to_string(),
        media: "movies".to_string(),
        large_image: "https://image.test/img.jpg".to_string(),
        buttons: vec![("IMDB".to_string(), "https://imdb.com/test".to_string())],
    };

    let cloned = payload.clone();
    assert_eq!(cloned.details, payload.details);
    assert_eq!(cloned.state, payload.state);
    assert_eq!(cloned.media, payload.media);
    assert_eq!(cloned.buttons, payload.buttons);
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

#[test]
fn test_build_payload_movie() {
    let payload = build_payload(&movie_watching());

    assert_eq!(payload.details, "Inception (2010)");
    assert_eq!(payload.state, "8.5 ⭐️");
    assert_eq!(payload.media, "movies");
}

#[test]
fn test_build_payload_movie_tmdb_button() {
    let payload = build_payload(&movie_watching());

    assert!(payload.buttons.contains(&(
        "TMDB".to_string(),
        "https://www.themoviedb.org/movie/27205".to_string()
    )));
}

#[test]
fn test_build_payload_movie_imdb_fallback_when_no_tmdb() {
    let mut watching = movie_watching();
    watching.ids.tmdb = None;

    let payload = build_payload(&watching);

    assert_eq!(
        payload.buttons.first(),
        Some(&(
            "IMDB".to_string(),
            "https://www.imdb.com/title/tt1375666".to_string()
        ))
    );
}

#[test]
fn test_build_payload_movie_project_button_is_secondary() {
    let payload = build_payload(&movie_watching());

    assert_eq!(
        payload.buttons.last(),
        Some(&(
            "Discrakt".to_string(),
            env!("CARGO_PKG_REPOSITORY").to_string()
        ))
    );
}

#[test]
fn test_build_payload_movie_uses_resolved_poster() {
    let payload = build_payload(&movie_watching());

    assert_eq!(
        payload.large_image,
        "https://image.tmdb.org/t/p/w600_and_h600_bestv2/poster.jpg"
    );
}

#[test]
fn test_build_payload_movie_rating_formatting() {
    let mut watching = movie_watching();

    watching.rating = Some(9.12345);
    assert_eq!(build_payload(&watching).state, "9.1 ⭐️");

    watching.rating = Some(7.0);
    assert_eq!(build_payload(&watching).state, "7.0 ⭐️");
}

#[test]
fn test_build_payload_movie_without_rating_hides_state() {
    let mut watching = movie_watching();
    watching.rating = None;

    assert_eq!(build_payload(&watching).state, "");
}

// ============================================================================
// Build Payload Tests - Episode
// ============================================================================

#[test]
fn test_build_payload_episode() {
    let payload = build_payload(&episode_watching());

    assert_eq!(payload.details, "Breaking Bad");
    assert_eq!(payload.state, "S05E16 - Felina");
    assert_eq!(payload.media, "shows");
}

#[test]
fn test_build_payload_episode_tmdb_button() {
    let payload = build_payload(&episode_watching());

    // Should link to the show page (tv), not the episode.
    assert!(payload.buttons.contains(&(
        "TMDB".to_string(),
        "https://www.themoviedb.org/tv/1396".to_string()
    )));
}

#[test]
fn test_build_payload_episode_project_button_is_secondary() {
    let payload = build_payload(&episode_watching());

    assert_eq!(
        payload.buttons.last(),
        Some(&(
            "Discrakt".to_string(),
            env!("CARGO_PKG_REPOSITORY").to_string()
        ))
    );
}

#[test]
fn test_build_payload_episode_without_poster_falls_back_to_media() {
    // The episode fixture has no resolved poster.
    let payload = build_payload(&episode_watching());

    assert_eq!(payload.large_image, "shows");
}

#[test]
fn test_build_payload_episode_formatting() {
    // Test episode number formatting (leading zeros)
    let mut watching = episode_watching();
    watching.season = Some(1);
    watching.episode_number = Some(1);
    watching.episode_title = Some("Pilot".to_string());

    let payload = build_payload(&watching);

    assert_eq!(payload.state, "S01E01 - Pilot");
}

// ============================================================================
// Build Payload Tests - Missing IDs
// ============================================================================

#[test]
fn test_build_payload_movie_without_metadata_ids_keeps_project_button() {
    let mut watching = movie_watching();
    watching.ids.tmdb = None;
    watching.imdb_url = None;

    let payload = build_payload(&watching);

    // Still renders; the project button is always present even with no metadata ids.
    assert_eq!(payload.details, "Inception (2010)");
    assert_eq!(
        payload.buttons,
        vec![(
            "Discrakt".to_string(),
            env!("CARGO_PKG_REPOSITORY").to_string()
        )]
    );
}

// ============================================================================
// App ID Selection Tests
// ============================================================================

#[test]
fn test_app_id_for_movie() {
    assert_eq!(
        app_id_for_kind(MediaKind::Movie),
        DEFAULT_DISCORD_APP_ID_MOVIE
    );
}

#[test]
fn test_app_id_for_episode() {
    assert_eq!(
        app_id_for_kind(MediaKind::Episode),
        DEFAULT_DISCORD_APP_ID_SHOW
    );
}

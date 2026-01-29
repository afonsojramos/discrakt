// Tests for utils module in src/utils.rs

mod common;

use chrono::{DateTime, Duration};
use discrakt::trakt::TraktWatchingResponse;
#[cfg(target_os = "macos")]
use discrakt::utils::is_light_mode;
use discrakt::utils::{
    create_dark_icon, get_watch_stats, poll_device_token, request_device_code, user_agent,
    DeviceTokenPollResult, MediaType, TraktAccessToken, TraktDeviceCode, DEFAULT_DISCORD_APP_ID,
    DEFAULT_DISCORD_APP_ID_MOVIE, DEFAULT_DISCORD_APP_ID_SHOW, DEFAULT_TMDB_TOKEN,
    DEFAULT_TRAKT_CLIENT_ID, LANGUAGES,
};
use image::RgbaImage;

// ============================================================================
// User Agent Tests
// ============================================================================

#[test]
fn test_user_agent_format() {
    let ua = user_agent();

    // Should contain package name and version
    assert!(ua.contains("discrakt"));
    assert!(ua.contains("/"));
}

#[test]
fn test_user_agent_is_static() {
    // Should return the same reference each time (OnceLock)
    let ua1 = user_agent();
    let ua2 = user_agent();
    assert_eq!(ua1, ua2);
}

// ============================================================================
// MediaType Tests
// ============================================================================

#[test]
fn test_media_type_movie_as_str() {
    let media_type = MediaType::Movie;
    assert_eq!(media_type.as_str(), "movie");
}

#[test]
fn test_media_type_show_as_str() {
    let media_type = MediaType::Show;
    assert_eq!(media_type.as_str(), "episode");
}

// ============================================================================
// Watch Stats Tests
// ============================================================================

#[test]
fn test_get_watch_stats_calculation() {
    // Create a response with known timestamps
    // Started at 10:00, expires at 12:00 (2 hours)
    let response: TraktWatchingResponse =
        serde_json::from_str(common::fixtures::TRAKT_MOVIE_WATCHING).unwrap();

    let stats = get_watch_stats(&response);

    // The percentage depends on current time, but we can verify the dates are parsed
    assert!(!stats.watch_percentage.is_empty());
    assert!(stats.watch_percentage.ends_with('%'));
}

#[test]
fn test_get_watch_stats_dates_parsed() {
    let response: TraktWatchingResponse =
        serde_json::from_str(common::fixtures::TRAKT_MOVIE_WATCHING).unwrap();

    let stats = get_watch_stats(&response);

    // Verify dates are valid (not default)
    assert!(stats.start_date.timestamp() > 0);
    assert!(stats.end_date.timestamp() > stats.start_date.timestamp());
}

#[test]
fn test_get_watch_stats_uses_runtime_over_stale_start() {
    let response: TraktWatchingResponse =
        serde_json::from_str(common::fixtures::TRAKT_EPISODE_WATCHING_STALE_START).unwrap();

    let stats = get_watch_stats(&response);

    let end_date =
        DateTime::parse_from_rfc3339("2024-01-15T11:00:00.000Z").expect("valid end date");
    let expected_start = end_date - Duration::minutes(44);

    assert_eq!(stats.end_date.timestamp(), end_date.timestamp());
    assert_eq!(stats.start_date.timestamp(), expected_start.timestamp());
}

// ============================================================================
// Constants Tests
// ============================================================================

#[test]
fn test_default_trakt_client_id() {
    assert!(!DEFAULT_TRAKT_CLIENT_ID.is_empty());
    assert!(DEFAULT_TRAKT_CLIENT_ID.len() > 10);
}

#[test]
fn test_default_discord_app_ids() {
    assert!(!DEFAULT_DISCORD_APP_ID_MOVIE.is_empty());
    assert!(!DEFAULT_DISCORD_APP_ID_SHOW.is_empty());
    assert_ne!(DEFAULT_DISCORD_APP_ID_MOVIE, DEFAULT_DISCORD_APP_ID_SHOW);
}

#[test]
fn test_default_discord_app_id_is_movie() {
    // Default should be the movie app ID
    assert_eq!(DEFAULT_DISCORD_APP_ID, DEFAULT_DISCORD_APP_ID_MOVIE);
}

#[test]
fn test_default_tmdb_token() {
    assert!(!DEFAULT_TMDB_TOKEN.is_empty());
}

// ============================================================================
// TraktDeviceCode Deserialization Tests
// ============================================================================

#[test]
fn test_trakt_device_code_deserialization() {
    let device_code: TraktDeviceCode =
        serde_json::from_str(common::fixtures::TRAKT_DEVICE_CODE).unwrap();

    assert_eq!(device_code.device_code, "abc123def456");
    assert_eq!(device_code.user_code, "ABCD1234");
    assert_eq!(device_code.verification_url, "https://trakt.tv/activate");
    assert_eq!(device_code.expires_in, 600);
    assert_eq!(device_code.interval, 5);
}

// ============================================================================
// TraktAccessToken Deserialization Tests
// ============================================================================

#[test]
fn test_trakt_access_token_deserialization() {
    let token: TraktAccessToken =
        serde_json::from_str(common::fixtures::TRAKT_ACCESS_TOKEN).unwrap();

    assert_eq!(token.access_token, "access_token_value");
    assert_eq!(token.token_type, "Bearer");
    assert_eq!(token.expires_in, 7776000);
    assert_eq!(token.refresh_token, "refresh_token_value");
    assert_eq!(token.scope, "public");
    assert_eq!(token.created_at, 1705312800);
}

// ============================================================================
// OAuth Device Code Request Tests (with mocking)
// ============================================================================

#[test]
fn test_request_device_code_success() {
    let mut server = mockito::Server::new();

    let mock = server
        .mock("POST", "/oauth/device/code")
        .match_header("content-type", "application/json")
        .with_status(200)
        .with_body(common::fixtures::TRAKT_DEVICE_CODE)
        .create();

    let result = request_device_code("test_client_id", Some(&server.url()));

    mock.assert();
    assert!(result.is_ok());

    let device_code = result.unwrap();
    assert_eq!(device_code.user_code, "ABCD1234");
    assert_eq!(device_code.verification_url, "https://trakt.tv/activate");
}

#[test]
fn test_request_device_code_network_error() {
    // Use an invalid URL to simulate network error
    let result = request_device_code(
        "test_client_id",
        Some("http://invalid.invalid.invalid:9999"),
    );

    assert!(result.is_err());
    assert!(result.unwrap_err().contains("error"));
}

#[test]
fn test_request_device_code_http_error() {
    let mut server = mockito::Server::new();

    let mock = server
        .mock("POST", "/oauth/device/code")
        .with_status(500)
        .create();

    let result = request_device_code("test_client_id", Some(&server.url()));

    mock.assert();
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("HTTP 500"));
}

// ============================================================================
// OAuth Device Token Polling Tests (with mocking)
// ============================================================================

#[test]
fn test_poll_device_token_success() {
    let mut server = mockito::Server::new();

    let mock = server
        .mock("POST", "/oauth/device/token")
        .match_header("content-type", "application/json")
        .with_status(200)
        .with_body(common::fixtures::TRAKT_ACCESS_TOKEN)
        .create();

    let result = poll_device_token("test_client_id", "device_code_123", Some(&server.url()));

    mock.assert();
    match result {
        DeviceTokenPollResult::Success(token) => {
            assert_eq!(token.access_token, "access_token_value");
        }
        _ => panic!("Expected Success, got {:?}", result),
    }
}

#[test]
fn test_poll_device_token_pending() {
    let mut server = mockito::Server::new();

    let mock = server
        .mock("POST", "/oauth/device/token")
        .with_status(400) // Pending authorization
        .create();

    let result = poll_device_token("test_client_id", "device_code_123", Some(&server.url()));

    mock.assert();
    assert!(matches!(result, DeviceTokenPollResult::Pending));
}

#[test]
fn test_poll_device_token_invalid_code() {
    let mut server = mockito::Server::new();

    let mock = server
        .mock("POST", "/oauth/device/token")
        .with_status(404) // Invalid device code
        .create();

    let result = poll_device_token("test_client_id", "invalid_code", Some(&server.url()));

    mock.assert();
    assert!(matches!(result, DeviceTokenPollResult::InvalidCode));
}

#[test]
fn test_poll_device_token_already_used() {
    let mut server = mockito::Server::new();

    let mock = server
        .mock("POST", "/oauth/device/token")
        .with_status(409) // Code already used
        .create();

    let result = poll_device_token("test_client_id", "used_code", Some(&server.url()));

    mock.assert();
    assert!(matches!(result, DeviceTokenPollResult::AlreadyUsed));
}

#[test]
fn test_poll_device_token_expired() {
    let mut server = mockito::Server::new();

    let mock = server
        .mock("POST", "/oauth/device/token")
        .with_status(410) // Device code expired
        .create();

    let result = poll_device_token("test_client_id", "expired_code", Some(&server.url()));

    mock.assert();
    assert!(matches!(result, DeviceTokenPollResult::Expired));
}

#[test]
fn test_poll_device_token_denied() {
    let mut server = mockito::Server::new();

    let mock = server
        .mock("POST", "/oauth/device/token")
        .with_status(418) // User denied authorization
        .create();

    let result = poll_device_token("test_client_id", "denied_code", Some(&server.url()));

    mock.assert();
    assert!(matches!(result, DeviceTokenPollResult::Denied));
}

#[test]
fn test_poll_device_token_slow_down() {
    let mut server = mockito::Server::new();

    let mock = server
        .mock("POST", "/oauth/device/token")
        .with_status(429) // Rate limited
        .create();

    let result = poll_device_token("test_client_id", "code", Some(&server.url()));

    mock.assert();
    assert!(matches!(result, DeviceTokenPollResult::SlowDown));
}

#[test]
fn test_poll_device_token_other_http_error() {
    let mut server = mockito::Server::new();

    let mock = server
        .mock("POST", "/oauth/device/token")
        .with_status(500) // Server error
        .create();

    let result = poll_device_token("test_client_id", "code", Some(&server.url()));

    mock.assert();
    match result {
        DeviceTokenPollResult::Error(msg) => {
            assert!(msg.contains("HTTP 500"));
        }
        _ => panic!("Expected Error, got {:?}", result),
    }
}

// ============================================================================
// Theme Detection Tests
// ============================================================================

// Note: is_light_mode() uses the dark-light crate which on Linux requires
// D-Bus via zbus, which needs a Tokio runtime. Only test on macOS where
// it works without async runtime.
#[cfg(target_os = "macos")]
#[test]
fn test_is_light_mode_returns_bool() {
    // Just verify it doesn't panic and returns a bool
    let _ = is_light_mode();
}

// ============================================================================
// Icon Inversion Tests
// ============================================================================

#[test]
fn test_create_dark_icon_inverts_rgb() {
    // Create a simple 2x2 image
    let mut image = RgbaImage::new(2, 2);

    // Set first pixel to white (255, 255, 255, 255)
    image.put_pixel(0, 0, image::Rgba([255, 255, 255, 255]));
    // Set second pixel to black (0, 0, 0, 255)
    image.put_pixel(1, 0, image::Rgba([0, 0, 0, 255]));
    // Set third pixel with partial values (100, 150, 200, 128)
    image.put_pixel(0, 1, image::Rgba([100, 150, 200, 128]));

    let dark = create_dark_icon(&image);

    // White should become black
    let pixel1 = dark.get_pixel(0, 0);
    assert_eq!(pixel1[0], 0); // R
    assert_eq!(pixel1[1], 0); // G
    assert_eq!(pixel1[2], 0); // B
    assert_eq!(pixel1[3], 255); // A preserved

    // Black should become white
    let pixel2 = dark.get_pixel(1, 0);
    assert_eq!(pixel2[0], 255); // R
    assert_eq!(pixel2[1], 255); // G
    assert_eq!(pixel2[2], 255); // B
    assert_eq!(pixel2[3], 255); // A preserved

    // Partial values should be inverted
    let pixel3 = dark.get_pixel(0, 1);
    assert_eq!(pixel3[0], 155); // 255 - 100
    assert_eq!(pixel3[1], 105); // 255 - 150
    assert_eq!(pixel3[2], 55); // 255 - 200
    assert_eq!(pixel3[3], 128); // A preserved
}

#[test]
fn test_create_dark_icon_preserves_alpha() {
    let mut image = RgbaImage::new(1, 1);
    image.put_pixel(0, 0, image::Rgba([128, 128, 128, 0])); // Fully transparent

    let dark = create_dark_icon(&image);

    let pixel = dark.get_pixel(0, 0);
    assert_eq!(pixel[3], 0); // Alpha should remain 0
}

// ============================================================================
// Language Constants Tests
// ============================================================================

#[test]
fn test_languages_constant_not_empty() {
    assert!(!LANGUAGES.is_empty());
}

#[test]
fn test_languages_contains_english() {
    let english = LANGUAGES.iter().find(|(_, code)| *code == "en-US");
    assert!(english.is_some());
    assert_eq!(english.unwrap().0, "English");
}

#[test]
fn test_languages_format_valid() {
    for (name, code) in LANGUAGES {
        // Display name should not be empty
        assert!(!name.is_empty(), "Display name should not be empty");

        // Language code should follow xx-YY format
        assert!(
            code.contains('-'),
            "Language code '{}' should contain a hyphen",
            code
        );

        let parts: Vec<&str> = code.split('-').collect();
        assert_eq!(
            parts.len(),
            2,
            "Language code '{}' should have exactly 2 parts",
            code
        );

        // First part should be lowercase (language code)
        assert!(
            parts[0].chars().all(|c| c.is_ascii_lowercase()),
            "Language part of '{}' should be lowercase",
            code
        );

        // Second part should be uppercase (country code)
        assert!(
            parts[1].chars().all(|c| c.is_ascii_uppercase()),
            "Country part of '{}' should be uppercase",
            code
        );
    }
}

#[test]
fn test_languages_no_duplicates() {
    let mut codes: Vec<&str> = LANGUAGES.iter().map(|(_, code)| *code).collect();
    let original_len = codes.len();
    codes.sort();
    codes.dedup();
    assert_eq!(
        codes.len(),
        original_len,
        "LANGUAGES should not contain duplicate codes"
    );
}

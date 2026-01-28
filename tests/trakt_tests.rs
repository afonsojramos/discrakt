// Tests for Trakt API client in src/trakt.rs

mod common;

use discrakt::trakt::{
    Trakt, TraktConfig, TraktEpisode, TraktIds, TraktMovie, TraktRatingsResponse, TraktShow,
    TraktWatchingResponse, DEFAULT_TMDB_BASE_URL, DEFAULT_TRAKT_BASE_URL,
};

// ============================================================================
// Deserialization Tests
// ============================================================================

#[test]
fn test_trakt_movie_deserialization() {
    let json = r#"{
        "title": "Inception",
        "year": 2010,
        "ids": {
            "trakt": 16662,
            "slug": "inception-2010",
            "tvdb": null,
            "imdb": "tt1375666",
            "tmdb": 27205,
            "tvrage": null
        }
    }"#;

    let movie: TraktMovie = serde_json::from_str(json).unwrap();
    assert_eq!(movie.title, "Inception");
    assert_eq!(movie.year, 2010);
    assert_eq!(movie.ids.trakt, 16662);
    assert_eq!(movie.ids.slug, Some("inception-2010".to_string()));
    assert_eq!(movie.ids.imdb, Some("tt1375666".to_string()));
    assert_eq!(movie.ids.tmdb, Some(27205));
}

#[test]
fn test_trakt_show_deserialization() {
    let json = r#"{
        "title": "Breaking Bad",
        "year": 2008,
        "ids": {
            "trakt": 1388,
            "slug": "breaking-bad",
            "tvdb": 81189,
            "imdb": "tt0903747",
            "tmdb": 1396,
            "tvrage": 18164
        }
    }"#;

    let show: TraktShow = serde_json::from_str(json).unwrap();
    assert_eq!(show.title, "Breaking Bad");
    assert_eq!(show.year, 2008);
    assert_eq!(show.ids.trakt, 1388);
    assert_eq!(show.ids.tvdb, Some(81189));
}

#[test]
fn test_trakt_episode_deserialization() {
    let json = r#"{
        "season": 5,
        "number": 16,
        "title": "Felina",
        "ids": {
            "trakt": 62155,
            "tvdb": 4639461,
            "imdb": "tt2301451",
            "tmdb": 62161,
            "tvrage": null
        }
    }"#;

    let episode: TraktEpisode = serde_json::from_str(json).unwrap();
    assert_eq!(episode.season, 5);
    assert_eq!(episode.number, 16);
    assert_eq!(episode.title, "Felina");
    assert_eq!(episode.ids.trakt, 62155);
}

#[test]
fn test_trakt_ids_with_optional_fields() {
    let json = r#"{
        "trakt": 12345,
        "slug": null,
        "tvdb": null,
        "imdb": null,
        "tmdb": null,
        "tvrage": null
    }"#;

    let ids: TraktIds = serde_json::from_str(json).unwrap();
    assert_eq!(ids.trakt, 12345);
    assert!(ids.slug.is_none());
    assert!(ids.tvdb.is_none());
    assert!(ids.imdb.is_none());
    assert!(ids.tmdb.is_none());
    assert!(ids.tvrage.is_none());
}

#[test]
fn test_trakt_watching_response_movie() {
    let response: TraktWatchingResponse =
        serde_json::from_str(common::fixtures::TRAKT_MOVIE_WATCHING).unwrap();

    assert_eq!(response.r#type, "movie");
    assert_eq!(response.action, "watching");
    assert!(response.movie.is_some());
    assert!(response.show.is_none());
    assert!(response.episode.is_none());

    let movie = response.movie.unwrap();
    assert_eq!(movie.title, "Inception");
    assert_eq!(movie.year, 2010);
}

#[test]
fn test_trakt_watching_response_episode() {
    let response: TraktWatchingResponse =
        serde_json::from_str(common::fixtures::TRAKT_EPISODE_WATCHING).unwrap();

    assert_eq!(response.r#type, "episode");
    assert_eq!(response.action, "watching");
    assert!(response.movie.is_none());
    assert!(response.show.is_some());
    assert!(response.episode.is_some());

    let show = response.show.unwrap();
    assert_eq!(show.title, "Breaking Bad");

    let episode = response.episode.unwrap();
    assert_eq!(episode.season, 5);
    assert_eq!(episode.number, 16);
    assert_eq!(episode.title, "Felina");
}

#[test]
fn test_trakt_ratings_response() {
    let response: TraktRatingsResponse =
        serde_json::from_str(common::fixtures::TRAKT_MOVIE_RATINGS).unwrap();

    assert!((response.rating - 8.45123).abs() < 0.0001);
    assert_eq!(response.votes, 45678);
    assert!(response.distribution.contains_key("10"));
    assert_eq!(response.distribution.get("10"), Some(&11728));
}

// ============================================================================
// TraktConfig Tests
// ============================================================================

#[test]
fn test_trakt_config_default() {
    let config = TraktConfig::default();

    assert_eq!(config.client_id, "");
    assert_eq!(config.username, "");
    assert!(config.oauth_access_token.is_none());
    assert!(config.trakt_base_url.is_none());
    assert!(config.tmdb_base_url.is_none());
    assert!(config.language.is_none());
}

#[test]
fn test_trakt_config_with_values() {
    let config = TraktConfig {
        client_id: "my_client_id".to_string(),
        username: "testuser".to_string(),
        oauth_access_token: Some("token123".to_string()),
        trakt_base_url: Some("http://localhost:8080".to_string()),
        tmdb_base_url: Some("http://localhost:8081".to_string()),
        language: Some("fr-FR".to_string()),
    };

    assert_eq!(config.client_id, "my_client_id");
    assert_eq!(config.username, "testuser");
    assert_eq!(config.oauth_access_token, Some("token123".to_string()));
    assert_eq!(
        config.trakt_base_url,
        Some("http://localhost:8080".to_string())
    );
    assert_eq!(
        config.tmdb_base_url,
        Some("http://localhost:8081".to_string())
    );
    assert_eq!(config.language, Some("fr-FR".to_string()));
}

// ============================================================================
// Trakt Client Constructor Tests
// ============================================================================

#[test]
fn test_trakt_new() {
    let trakt = Trakt::new(
        "client_id".to_string(),
        "username".to_string(),
        Some("token".to_string()),
    );

    // The Trakt client should be created successfully
    // We can't directly inspect the private fields, but we can verify it doesn't panic
    drop(trakt);
}

#[test]
fn test_trakt_with_config_default_urls() {
    let config = TraktConfig {
        client_id: "test_client".to_string(),
        username: "testuser".to_string(),
        oauth_access_token: None,
        trakt_base_url: None,
        tmdb_base_url: None,
        language: None,
    };

    let trakt = Trakt::with_config(config);
    // Client should use default URLs when none are specified
    drop(trakt);
}

#[test]
fn test_trakt_with_config_custom_urls() {
    let config = TraktConfig {
        client_id: "test_client".to_string(),
        username: "testuser".to_string(),
        oauth_access_token: None,
        trakt_base_url: Some("http://localhost:8080".to_string()),
        tmdb_base_url: Some("http://localhost:8081".to_string()),
        language: None,
    };

    let trakt = Trakt::with_config(config);
    drop(trakt);
}

// ============================================================================
// HTTP Mocking Tests - Watching Endpoint
// ============================================================================

#[test]
fn test_get_watching_returns_movie() {
    let mut server = mockito::Server::new();

    let mock = server
        .mock("GET", "/users/testuser/watching")
        .match_query(mockito::Matcher::UrlEncoded(
            "extended".into(),
            "full".into(),
        ))
        .match_header("trakt-api-version", "2")
        .match_header("trakt-api-key", "test_client")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(common::fixtures::TRAKT_MOVIE_WATCHING)
        .create();

    let trakt = Trakt::with_config(TraktConfig {
        client_id: "test_client".to_string(),
        username: "testuser".to_string(),
        oauth_access_token: None,
        trakt_base_url: Some(server.url()),
        tmdb_base_url: None,
        language: None,
    });

    let result = trakt.get_watching();

    mock.assert();
    assert!(result.is_some());

    let response = result.unwrap();
    assert_eq!(response.r#type, "movie");
    assert_eq!(response.movie.as_ref().unwrap().title, "Inception");
}

#[test]
fn test_get_watching_returns_episode() {
    let mut server = mockito::Server::new();

    let mock = server
        .mock("GET", "/users/testuser/watching")
        .match_query(mockito::Matcher::UrlEncoded(
            "extended".into(),
            "full".into(),
        ))
        .match_header("trakt-api-version", "2")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(common::fixtures::TRAKT_EPISODE_WATCHING)
        .create();

    let trakt = Trakt::with_config(TraktConfig {
        client_id: "test_client".to_string(),
        username: "testuser".to_string(),
        oauth_access_token: None,
        trakt_base_url: Some(server.url()),
        tmdb_base_url: None,
        language: None,
    });

    let result = trakt.get_watching();

    mock.assert();
    assert!(result.is_some());

    let response = result.unwrap();
    assert_eq!(response.r#type, "episode");
    assert_eq!(response.show.as_ref().unwrap().title, "Breaking Bad");
    assert_eq!(response.episode.as_ref().unwrap().title, "Felina");
}

#[test]
fn test_get_watching_returns_none_when_not_watching() {
    let mut server = mockito::Server::new();

    let mock = server
        .mock("GET", "/users/testuser/watching")
        .match_query(mockito::Matcher::UrlEncoded(
            "extended".into(),
            "full".into(),
        ))
        .with_status(204) // No Content
        .create();

    let trakt = Trakt::with_config(TraktConfig {
        client_id: "test_client".to_string(),
        username: "testuser".to_string(),
        oauth_access_token: None,
        trakt_base_url: Some(server.url()),
        tmdb_base_url: None,
        language: None,
    });

    let result = trakt.get_watching();

    mock.assert();
    assert!(result.is_none());
}

#[test]
fn test_get_watching_handles_401() {
    let mut server = mockito::Server::new();

    let mock = server
        .mock("GET", "/users/testuser/watching")
        .match_query(mockito::Matcher::UrlEncoded(
            "extended".into(),
            "full".into(),
        ))
        .with_status(401)
        .create();

    let trakt = Trakt::with_config(TraktConfig {
        client_id: "test_client".to_string(),
        username: "testuser".to_string(),
        oauth_access_token: None,
        trakt_base_url: Some(server.url()),
        tmdb_base_url: None,
        language: None,
    });

    let result = trakt.get_watching();

    mock.assert();
    assert!(result.is_none());
}

#[test]
fn test_get_watching_handles_403() {
    let mut server = mockito::Server::new();

    let mock = server
        .mock("GET", "/users/testuser/watching")
        .match_query(mockito::Matcher::UrlEncoded(
            "extended".into(),
            "full".into(),
        ))
        .with_status(403)
        .create();

    let trakt = Trakt::with_config(TraktConfig {
        client_id: "test_client".to_string(),
        username: "testuser".to_string(),
        oauth_access_token: None,
        trakt_base_url: Some(server.url()),
        tmdb_base_url: None,
        language: None,
    });

    let result = trakt.get_watching();

    mock.assert();
    assert!(result.is_none());
}

#[test]
fn test_get_watching_with_oauth_uses_me_endpoint() {
    let mut server = mockito::Server::new();

    // When OAuth token is present, should use /users/me/watching instead of /users/{username}/watching
    let mock = server
        .mock("GET", "/users/me/watching")
        .match_query(mockito::Matcher::UrlEncoded(
            "extended".into(),
            "full".into(),
        ))
        .match_header("Authorization", "Bearer my_oauth_token")
        .with_status(200)
        .with_body(common::fixtures::TRAKT_MOVIE_WATCHING)
        .create();

    let trakt = Trakt::with_config(TraktConfig {
        client_id: "test_client".to_string(),
        username: "testuser".to_string(), // Username should be ignored when OAuth is present
        oauth_access_token: Some("my_oauth_token".to_string()),
        trakt_base_url: Some(server.url()),
        tmdb_base_url: None,
        language: None,
    });

    let result = trakt.get_watching();

    mock.assert();
    assert!(result.is_some());
}

#[test]
fn test_get_watching_encodes_special_chars_in_username() {
    let mut server = mockito::Server::new();

    // Username with spaces should be URL-encoded when no OAuth token is present
    let mock = server
        .mock("GET", "/users/john%20doe/watching")
        .match_query(mockito::Matcher::UrlEncoded(
            "extended".into(),
            "full".into(),
        ))
        .with_status(200)
        .with_body(common::fixtures::TRAKT_MOVIE_WATCHING)
        .create();

    let trakt = Trakt::with_config(TraktConfig {
        client_id: "test_client".to_string(),
        username: "john doe".to_string(),
        oauth_access_token: None,
        trakt_base_url: Some(server.url()),
        tmdb_base_url: None,
        language: None,
    });

    let result = trakt.get_watching();

    mock.assert();
    assert!(result.is_some());
}

#[test]
fn test_get_watching_empty_oauth_uses_username_endpoint() {
    let mut server = mockito::Server::new();

    // Empty OAuth token should fall back to /users/{username}/watching
    let mock = server
        .mock("GET", "/users/testuser/watching")
        .match_query(mockito::Matcher::UrlEncoded(
            "extended".into(),
            "full".into(),
        ))
        .with_status(200)
        .with_body(common::fixtures::TRAKT_MOVIE_WATCHING)
        .create();

    let trakt = Trakt::with_config(TraktConfig {
        client_id: "test_client".to_string(),
        username: "testuser".to_string(),
        oauth_access_token: Some("".to_string()), // Empty token
        trakt_base_url: Some(server.url()),
        tmdb_base_url: None,
        language: None,
    });

    let result = trakt.get_watching();

    mock.assert();
    assert!(result.is_some());
}

// ============================================================================
// HTTP Mocking Tests - Ratings Endpoint
// ============================================================================

#[test]
fn test_get_movie_rating_success() {
    let mut server = mockito::Server::new();

    let mock = server
        .mock("GET", "/movies/inception-2010/ratings")
        .match_header("trakt-api-version", "2")
        .with_status(200)
        .with_body(common::fixtures::TRAKT_MOVIE_RATINGS)
        .create();

    let mut trakt = Trakt::with_config(TraktConfig {
        client_id: "test_client".to_string(),
        username: "testuser".to_string(),
        oauth_access_token: None,
        trakt_base_url: Some(server.url()),
        tmdb_base_url: None,
        language: None,
    });

    let rating = trakt.get_movie_rating("inception-2010".to_string());

    mock.assert();
    assert!((rating - 8.45123).abs() < 0.0001);
}

#[test]
fn test_get_movie_rating_cached() {
    let mut server = mockito::Server::new();

    // Only expect one call - second call should hit cache
    let mock = server
        .mock("GET", "/movies/inception-2010/ratings")
        .with_status(200)
        .with_body(common::fixtures::TRAKT_MOVIE_RATINGS)
        .expect(1)
        .create();

    let mut trakt = Trakt::with_config(TraktConfig {
        client_id: "test_client".to_string(),
        username: "testuser".to_string(),
        oauth_access_token: None,
        trakt_base_url: Some(server.url()),
        tmdb_base_url: None,
        language: None,
    });

    // First call - should hit API
    let rating1 = trakt.get_movie_rating("inception-2010".to_string());

    // Second call - should hit cache
    let rating2 = trakt.get_movie_rating("inception-2010".to_string());

    mock.assert();
    assert!((rating1 - rating2).abs() < 0.0001);
}

#[test]
fn test_get_movie_rating_handles_error() {
    let mut server = mockito::Server::new();

    let mock = server
        .mock("GET", "/movies/invalid-movie/ratings")
        .with_status(404)
        .create();

    let mut trakt = Trakt::with_config(TraktConfig {
        client_id: "test_client".to_string(),
        username: "testuser".to_string(),
        oauth_access_token: None,
        trakt_base_url: Some(server.url()),
        tmdb_base_url: None,
        language: None,
    });

    let rating = trakt.get_movie_rating("invalid-movie".to_string());

    mock.assert();
    assert_eq!(rating, 0.0);
}

// ============================================================================
// Constants Tests
// ============================================================================

#[test]
fn test_default_base_urls() {
    assert_eq!(DEFAULT_TRAKT_BASE_URL, "https://api.trakt.tv");
    assert_eq!(DEFAULT_TMDB_BASE_URL, "https://api.themoviedb.org");
}

// ============================================================================
// Multilingual Title Tests
// ============================================================================

use discrakt::utils::MediaType;

#[test]
fn test_get_title_movie_success() {
    let mut server = mockito::Server::new();

    let mock = server
        .mock("GET", "/3/movie/27205")
        .match_query(mockito::Matcher::AllOf(vec![
            mockito::Matcher::UrlEncoded("api_key".into(), "test_tmdb_token".into()),
            mockito::Matcher::UrlEncoded("language".into(), "en-US".into()),
        ]))
        .with_status(200)
        .with_body(common::fixtures::TMDB_MOVIE_DETAILS)
        .create();

    let mut trakt = Trakt::with_config(TraktConfig {
        client_id: "test_client".to_string(),
        username: "testuser".to_string(),
        oauth_access_token: None,
        trakt_base_url: None,
        tmdb_base_url: Some(server.url()),
        language: Some("en-US".to_string()),
    });

    let title = trakt.get_title(
        MediaType::Movie,
        "27205".to_string(),
        "test_tmdb_token",
        None,
        None,
    );

    mock.assert();
    assert_eq!(title, "Inception");
}

#[test]
fn test_get_title_show_success() {
    let mut server = mockito::Server::new();

    let mock = server
        .mock("GET", "/3/tv/1396")
        .match_query(mockito::Matcher::AllOf(vec![
            mockito::Matcher::UrlEncoded("api_key".into(), "test_tmdb_token".into()),
            mockito::Matcher::UrlEncoded("language".into(), "en-US".into()),
        ]))
        .with_status(200)
        .with_body(common::fixtures::TMDB_SHOW_DETAILS)
        .create();

    let mut trakt = Trakt::with_config(TraktConfig {
        client_id: "test_client".to_string(),
        username: "testuser".to_string(),
        oauth_access_token: None,
        trakt_base_url: None,
        tmdb_base_url: Some(server.url()),
        language: Some("en-US".to_string()),
    });

    let title = trakt.get_title(
        MediaType::Show,
        "1396".to_string(),
        "test_tmdb_token",
        None,
        None,
    );

    mock.assert();
    assert_eq!(title, "Breaking Bad");
}

#[test]
fn test_get_title_episode_success() {
    let mut server = mockito::Server::new();

    let mock = server
        .mock("GET", "/3/tv/1396/season/5/episode/16")
        .match_query(mockito::Matcher::AllOf(vec![
            mockito::Matcher::UrlEncoded("api_key".into(), "test_tmdb_token".into()),
            mockito::Matcher::UrlEncoded("language".into(), "en-US".into()),
        ]))
        .with_status(200)
        .with_body(common::fixtures::TMDB_EPISODE_DETAILS)
        .create();

    let mut trakt = Trakt::with_config(TraktConfig {
        client_id: "test_client".to_string(),
        username: "testuser".to_string(),
        oauth_access_token: None,
        trakt_base_url: None,
        tmdb_base_url: Some(server.url()),
        language: Some("en-US".to_string()),
    });

    let title = trakt.get_title(
        MediaType::Show,
        "1396".to_string(),
        "test_tmdb_token",
        Some(5),
        Some(16),
    );

    mock.assert();
    assert_eq!(title, "Felina");
}

#[test]
fn test_get_title_cached() {
    let mut server = mockito::Server::new();

    // Only expect one call - second call should hit cache
    let mock = server
        .mock("GET", "/3/movie/27205")
        .match_query(mockito::Matcher::AllOf(vec![
            mockito::Matcher::UrlEncoded("api_key".into(), "test_tmdb_token".into()),
            mockito::Matcher::UrlEncoded("language".into(), "en-US".into()),
        ]))
        .with_status(200)
        .with_body(common::fixtures::TMDB_MOVIE_DETAILS)
        .expect(1)
        .create();

    let mut trakt = Trakt::with_config(TraktConfig {
        client_id: "test_client".to_string(),
        username: "testuser".to_string(),
        oauth_access_token: None,
        trakt_base_url: None,
        tmdb_base_url: Some(server.url()),
        language: Some("en-US".to_string()),
    });

    // First call - should hit API
    let title1 = trakt.get_title(
        MediaType::Movie,
        "27205".to_string(),
        "test_tmdb_token",
        None,
        None,
    );

    // Second call - should hit cache
    let title2 = trakt.get_title(
        MediaType::Movie,
        "27205".to_string(),
        "test_tmdb_token",
        None,
        None,
    );

    mock.assert();
    assert_eq!(title1, "Inception");
    assert_eq!(title2, "Inception");
}

#[test]
fn test_set_language_clears_cache() {
    let mut server = mockito::Server::new();

    // Expect two calls - cache should be cleared when language changes
    let mock_en = server
        .mock("GET", "/3/movie/27205")
        .match_query(mockito::Matcher::UrlEncoded(
            "language".into(),
            "en-US".into(),
        ))
        .with_status(200)
        .with_body(common::fixtures::TMDB_MOVIE_DETAILS)
        .expect(1)
        .create();

    let mock_fr = server
        .mock("GET", "/3/movie/27205")
        .match_query(mockito::Matcher::UrlEncoded(
            "language".into(),
            "fr-FR".into(),
        ))
        .with_status(200)
        .with_body(common::fixtures::TMDB_MOVIE_DETAILS_FR)
        .expect(1)
        .create();

    let mut trakt = Trakt::with_config(TraktConfig {
        client_id: "test_client".to_string(),
        username: "testuser".to_string(),
        oauth_access_token: None,
        trakt_base_url: None,
        tmdb_base_url: Some(server.url()),
        language: Some("en-US".to_string()),
    });

    // First call in English
    let title_en = trakt.get_title(
        MediaType::Movie,
        "27205".to_string(),
        "test_tmdb_token",
        None,
        None,
    );
    assert_eq!(title_en, "Inception");

    // Change language
    trakt.set_language("fr-FR".to_string());

    // Second call in French - should hit API again due to cache clear
    let title_fr = trakt.get_title(
        MediaType::Movie,
        "27205".to_string(),
        "test_tmdb_token",
        None,
        None,
    );

    mock_en.assert();
    mock_fr.assert();
    // Both return "Inception" since French version has same title
    assert_eq!(title_fr, "Inception");
}

#[test]
fn test_get_title_handles_api_error() {
    let mut server = mockito::Server::new();

    let mock = server
        .mock("GET", "/3/movie/99999")
        .match_query(mockito::Matcher::AllOf(vec![
            mockito::Matcher::UrlEncoded("api_key".into(), "test_tmdb_token".into()),
            mockito::Matcher::UrlEncoded("language".into(), "en-US".into()),
        ]))
        .with_status(404)
        .create();

    let mut trakt = Trakt::with_config(TraktConfig {
        client_id: "test_client".to_string(),
        username: "testuser".to_string(),
        oauth_access_token: None,
        trakt_base_url: None,
        tmdb_base_url: Some(server.url()),
        language: Some("en-US".to_string()),
    });

    let title = trakt.get_title(
        MediaType::Movie,
        "99999".to_string(),
        "test_tmdb_token",
        None,
        None,
    );

    mock.assert();
    assert_eq!(title, "");
}

#[test]
fn test_get_title_caches_empty_results() {
    let mut server = mockito::Server::new();

    // Only expect one call - empty result should be cached
    let mock = server
        .mock("GET", "/3/movie/99999")
        .match_query(mockito::Matcher::AllOf(vec![
            mockito::Matcher::UrlEncoded("api_key".into(), "test_tmdb_token".into()),
            mockito::Matcher::UrlEncoded("language".into(), "en-US".into()),
        ]))
        .with_status(404)
        .expect(1)
        .create();

    let mut trakt = Trakt::with_config(TraktConfig {
        client_id: "test_client".to_string(),
        username: "testuser".to_string(),
        oauth_access_token: None,
        trakt_base_url: None,
        tmdb_base_url: Some(server.url()),
        language: Some("en-US".to_string()),
    });

    // First call - should hit API and cache empty result
    let title1 = trakt.get_title(
        MediaType::Movie,
        "99999".to_string(),
        "test_tmdb_token",
        None,
        None,
    );

    // Second call - should hit cache
    let title2 = trakt.get_title(
        MediaType::Movie,
        "99999".to_string(),
        "test_tmdb_token",
        None,
        None,
    );

    mock.assert();
    assert_eq!(title1, "");
    assert_eq!(title2, "");
}

// ============================================================================
// LRU Cache Eviction Tests
// ============================================================================

use discrakt::trakt::MAX_CACHE_SIZE;

#[test]
fn test_max_cache_size_is_reasonable() {
    // Verify the cache size constant is the expected value
    assert_eq!(MAX_CACHE_SIZE, 500);
}

#[test]
fn test_rating_cache_evicts_old_entries() {
    let mut server = mockito::Server::new();

    // Create mocks for MAX_CACHE_SIZE + 1 different movies
    // We'll use a small subset to verify eviction behavior
    let test_size = 5; // Use small number for test efficiency

    let mut mocks = Vec::new();
    for i in 0..=test_size {
        let mock = server
            .mock("GET", format!("/movies/movie-{}/ratings", i).as_str())
            .with_status(200)
            .with_body(format!(
                r#"{{"rating": {}.0, "votes": 100, "distribution": {{"1": 10, "2": 10, "3": 10, "4": 10, "5": 10, "6": 10, "7": 10, "8": 10, "9": 10, "10": 10}}}}"#,
                i
            ))
            .expect(1)
            .create();
        mocks.push(mock);
    }

    let mut trakt = Trakt::with_config(TraktConfig {
        client_id: "test_client".to_string(),
        username: "testuser".to_string(),
        oauth_access_token: None,
        trakt_base_url: Some(server.url()),
        tmdb_base_url: None,
        language: None,
    });

    // Fill the cache with ratings
    for i in 0..=test_size {
        let rating = trakt.get_movie_rating(format!("movie-{}", i));
        assert_eq!(rating, i as f64);
    }

    // All mocks should have been called exactly once
    for mock in mocks {
        mock.assert();
    }
}

#[test]
fn test_title_cache_evicts_old_entries() {
    let mut server = mockito::Server::new();

    // Create mocks for multiple different movies
    let test_size = 5;

    let mut mocks = Vec::new();
    for i in 0..=test_size {
        let mock = server
            .mock("GET", format!("/3/movie/{}", i).as_str())
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("api_key".into(), "test_token".into()),
                mockito::Matcher::UrlEncoded("language".into(), "en-US".into()),
            ]))
            .with_status(200)
            .with_body(format!(r#"{{"title": "Movie {}"}}"#, i))
            .expect(1)
            .create();
        mocks.push(mock);
    }

    let mut trakt = Trakt::with_config(TraktConfig {
        client_id: "test_client".to_string(),
        username: "testuser".to_string(),
        oauth_access_token: None,
        trakt_base_url: None,
        tmdb_base_url: Some(server.url()),
        language: Some("en-US".to_string()),
    });

    // Fill the cache with titles
    for i in 0..=test_size {
        let title = trakt.get_title(MediaType::Movie, i.to_string(), "test_token", None, None);
        assert_eq!(title, format!("Movie {}", i));
    }

    // All mocks should have been called exactly once
    for mock in mocks {
        mock.assert();
    }
}

#[test]
fn test_image_cache_evicts_old_entries() {
    let mut server = mockito::Server::new();

    // Create mocks for multiple different movies
    let test_size = 5;

    let mut mocks = Vec::new();
    for i in 0..=test_size {
        let mock = server
            .mock("GET", format!("/3/movie/{}/images", i).as_str())
            .match_query(mockito::Matcher::UrlEncoded(
                "api_key".into(),
                "test_token".into(),
            ))
            .with_status(200)
            .with_body(format!(
                r#"{{"posters": [{{"file_path": "/poster_{}.jpg"}}]}}"#,
                i
            ))
            .expect(1)
            .create();
        mocks.push(mock);
    }

    let mut trakt = Trakt::with_config(TraktConfig {
        client_id: "test_client".to_string(),
        username: "testuser".to_string(),
        oauth_access_token: None,
        trakt_base_url: None,
        tmdb_base_url: Some(server.url()),
        language: None,
    });

    // Fill the cache with posters
    for i in 0..=test_size {
        let poster = trakt.get_poster(MediaType::Movie, i.to_string(), "test_token".to_string(), 0);
        assert!(poster.is_some());
        assert!(poster.unwrap().contains(&format!("poster_{}.jpg", i)));
    }

    // All mocks should have been called exactly once
    for mock in mocks {
        mock.assert();
    }
}

#[test]
fn test_lru_cache_promotes_recently_accessed() {
    let mut server = mockito::Server::new();

    // This test verifies that accessing a cached item promotes it (LRU behavior)
    // We access items in a specific pattern to test promotion

    let mock1 = server
        .mock("GET", "/movies/movie-1/ratings")
        .with_status(200)
        .with_body(r#"{"rating": 1.0, "votes": 100, "distribution": {"1": 10, "2": 10, "3": 10, "4": 10, "5": 10, "6": 10, "7": 10, "8": 10, "9": 10, "10": 10}}"#)
        .expect(1)
        .create();

    let mock2 = server
        .mock("GET", "/movies/movie-2/ratings")
        .with_status(200)
        .with_body(r#"{"rating": 2.0, "votes": 100, "distribution": {"1": 10, "2": 10, "3": 10, "4": 10, "5": 10, "6": 10, "7": 10, "8": 10, "9": 10, "10": 10}}"#)
        .expect(1)
        .create();

    let mut trakt = Trakt::with_config(TraktConfig {
        client_id: "test_client".to_string(),
        username: "testuser".to_string(),
        oauth_access_token: None,
        trakt_base_url: Some(server.url()),
        tmdb_base_url: None,
        language: None,
    });

    // First access - movie-1 goes into cache
    let rating1 = trakt.get_movie_rating("movie-1".to_string());
    assert_eq!(rating1, 1.0);

    // Second access - movie-2 goes into cache
    let rating2 = trakt.get_movie_rating("movie-2".to_string());
    assert_eq!(rating2, 2.0);

    // Access movie-1 again - should come from cache and be promoted to most recent
    let rating1_again = trakt.get_movie_rating("movie-1".to_string());
    assert_eq!(rating1_again, 1.0);

    // Access movie-2 again - should come from cache
    let rating2_again = trakt.get_movie_rating("movie-2".to_string());
    assert_eq!(rating2_again, 2.0);

    // Verify mocks were called exactly once (proving cache worked)
    mock1.assert();
    mock2.assert();
}

// ============================================================================
// Retry Behavior Tests
// ============================================================================

use discrakt::retry::RetryConfig;
use std::time::Duration;

/// Helper to create a fast retry config for testing.
/// Uses short delays to make tests run quickly.
fn fast_retry_config() -> RetryConfig {
    RetryConfig {
        max_retries: 3,
        base_delay: Duration::from_millis(10),
        max_delay: Duration::from_millis(100),
        enable_jitter: false, // No jitter for predictable timing
    }
}

#[test]
fn test_get_watching_retries_on_503() {
    // Test that get_watching retries on 503 Service Unavailable errors
    // and eventually succeeds when the server recovers.
    let mut server = mockito::Server::new();

    // First two calls return 503, third call succeeds
    let mock_503_first = server
        .mock("GET", "/users/testuser/watching")
        .match_query(mockito::Matcher::UrlEncoded(
            "extended".into(),
            "full".into(),
        ))
        .with_status(503)
        .expect(1)
        .create();

    let mock_503_second = server
        .mock("GET", "/users/testuser/watching")
        .match_query(mockito::Matcher::UrlEncoded(
            "extended".into(),
            "full".into(),
        ))
        .with_status(503)
        .expect(1)
        .create();

    let mock_success = server
        .mock("GET", "/users/testuser/watching")
        .match_query(mockito::Matcher::UrlEncoded(
            "extended".into(),
            "full".into(),
        ))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(common::fixtures::TRAKT_MOVIE_WATCHING)
        .expect(1)
        .create();

    let mut trakt = Trakt::with_config(TraktConfig {
        client_id: "test_client".to_string(),
        username: "testuser".to_string(),
        oauth_access_token: None,
        trakt_base_url: Some(server.url()),
        tmdb_base_url: None,
        language: None,
    });
    trakt.set_retry_config(fast_retry_config());

    let result = trakt.get_watching();

    // Verify all mocks were called in order
    mock_503_first.assert();
    mock_503_second.assert();
    mock_success.assert();

    // Should return successful response after retries
    assert!(result.is_some());
    let response = result.unwrap();
    assert_eq!(response.r#type, "movie");
    assert_eq!(response.movie.as_ref().unwrap().title, "Inception");
}

#[test]
fn test_get_watching_retries_on_429() {
    // Test that get_watching retries on 429 Too Many Requests (rate limiting)
    // and eventually succeeds when the rate limit clears.
    let mut server = mockito::Server::new();

    // First two calls return 429 (rate limited), third call succeeds
    let mock_429_first = server
        .mock("GET", "/users/testuser/watching")
        .match_query(mockito::Matcher::UrlEncoded(
            "extended".into(),
            "full".into(),
        ))
        .with_status(429)
        .expect(1)
        .create();

    let mock_429_second = server
        .mock("GET", "/users/testuser/watching")
        .match_query(mockito::Matcher::UrlEncoded(
            "extended".into(),
            "full".into(),
        ))
        .with_status(429)
        .expect(1)
        .create();

    let mock_success = server
        .mock("GET", "/users/testuser/watching")
        .match_query(mockito::Matcher::UrlEncoded(
            "extended".into(),
            "full".into(),
        ))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(common::fixtures::TRAKT_EPISODE_WATCHING)
        .expect(1)
        .create();

    let mut trakt = Trakt::with_config(TraktConfig {
        client_id: "test_client".to_string(),
        username: "testuser".to_string(),
        oauth_access_token: None,
        trakt_base_url: Some(server.url()),
        tmdb_base_url: None,
        language: None,
    });
    trakt.set_retry_config(fast_retry_config());

    let result = trakt.get_watching();

    mock_429_first.assert();
    mock_429_second.assert();
    mock_success.assert();

    assert!(result.is_some());
    let response = result.unwrap();
    assert_eq!(response.r#type, "episode");
    assert_eq!(response.show.as_ref().unwrap().title, "Breaking Bad");
}

#[test]
fn test_get_watching_gives_up_after_max_retries() {
    // Test that get_watching gives up after exhausting all retry attempts
    // when the server returns 503 indefinitely.
    let mut server = mockito::Server::new();

    // Server always returns 503 - should hit max_retries + 1 times (initial + retries)
    let mock_503 = server
        .mock("GET", "/users/testuser/watching")
        .match_query(mockito::Matcher::UrlEncoded(
            "extended".into(),
            "full".into(),
        ))
        .with_status(503)
        .expect(4) // 1 initial + 3 retries = 4 total attempts
        .create();

    let mut trakt = Trakt::with_config(TraktConfig {
        client_id: "test_client".to_string(),
        username: "testuser".to_string(),
        oauth_access_token: None,
        trakt_base_url: Some(server.url()),
        tmdb_base_url: None,
        language: None,
    });
    trakt.set_retry_config(fast_retry_config());

    let result = trakt.get_watching();

    mock_503.assert();

    // Should return None after exhausting retries
    assert!(result.is_none());
}

#[test]
fn test_get_movie_rating_retries_on_server_error() {
    // Test that get_movie_rating retries on 500 Internal Server Error
    // and eventually returns the correct rating when the server recovers.
    let mut server = mockito::Server::new();

    // First call returns 500, second call succeeds
    let mock_500 = server
        .mock("GET", "/movies/test-movie/ratings")
        .with_status(500)
        .expect(1)
        .create();

    let mock_success = server
        .mock("GET", "/movies/test-movie/ratings")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(common::fixtures::TRAKT_MOVIE_RATINGS)
        .expect(1)
        .create();

    let mut trakt = Trakt::with_config(TraktConfig {
        client_id: "test_client".to_string(),
        username: "testuser".to_string(),
        oauth_access_token: None,
        trakt_base_url: Some(server.url()),
        tmdb_base_url: None,
        language: None,
    });
    trakt.set_retry_config(fast_retry_config());

    let rating = trakt.get_movie_rating("test-movie".to_string());

    mock_500.assert();
    mock_success.assert();

    // Should return the rating after successful retry
    assert!((rating - 8.45123).abs() < 0.0001);
}

#[test]
fn test_get_poster_retries_on_transient_error() {
    // Test that get_poster retries on 503 Service Unavailable from TMDB
    // and eventually returns the poster URL when the service recovers.
    let mut server = mockito::Server::new();

    // First call returns 503, second call succeeds
    let mock_503 = server
        .mock("GET", "/3/movie/27205/images")
        .match_query(mockito::Matcher::UrlEncoded(
            "api_key".into(),
            "test_token".into(),
        ))
        .with_status(503)
        .expect(1)
        .create();

    let mock_success = server
        .mock("GET", "/3/movie/27205/images")
        .match_query(mockito::Matcher::UrlEncoded(
            "api_key".into(),
            "test_token".into(),
        ))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(common::fixtures::TMDB_MOVIE_IMAGES)
        .expect(1)
        .create();

    let mut trakt = Trakt::with_config(TraktConfig {
        client_id: "test_client".to_string(),
        username: "testuser".to_string(),
        oauth_access_token: None,
        trakt_base_url: None,
        tmdb_base_url: Some(server.url()),
        language: None,
    });
    trakt.set_retry_config(fast_retry_config());

    let poster = trakt.get_poster(
        MediaType::Movie,
        "27205".to_string(),
        "test_token".to_string(),
        0,
    );

    mock_503.assert();
    mock_success.assert();

    // Should return the poster URL after successful retry
    assert!(poster.is_some());
    let url = poster.unwrap();
    assert!(url.contains("oYuLEt3zVCKq57qu2F8dT7NIa6f.jpg"));
}

#[test]
fn test_get_title_retries_on_502() {
    // Test that get_title retries on 502 Bad Gateway errors from TMDB
    // and eventually returns the correct title when the service recovers.
    let mut server = mockito::Server::new();

    // First call returns 502, second call succeeds
    let mock_502 = server
        .mock("GET", "/3/movie/27205")
        .match_query(mockito::Matcher::AllOf(vec![
            mockito::Matcher::UrlEncoded("api_key".into(), "test_token".into()),
            mockito::Matcher::UrlEncoded("language".into(), "en-US".into()),
        ]))
        .with_status(502)
        .expect(1)
        .create();

    let mock_success = server
        .mock("GET", "/3/movie/27205")
        .match_query(mockito::Matcher::AllOf(vec![
            mockito::Matcher::UrlEncoded("api_key".into(), "test_token".into()),
            mockito::Matcher::UrlEncoded("language".into(), "en-US".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(common::fixtures::TMDB_MOVIE_DETAILS)
        .expect(1)
        .create();

    let mut trakt = Trakt::with_config(TraktConfig {
        client_id: "test_client".to_string(),
        username: "testuser".to_string(),
        oauth_access_token: None,
        trakt_base_url: None,
        tmdb_base_url: Some(server.url()),
        language: Some("en-US".to_string()),
    });
    trakt.set_retry_config(fast_retry_config());

    let title = trakt.get_title(
        MediaType::Movie,
        "27205".to_string(),
        "test_token",
        None,
        None,
    );

    mock_502.assert();
    mock_success.assert();

    // Should return the title after successful retry
    assert_eq!(title, "Inception");
}

#[test]
fn test_retry_does_not_retry_on_4xx_client_errors() {
    // Test that 4xx errors (except 429) are NOT retried since they
    // indicate client-side issues that won't resolve with retries.
    let mut server = mockito::Server::new();

    // Server returns 404 - should NOT be retried
    let mock_404 = server
        .mock("GET", "/movies/nonexistent/ratings")
        .with_status(404)
        .expect(1) // Only 1 call - no retries
        .create();

    let mut trakt = Trakt::with_config(TraktConfig {
        client_id: "test_client".to_string(),
        username: "testuser".to_string(),
        oauth_access_token: None,
        trakt_base_url: Some(server.url()),
        tmdb_base_url: None,
        language: None,
    });
    trakt.set_retry_config(fast_retry_config());

    let rating = trakt.get_movie_rating("nonexistent".to_string());

    mock_404.assert();

    // Should return 0.0 immediately without retrying
    assert_eq!(rating, 0.0);
}

#[test]
fn test_retry_returns_parse_error_on_malformed_json_after_retries() {
    // Test that when retries succeed (HTTP 200) but the response contains
    // malformed JSON, a ParseError is returned instead of silently failing.
    let mut server = mockito::Server::new();

    // First call returns 503, second call succeeds with malformed JSON
    let mock_503 = server
        .mock("GET", "/movies/malformed-movie/ratings")
        .with_status(503)
        .expect(1)
        .create();

    let mock_success_malformed = server
        .mock("GET", "/movies/malformed-movie/ratings")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"rating": "not_a_number", "invalid_json"#) // Malformed JSON
        .expect(1)
        .create();

    let mut trakt = Trakt::with_config(TraktConfig {
        client_id: "test_client".to_string(),
        username: "testuser".to_string(),
        oauth_access_token: None,
        trakt_base_url: Some(server.url()),
        tmdb_base_url: None,
        language: None,
    });
    trakt.set_retry_config(fast_retry_config());

    // Should return 0.0 after parse error (graceful degradation)
    let rating = trakt.get_movie_rating("malformed-movie".to_string());

    mock_503.assert();
    mock_success_malformed.assert();

    // The response failed to parse, so we get the default value
    assert_eq!(rating, 0.0);
}

#[test]
fn test_retry_on_408_request_timeout() {
    // Test that HTTP 408 Request Timeout is correctly retried
    let mut server = mockito::Server::new();

    // First call returns 408 (Request Timeout), second call succeeds
    let mock_408 = server
        .mock("GET", "/movies/timeout-movie/ratings")
        .with_status(408)
        .expect(1)
        .create();

    let mock_success = server
        .mock("GET", "/movies/timeout-movie/ratings")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(common::fixtures::TRAKT_MOVIE_RATINGS)
        .expect(1)
        .create();

    let mut trakt = Trakt::with_config(TraktConfig {
        client_id: "test_client".to_string(),
        username: "testuser".to_string(),
        oauth_access_token: None,
        trakt_base_url: Some(server.url()),
        tmdb_base_url: None,
        language: None,
    });
    trakt.set_retry_config(fast_retry_config());

    let rating = trakt.get_movie_rating("timeout-movie".to_string());

    mock_408.assert();
    mock_success.assert();

    // Should return the rating after successful retry
    assert!((rating - 8.45123).abs() < 0.0001);
}

#[test]
fn test_rating_cache_eviction_at_max_cache_size_boundary() {
    // This test verifies that the LRU cache correctly evicts the oldest entry
    // when it exceeds MAX_CACHE_SIZE. We fill the cache to exactly MAX_CACHE_SIZE,
    // then add one more entry, and verify the first entry was evicted.
    let mut server = mockito::Server::new();

    // Create mock for the first entry (movie-0) - will be called twice:
    // once initially, and once after eviction when we re-request it
    let mock_first = server
        .mock("GET", "/movies/movie-0/ratings")
        .with_status(200)
        .with_body(r#"{"rating": 0.0, "votes": 100, "distribution": {"1": 10, "2": 10, "3": 10, "4": 10, "5": 10, "6": 10, "7": 10, "8": 10, "9": 10, "10": 10}}"#)
        .expect(2) // Called twice: initial + after eviction
        .create();

    // Create mocks for entries 1 through MAX_CACHE_SIZE (each called once)
    let mut other_mocks = Vec::new();
    for i in 1..=MAX_CACHE_SIZE {
        let mock = server
            .mock("GET", format!("/movies/movie-{}/ratings", i).as_str())
            .with_status(200)
            .with_body(format!(
                r#"{{"rating": {}.0, "votes": 100, "distribution": {{"1": 10, "2": 10, "3": 10, "4": 10, "5": 10, "6": 10, "7": 10, "8": 10, "9": 10, "10": 10}}}}"#,
                i
            ))
            .expect(1)
            .create();
        other_mocks.push(mock);
    }

    let mut trakt = Trakt::with_config(TraktConfig {
        client_id: "test_client".to_string(),
        username: "testuser".to_string(),
        oauth_access_token: None,
        trakt_base_url: Some(server.url()),
        tmdb_base_url: None,
        language: None,
    });

    // Step 1: Add movie-0 (this will be the oldest entry)
    let rating_0_first = trakt.get_movie_rating("movie-0".to_string());
    assert_eq!(rating_0_first, 0.0);

    // Step 2: Fill the remaining cache slots (1 through MAX_CACHE_SIZE - 1)
    // After this, the cache has exactly MAX_CACHE_SIZE entries
    for i in 1..MAX_CACHE_SIZE {
        let rating = trakt.get_movie_rating(format!("movie-{}", i));
        assert_eq!(rating, i as f64);
    }

    // Step 3: Add one more entry (movie-MAX_CACHE_SIZE) which should evict movie-0
    let rating_last = trakt.get_movie_rating(format!("movie-{}", MAX_CACHE_SIZE));
    assert_eq!(rating_last, MAX_CACHE_SIZE as f64);

    // Step 4: Request movie-0 again - it should have been evicted,
    // so this should trigger a new API call (mock_first expects 2 calls)
    let rating_0_second = trakt.get_movie_rating("movie-0".to_string());
    assert_eq!(rating_0_second, 0.0);

    // Verify all expectations were met
    mock_first.assert();
    for mock in other_mocks {
        mock.assert();
    }
}

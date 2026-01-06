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
fn test_get_watching_with_oauth_token() {
    let mut server = mockito::Server::new();

    let mock = server
        .mock("GET", "/users/testuser/watching")
        .match_header("Authorization", "Bearer my_oauth_token")
        .with_status(200)
        .with_body(common::fixtures::TRAKT_MOVIE_WATCHING)
        .create();

    let trakt = Trakt::with_config(TraktConfig {
        client_id: "test_client".to_string(),
        username: "testuser".to_string(),
        oauth_access_token: Some("my_oauth_token".to_string()),
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

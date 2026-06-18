// Integration tests for the source layer in src/source/.
//
// These verify that `TraktSource` maps a raw Trakt response into a fully
// enriched, source-agnostic `Watching` (artwork, localized titles, rating,
// links, and timing), using a mock server for both the Trakt and TMDB APIs.

mod common;

use discrakt::source::trakt::TraktSource;
use discrakt::source::{MediaKind, Source};
use discrakt::trakt::{Trakt, TraktConfig};

fn trakt_source(server_url: String) -> TraktSource {
    let trakt = Trakt::with_config(TraktConfig {
        client_id: "test_client".to_string(),
        username: "testuser".to_string(),
        oauth_access_token: None,
        // Point both Trakt and TMDB at the same mock server.
        trakt_base_url: Some(server_url.clone()),
        tmdb_base_url: Some(server_url),
        language: None,
    });
    TraktSource::new(trakt, "test_tmdb_token".to_string())
}

#[test]
fn test_trakt_source_enriches_movie() {
    let mut server = mockito::Server::new();

    let watching = server
        .mock("GET", "/users/testuser/watching")
        .match_query(mockito::Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(common::fixtures::TRAKT_MOVIE_WATCHING)
        .create();

    let rating = server
        .mock("GET", "/movies/inception-2010/ratings")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"rating": 8.5, "votes": 100, "distribution": {}}"#)
        .create();

    let poster = server
        .mock("GET", "/3/movie/27205/images")
        .match_query(mockito::Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"posters": [{"file_path": "/abc.jpg"}]}"#)
        .create();

    let title = server
        .mock("GET", "/3/movie/27205")
        .match_query(mockito::Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"title": "Inception"}"#)
        .create();

    let mut source = trakt_source(server.url());
    let result = source.get_watching().expect("movie should be watching");

    watching.assert();
    rating.assert();
    poster.assert();
    title.assert();

    assert_eq!(result.kind, MediaKind::Movie);
    assert_eq!(result.title, "Inception");
    assert_eq!(result.year, Some(2010));
    assert_eq!(result.rating, Some(8.5));
    assert_eq!(
        result.poster_url.as_deref(),
        Some("https://image.tmdb.org/t/p/w600_and_h600_bestv2/abc.jpg")
    );
    assert_eq!(
        result.imdb_url.as_deref(),
        Some("https://www.imdb.com/title/tt1375666")
    );
    assert_eq!(
        result.source_link,
        Some((
            "Trakt".to_string(),
            "https://trakt.tv/movies/inception-2010".to_string()
        ))
    );
    assert_eq!(result.runtime_minutes, Some(150));
}

#[test]
fn test_trakt_source_enriches_episode() {
    let mut server = mockito::Server::new();

    let watching = server
        .mock("GET", "/users/testuser/watching")
        .match_query(mockito::Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(common::fixtures::TRAKT_EPISODE_WATCHING)
        .create();

    let poster = server
        .mock("GET", "/3/tv/1396/season/5/images")
        .match_query(mockito::Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"posters": [{"file_path": "/show.jpg"}]}"#)
        .create();

    let show_title = server
        .mock("GET", "/3/tv/1396")
        .match_query(mockito::Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"name": "Breaking Bad"}"#)
        .create();

    let episode_title = server
        .mock("GET", "/3/tv/1396/season/5/episode/16")
        .match_query(mockito::Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"name": "Felina"}"#)
        .create();

    let mut source = trakt_source(server.url());
    let result = source.get_watching().expect("episode should be watching");

    watching.assert();
    poster.assert();
    show_title.assert();
    episode_title.assert();

    assert_eq!(result.kind, MediaKind::Episode);
    assert_eq!(result.title, "Breaking Bad");
    assert_eq!(result.season, Some(5));
    assert_eq!(result.episode_number, Some(16));
    assert_eq!(result.episode_title.as_deref(), Some("Felina"));
    assert_eq!(result.rating, None);
    assert_eq!(
        result.poster_url.as_deref(),
        Some("https://image.tmdb.org/t/p/w600_and_h600_bestv2/show.jpg")
    );
    assert_eq!(
        result.source_link,
        Some((
            "Trakt".to_string(),
            "https://trakt.tv/shows/breaking-bad".to_string()
        ))
    );
    assert_eq!(result.runtime_minutes, Some(60));
}

#[test]
fn test_trakt_source_returns_none_when_nothing_watching() {
    let mut server = mockito::Server::new();

    let watching = server
        .mock("GET", "/users/testuser/watching")
        .match_query(mockito::Matcher::Any)
        .with_status(204)
        .create();

    let mut source = trakt_source(server.url());
    assert!(source.get_watching().is_none());

    watching.assert();
}

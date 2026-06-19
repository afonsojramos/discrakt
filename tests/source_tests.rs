// Integration tests for the source layer in src/source/.
//
// These verify that `TraktSource` maps a raw Trakt response into a fully
// enriched, source-agnostic `Watching` (artwork, localized titles, rating,
// links, and timing), using a mock server for both the Trakt and TMDB APIs.

mod common;

use discrakt::source::plex::{PlexConfig, PlexSource};
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

// ============================================================================
// PlexSource tests
// ============================================================================

const PLEX_MOVIE_SESSION: &str = r#"{
    "MediaContainer": {
        "size": 1,
        "Metadata": [{
            "type": "movie",
            "title": "Inception",
            "year": 2010,
            "duration": 8880000,
            "viewOffset": 600000,
            "Guid": [
                {"id": "imdb://tt1375666"},
                {"id": "tmdb://27205"},
                {"id": "tvdb://12345"}
            ],
            "User": {"id": "1", "title": "alice"},
            "Player": {"state": "playing"}
        }]
    }
}"#;

const PLEX_EPISODE_SESSION: &str = r#"{
    "MediaContainer": {
        "size": 1,
        "Metadata": [{
            "type": "episode",
            "title": "Felina",
            "grandparentTitle": "Breaking Bad",
            "parentIndex": 5,
            "index": 16,
            "year": 2013,
            "duration": 3120000,
            "viewOffset": 60000,
            "grandparentGuid": "tmdb://1396",
            "Guid": [{"id": "imdb://tt2301451"}],
            "User": {"title": "alice"},
            "Player": {"state": "playing"}
        }]
    }
}"#;

fn plex_source(server_url: String, body: &str, server: &mut mockito::Server) -> PlexSource {
    server
        .mock("GET", "/status/sessions")
        .match_header("x-plex-token", "plex_token")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(body)
        .create();

    PlexSource::new(PlexConfig {
        server_url: server_url.clone(),
        token: "plex_token".to_string(),
        username: "alice".to_string(),
        tmdb_token: "test_tmdb_token".to_string(),
        tmdb_base_url: Some(server_url),
        language: None,
    })
}

#[test]
fn test_plex_source_enriches_movie() {
    let mut server = mockito::Server::new();
    let url = server.url();

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

    let mut source = plex_source(url, PLEX_MOVIE_SESSION, &mut server);
    let result = source.get_watching().expect("movie should be playing");

    poster.assert();
    title.assert();

    assert_eq!(result.kind, MediaKind::Movie);
    assert_eq!(result.title, "Inception");
    assert_eq!(result.year, Some(2010));
    assert_eq!(result.rating, None);
    assert_eq!(result.ids.tmdb, Some(27205));
    assert_eq!(
        result.poster_url.as_deref(),
        Some("https://image.tmdb.org/t/p/w600_and_h600_bestv2/abc.jpg")
    );
    assert_eq!(
        result.imdb_url.as_deref(),
        Some("https://www.imdb.com/title/tt1375666")
    );
    assert_eq!(result.source_link, None);
}

#[test]
fn test_plex_source_enriches_episode() {
    let mut server = mockito::Server::new();
    let url = server.url();

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

    let mut source = plex_source(url, PLEX_EPISODE_SESSION, &mut server);
    let result = source.get_watching().expect("episode should be playing");

    poster.assert();
    show_title.assert();
    episode_title.assert();

    assert_eq!(result.kind, MediaKind::Episode);
    assert_eq!(result.title, "Breaking Bad");
    assert_eq!(result.season, Some(5));
    assert_eq!(result.episode_number, Some(16));
    assert_eq!(result.episode_title.as_deref(), Some("Felina"));
    assert_eq!(
        result.poster_url.as_deref(),
        Some("https://image.tmdb.org/t/p/w600_and_h600_bestv2/show.jpg")
    );
    assert_eq!(result.ids.tmdb, Some(1396));
}

#[test]
fn test_plex_source_episode_without_tmdb_falls_back_to_plex_titles() {
    let body = r#"{
        "MediaContainer": {
            "Metadata": [{
                "type": "episode",
                "title": "Pilot",
                "grandparentTitle": "The Office",
                "parentIndex": 1,
                "index": 1,
                "duration": 1320000,
                "viewOffset": 60000,
                "grandparentGuid": "plex://show/abcdef",
                "Guid": [],
                "User": {"title": "alice"},
                "Player": {"state": "playing"}
            }]
        }
    }"#;
    let mut server = mockito::Server::new();
    let url = server.url();
    let mut source = plex_source(url, body, &mut server);

    let result = source.get_watching().expect("episode should be playing");

    // No usable TMDB id -> Plex's own titles, no poster, no buttons.
    assert_eq!(result.title, "The Office");
    assert_eq!(result.episode_title.as_deref(), Some("Pilot"));
    assert_eq!(result.poster_url, None);
    assert_eq!(result.imdb_url, None);
}

#[test]
fn test_plex_source_ignores_other_users() {
    let body = r#"{
        "MediaContainer": {
            "Metadata": [{
                "type": "movie",
                "title": "Inception",
                "year": 2010,
                "duration": 8880000,
                "viewOffset": 600000,
                "Guid": [{"id": "tmdb://27205"}],
                "User": {"title": "bob"},
                "Player": {"state": "playing"}
            }]
        }
    }"#;
    let mut server = mockito::Server::new();
    let url = server.url();
    let mut source = plex_source(url, body, &mut server);

    assert!(source.get_watching().is_none());
}

#[test]
fn test_plex_source_ignores_paused_sessions() {
    let body = r#"{
        "MediaContainer": {
            "Metadata": [{
                "type": "movie",
                "title": "Inception",
                "year": 2010,
                "duration": 8880000,
                "viewOffset": 600000,
                "Guid": [{"id": "tmdb://27205"}],
                "User": {"title": "alice"},
                "Player": {"state": "paused"}
            }]
        }
    }"#;
    let mut server = mockito::Server::new();
    let url = server.url();
    let mut source = plex_source(url, body, &mut server);

    assert!(source.get_watching().is_none());
}

#[test]
fn test_plex_source_returns_none_when_no_sessions() {
    let body = r#"{"MediaContainer": {"size": 0}}"#;
    let mut server = mockito::Server::new();
    let url = server.url();
    let mut source = plex_source(url, body, &mut server);

    assert!(source.get_watching().is_none());
}

#[test]
fn test_plex_source_missing_duration_still_displays() {
    // A session with no duration/viewOffset must not collapse to a zero-length
    // window (which main would treat as already expired).
    let body = r#"{
        "MediaContainer": {
            "Metadata": [{
                "type": "movie",
                "title": "Some Movie",
                "year": 2020,
                "Guid": [],
                "User": {"title": "alice"},
                "Player": {"state": "playing"}
            }]
        }
    }"#;
    let mut server = mockito::Server::new();
    let url = server.url();
    let mut source = plex_source(url, body, &mut server);

    let result = source.get_watching().expect("movie should be playing");

    assert_eq!(result.title, "Some Movie");
    assert_eq!(result.poster_url, None);
    // Window must be well into the future so the session is not seen as expired.
    let window = result.expires_at.timestamp() - result.started_at.timestamp();
    assert!(window >= 3600, "window was only {window}s");
}

#[test]
fn test_plex_source_episode_without_season_still_localizes_title() {
    // No parentIndex (season): the show title should still be localized via TMDB,
    // even though no poster or localized episode title can be resolved.
    let body = r#"{
        "MediaContainer": {
            "Metadata": [{
                "type": "episode",
                "title": "Felina",
                "grandparentTitle": "BB",
                "index": 16,
                "duration": 3120000,
                "viewOffset": 60000,
                "grandparentGuid": "tmdb://1396",
                "Guid": [],
                "User": {"title": "alice"},
                "Player": {"state": "playing"}
            }]
        }
    }"#;
    let mut server = mockito::Server::new();
    let url = server.url();

    let show_title = server
        .mock("GET", "/3/tv/1396")
        .match_query(mockito::Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"name": "Breaking Bad"}"#)
        .create();

    let mut source = plex_source(url, body, &mut server);
    let result = source.get_watching().expect("episode should be playing");

    show_title.assert();
    assert_eq!(result.title, "Breaking Bad"); // localized, overriding "BB"
    assert_eq!(result.season, None);
    assert_eq!(result.poster_url, None);
    assert_eq!(result.episode_title.as_deref(), Some("Felina"));
}

#[test]
fn test_plex_source_resolves_tmdb_from_metadata_when_session_lacks_ids() {
    // Real Plex /status/sessions omits the external-id Guid array, exposing only
    // opaque plex:// guids plus rating keys. The TMDB id must be resolved via a
    // follow-up /library/metadata lookup.
    let body = r#"{
        "MediaContainer": {
            "Metadata": [{
                "type": "episode",
                "title": "Felina",
                "grandparentTitle": "Breaking Bad",
                "parentIndex": 5,
                "index": 16,
                "duration": 3120000,
                "viewOffset": 60000,
                "guid": "plex://episode/abc",
                "grandparentGuid": "plex://show/def",
                "grandparentRatingKey": "66310",
                "User": {"title": "alice"},
                "Player": {"state": "playing"}
            }]
        }
    }"#;
    let mut server = mockito::Server::new();
    let url = server.url();

    let metadata = server
        .mock("GET", "/library/metadata/66310")
        .match_query(mockito::Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{"MediaContainer":{"Metadata":[{"Guid":[{"id":"imdb://tt0903747"},{"id":"tmdb://1396"}]}]}}"#,
        )
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

    let mut source = plex_source(url, body, &mut server);
    let result = source.get_watching().expect("episode should be playing");

    metadata.assert();
    poster.assert();
    show_title.assert();
    episode_title.assert();
    assert_eq!(result.ids.tmdb, Some(1396));
    assert_eq!(
        result.poster_url.as_deref(),
        Some("https://image.tmdb.org/t/p/w600_and_h600_bestv2/show.jpg")
    );
}

#[test]
fn test_plex_source_resolves_movie_tmdb_from_metadata() {
    let body = r#"{
        "MediaContainer": {
            "Metadata": [{
                "type": "movie",
                "title": "Inception",
                "year": 2010,
                "duration": 8880000,
                "viewOffset": 600000,
                "guid": "plex://movie/abc",
                "ratingKey": "555",
                "User": {"title": "alice"},
                "Player": {"state": "playing"}
            }]
        }
    }"#;
    let mut server = mockito::Server::new();
    let url = server.url();

    let metadata = server
        .mock("GET", "/library/metadata/555")
        .match_query(mockito::Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"MediaContainer":{"Metadata":[{"Guid":[{"id":"tmdb://27205"}]}]}}"#)
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

    let mut source = plex_source(url, body, &mut server);
    let result = source.get_watching().expect("movie should be playing");

    metadata.assert();
    poster.assert();
    title.assert();
    assert_eq!(result.ids.tmdb, Some(27205));
    assert_eq!(
        result.poster_url.as_deref(),
        Some("https://image.tmdb.org/t/p/w600_and_h600_bestv2/abc.jpg")
    );
}

// ============================================================================
// JellyfinSource tests
// ============================================================================

use discrakt::source::jellyfin::{JellyfinConfig, JellyfinSource};

fn jellyfin_source(server_url: String, body: &str, server: &mut mockito::Server) -> JellyfinSource {
    server
        .mock("GET", "/Sessions")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(body)
        .create();

    JellyfinSource::new(JellyfinConfig {
        server_url: server_url.clone(),
        access_token: "tok".to_string(),
        device_id: "dev".to_string(),
        user_id: "u1".to_string(),
        username: String::new(),
        tmdb_token: "test_tmdb_token".to_string(),
        tmdb_base_url: Some(server_url),
        language: None,
    })
}

#[test]
fn test_jellyfin_source_enriches_movie() {
    let body = r#"[{
        "UserId": "u1", "UserName": "alice",
        "NowPlayingItem": {
            "Name": "Inception", "Type": "Movie", "ProductionYear": 2010,
            "RunTimeTicks": 88800000000,
            "ProviderIds": {"Tmdb": "27205", "Imdb": "tt1375666"}
        },
        "PlayState": {"PositionTicks": 6000000000, "IsPaused": false}
    }]"#;
    let mut server = mockito::Server::new();
    let url = server.url();

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

    let mut source = jellyfin_source(url, body, &mut server);
    let result = source.get_watching().expect("movie should be playing");

    poster.assert();
    title.assert();
    assert_eq!(result.kind, MediaKind::Movie);
    assert_eq!(result.title, "Inception");
    assert_eq!(result.year, Some(2010));
    assert_eq!(result.ids.tmdb, Some(27205));
    assert_eq!(
        result.imdb_url.as_deref(),
        Some("https://www.imdb.com/title/tt1375666")
    );
    assert_eq!(
        result.poster_url.as_deref(),
        Some("https://image.tmdb.org/t/p/w600_and_h600_bestv2/abc.jpg")
    );
}

#[test]
fn test_jellyfin_source_enriches_episode_resolving_series_tmdb() {
    let body = r#"[{
        "UserId": "u1", "UserName": "alice",
        "NowPlayingItem": {
            "Name": "Felina", "Type": "Episode",
            "SeriesName": "Breaking Bad", "SeriesId": "series1",
            "IndexNumber": 16, "ParentIndexNumber": 5,
            "RunTimeTicks": 31200000000,
            "ProviderIds": {"Imdb": "tt2301451"}
        },
        "PlayState": {"PositionTicks": 600000000, "IsPaused": false}
    }]"#;
    let mut server = mockito::Server::new();
    let url = server.url();

    let series = server
        .mock("GET", "/Items")
        .match_query(mockito::Matcher::UrlEncoded("ids".into(), "series1".into()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"Items": [{"ProviderIds": {"Tmdb": "1396"}}]}"#)
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

    let mut source = jellyfin_source(url, body, &mut server);
    let result = source.get_watching().expect("episode should be playing");

    series.assert();
    poster.assert();
    show_title.assert();
    episode_title.assert();
    assert_eq!(result.kind, MediaKind::Episode);
    assert_eq!(result.title, "Breaking Bad");
    assert_eq!(result.season, Some(5));
    assert_eq!(result.episode_number, Some(16));
    assert_eq!(result.ids.tmdb, Some(1396));
    assert_eq!(
        result.poster_url.as_deref(),
        Some("https://image.tmdb.org/t/p/w600_and_h600_bestv2/show.jpg")
    );
}

#[test]
fn test_jellyfin_source_ignores_other_users_and_paused() {
    let other_user = r#"[{
        "UserId": "u2", "UserName": "bob",
        "NowPlayingItem": {"Name": "X", "Type": "Movie", "ProviderIds": {"Tmdb": "1"}},
        "PlayState": {"IsPaused": false}
    }]"#;
    let mut server = mockito::Server::new();
    let mut source = jellyfin_source(server.url(), other_user, &mut server);
    assert!(source.get_watching().is_none());

    let paused = r#"[{
        "UserId": "u1", "UserName": "alice",
        "NowPlayingItem": {"Name": "X", "Type": "Movie", "ProviderIds": {"Tmdb": "1"}},
        "PlayState": {"IsPaused": true}
    }]"#;
    let mut server2 = mockito::Server::new();
    let mut source2 = jellyfin_source(server2.url(), paused, &mut server2);
    assert!(source2.get_watching().is_none());
}

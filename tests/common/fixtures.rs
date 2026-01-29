// JSON response fixtures for API mocking
// Allow dead code since fixtures are used by different test files compiled separately
#![allow(dead_code)]

/// Trakt API: Movie watching response
pub const TRAKT_MOVIE_WATCHING: &str = r#"{
    "expires_at": "2024-01-15T12:30:00.000Z",
    "started_at": "2024-01-15T10:00:00.000Z",
    "action": "watching",
    "type": "movie",
    "movie": {
        "title": "Inception",
        "year": 2010,
        "ids": {
            "trakt": 16662,
            "slug": "inception-2010",
            "tvdb": null,
            "imdb": "tt1375666",
            "tmdb": 27205,
            "tvrage": null
        },
        "runtime": 150
    }
}"#;

/// Trakt API: Episode watching response
pub const TRAKT_EPISODE_WATCHING: &str = r#"{
    "expires_at": "2024-01-15T11:00:00.000Z",
    "started_at": "2024-01-15T10:00:00.000Z",
    "action": "watching",
    "type": "episode",
    "show": {
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
    },
    "episode": {
        "season": 5,
        "number": 16,
        "title": "Felina",
        "ids": {
            "trakt": 62155,
            "tvdb": 4639461,
            "imdb": "tt2301451",
            "tmdb": 62161,
            "tvrage": null
        },
        "runtime": 60
    }
}"#;

/// Trakt API: Episode watching response with stale started_at and runtime
pub const TRAKT_EPISODE_WATCHING_STALE_START: &str = r#"{
    "expires_at": "2024-01-15T11:00:00.000Z",
    "started_at": "2024-01-15T08:00:00.000Z",
    "action": "checkin",
    "type": "episode",
    "show": {
        "title": "Stargate SG-1",
        "year": 1997,
        "ids": {
            "trakt": 4605,
            "slug": "stargate-sg-1",
            "tvdb": 72449,
            "imdb": "tt0118480",
            "tmdb": 4629,
            "tvrage": null
        },
        "runtime": 44
    },
    "episode": {
        "season": 4,
        "number": 7,
        "title": "Watergate",
        "ids": {
            "trakt": 344183,
            "tvdb": 85823,
            "imdb": "tt0709217",
            "tmdb": 335902,
            "tvrage": null
        },
        "runtime": 44
    }
}"#;

/// Trakt API: Movie ratings response
pub const TRAKT_MOVIE_RATINGS: &str = r#"{
    "rating": 8.45123,
    "votes": 45678,
    "distribution": {
        "1": 100,
        "2": 50,
        "3": 100,
        "4": 200,
        "5": 500,
        "6": 1000,
        "7": 5000,
        "8": 15000,
        "9": 12000,
        "10": 11728
    }
}"#;

/// Trakt API: Device code response
pub const TRAKT_DEVICE_CODE: &str = r#"{
    "device_code": "abc123def456",
    "user_code": "ABCD1234",
    "verification_url": "https://trakt.tv/activate",
    "expires_in": 600,
    "interval": 5
}"#;

/// Trakt API: Access token response
pub const TRAKT_ACCESS_TOKEN: &str = r#"{
    "access_token": "access_token_value",
    "token_type": "Bearer",
    "expires_in": 7776000,
    "refresh_token": "refresh_token_value",
    "scope": "public",
    "created_at": 1705312800
}"#;

/// TMDB API: Movie images response
pub const TMDB_MOVIE_IMAGES: &str = r#"{
    "id": 27205,
    "posters": [
        {
            "aspect_ratio": 0.667,
            "height": 1500,
            "file_path": "/oYuLEt3zVCKq57qu2F8dT7NIa6f.jpg",
            "vote_average": 5.318,
            "width": 1000
        }
    ]
}"#;

/// TMDB API: TV show season images response
pub const TMDB_SHOW_IMAGES: &str = r#"{
    "id": 1396,
    "posters": [
        {
            "aspect_ratio": 0.667,
            "height": 750,
            "file_path": "/zzWGRw277MNoCs3zhyG3YmYQsXv.jpg",
            "vote_average": 5.5,
            "width": 500
        }
    ]
}"#;

/// TMDB API: Empty images response (no posters found)
pub const TMDB_EMPTY_IMAGES: &str = r#"{
    "id": 12345,
    "posters": []
}"#;

/// Sample credentials.ini content for config tests
pub const SAMPLE_CONFIG_INI: &str = r#"[Trakt API]
traktUser = testuser
traktClientID = test_client_id
traktClientSecret =
enabledOAuth = false
OAuthAccessToken =
OAuthRefreshToken =
OAuthRefreshTokenExpiresAt =
"#;

/// Sample credentials.ini with OAuth enabled
pub const SAMPLE_CONFIG_INI_OAUTH: &str = r#"[Trakt API]
traktUser = testuser
traktClientID = test_client_id
traktClientSecret =
enabledOAuth = true
OAuthAccessToken = test_access_token
OAuthRefreshToken = test_refresh_token
OAuthRefreshTokenExpiresAt = 9999999999
"#;

/// TMDB API: Movie details response (for title localization)
pub const TMDB_MOVIE_DETAILS: &str = r#"{
    "id": 27205,
    "title": "Inception",
    "original_title": "Inception",
    "overview": "A thief who steals corporate secrets through use of dream-sharing technology...",
    "release_date": "2010-07-16"
}"#;

/// TMDB API: Movie details response in French
pub const TMDB_MOVIE_DETAILS_FR: &str = r#"{
    "id": 27205,
    "title": "Inception",
    "original_title": "Inception",
    "overview": "Dom Cobb est un voleur expérimenté...",
    "release_date": "2010-07-21"
}"#;

/// TMDB API: TV show details response
pub const TMDB_SHOW_DETAILS: &str = r#"{
    "id": 1396,
    "name": "Breaking Bad",
    "original_name": "Breaking Bad",
    "overview": "A high school chemistry teacher diagnosed with inoperable lung cancer...",
    "first_air_date": "2008-01-20"
}"#;

/// TMDB API: TV episode details response
pub const TMDB_EPISODE_DETAILS: &str = r#"{
    "id": 62161,
    "name": "Felina",
    "overview": "All bad things must come to an end.",
    "air_date": "2013-09-29",
    "season_number": 5,
    "episode_number": 16
}"#;

// Tests for the "Login with Plex" PIN flow and server discovery.

use discrakt::plex_auth::{
    build_auth_url, discover_plex_server, fetch_plex_username, poll_plex_pin, request_plex_pin,
    PlexPinPoll, PlexServer,
};

#[test]
fn test_build_auth_url_embeds_client_and_code() {
    let url = build_auth_url("cid-123", "abcd");
    assert!(url.starts_with("https://app.plex.tv/auth#?"));
    assert!(url.contains("clientID=cid-123"));
    assert!(url.contains("code=abcd"));
    // product is inside a URL-encoded context[device][product] key
    assert!(url.contains("product%5D=Discrakt"));
}

#[test]
fn test_request_plex_pin() {
    let mut server = mockito::Server::new();
    let mock = server
        .mock("POST", "/api/v2/pins")
        .match_query(mockito::Matcher::UrlEncoded("strong".into(), "true".into()))
        .match_header("x-plex-client-identifier", "cid-1")
        .with_status(201)
        .with_header("content-type", "application/json")
        .with_body(r#"{"id": 42, "code": "longcode", "expiresIn": 1800, "authToken": null}"#)
        .create();

    let pin = request_plex_pin("cid-1", Some(&server.url())).expect("pin");
    mock.assert();
    assert_eq!(pin.id, 42);
    assert_eq!(pin.code, "longcode");
    assert_eq!(pin.expires_in, 1800);
    assert_eq!(pin.auth_token, None);
}

#[test]
fn test_poll_plex_pin_pending_then_authorized() {
    let mut server = mockito::Server::new();

    let pending = server
        .mock("GET", "/api/v2/pins/42")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"id": 42, "code": "c", "expiresIn": 1800, "authToken": null}"#)
        .create();
    let result = poll_plex_pin("cid-1", 42, Some(&server.url()));
    pending.assert();
    assert!(matches!(result, PlexPinPoll::Pending));

    let authorized = server
        .mock("GET", "/api/v2/pins/42")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"id": 42, "code": "c", "expiresIn": 1800, "authToken": "tok-xyz"}"#)
        .create();
    let result = poll_plex_pin("cid-1", 42, Some(&server.url()));
    authorized.assert();
    match result {
        PlexPinPoll::Authorized(token) => assert_eq!(token, "tok-xyz"),
        other => panic!("expected Authorized, got {other:?}"),
    }
}

#[test]
fn test_poll_plex_pin_expired() {
    let mut server = mockito::Server::new();
    server
        .mock("GET", "/api/v2/pins/99")
        .with_status(404)
        .create();
    let result = poll_plex_pin("cid-1", 99, Some(&server.url()));
    match result {
        PlexPinPoll::Error(e) => assert!(e.contains("expired")),
        other => panic!("expected Error(expired), got {other:?}"),
    }
}

#[test]
fn test_discover_plex_server_skips_unreachable_and_picks_reachable() {
    let mut server = mockito::Server::new();
    let reachable = server.url(); // the mock server is reachable
                                  // Higher-preference (local) connection points at a dead port; the reachable
                                  // one is lower-preference. Probing must override the score.
    let body = format!(
        r#"[
        {{"name":"Client","provides":"client","owned":true,"connections":[]}},
        {{"name":"My Server","provides":"server","owned":true,"accessToken":"srvtoken",
         "connections":[
            {{"uri":"http://127.0.0.1:1","local":true,"relay":false}},
            {{"uri":"{reachable}","local":false,"relay":false}}
         ]}}
    ]"#
    );
    server
        .mock("GET", "/api/v2/resources")
        .match_query(mockito::Matcher::Any)
        .match_header("x-plex-token", "tok")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(body)
        .create();
    // Reachability probe hits /identity on the reachable connection.
    server.mock("GET", "/identity").with_status(200).create();

    let result = discover_plex_server("tok", "cid", Some(&server.url())).expect("server");
    assert_eq!(
        result,
        PlexServer {
            uri: reachable,
            access_token: "srvtoken".to_string(),
        }
    );
}

#[test]
fn test_discover_plex_server_falls_back_to_best_when_none_reachable() {
    let mut server = mockito::Server::new();
    // Both connections are dead; the highest-scored (local) is returned as a
    // best-effort fallback so a config is still written.
    let body = r#"[
        {"name":"My Server","provides":"server","owned":true,"accessToken":"srvtoken",
         "connections":[
            {"uri":"http://127.0.0.1:2","local":false,"relay":true},
            {"uri":"http://127.0.0.1:1","local":true,"relay":false}
         ]}
    ]"#;
    server
        .mock("GET", "/api/v2/resources")
        .match_query(mockito::Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(body)
        .create();

    let result = discover_plex_server("tok", "cid", Some(&server.url())).expect("server");
    assert_eq!(result.uri, "http://127.0.0.1:1"); // local scored highest
    assert_eq!(result.access_token, "srvtoken");
}

#[test]
fn test_discover_plex_server_errors_when_no_servers() {
    let mut server = mockito::Server::new();
    server
        .mock("GET", "/api/v2/resources")
        .match_query(mockito::Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"[{"name":"Phone","provides":"client","connections":[]}]"#)
        .create();

    assert!(discover_plex_server("tok", "cid", Some(&server.url())).is_err());
}

#[test]
fn test_fetch_plex_username() {
    let mut server = mockito::Server::new();
    server
        .mock("GET", "/api/v2/user")
        .match_header("x-plex-token", "tok")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"username": "alice", "title": "Alice A."}"#)
        .create();

    assert_eq!(
        fetch_plex_username("tok", "cid", Some(&server.url())),
        Some("alice".to_string())
    );
}

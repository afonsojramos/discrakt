// Tests for the Jellyfin Quick Connect flow.

use discrakt::jellyfin_auth::{
    authenticate_with_quick_connect, auth_header, initiate_quick_connect, poll_quick_connect,
    JellyfinAuth, QuickConnectPoll,
};

#[test]
fn test_auth_header_includes_client_and_token() {
    let header = auth_header("dev-1", Some("tok"));
    assert!(header.starts_with("MediaBrowser "));
    assert!(header.contains("Client=\"Discrakt\""));
    assert!(header.contains("DeviceId=\"dev-1\""));
    assert!(header.contains("Token=\"tok\""));

    let no_token = auth_header("dev-1", None);
    assert!(!no_token.contains("Token="));
}

#[test]
fn test_initiate_quick_connect() {
    let mut server = mockito::Server::new();
    let mock = server
        .mock("GET", "/QuickConnect/Initiate")
        .match_header("authorization", mockito::Matcher::Regex("MediaBrowser".into()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"Secret": "sec-1", "Code": "123456", "Authenticated": false}"#)
        .create();

    let state = initiate_quick_connect(&server.url(), "dev-1").expect("initiate");
    mock.assert();
    assert_eq!(state.secret, "sec-1");
    assert_eq!(state.code, "123456");
    assert!(!state.authenticated);
}

#[test]
fn test_poll_quick_connect_pending_then_authorized() {
    let mut server = mockito::Server::new();

    let pending = server
        .mock("GET", "/QuickConnect/Connect")
        .match_query(mockito::Matcher::UrlEncoded("secret".into(), "sec-1".into()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"Secret": "sec-1", "Code": "123456", "Authenticated": false}"#)
        .create();
    assert!(matches!(
        poll_quick_connect(&server.url(), "sec-1"),
        QuickConnectPoll::Pending
    ));
    pending.assert();

    let approved = server
        .mock("GET", "/QuickConnect/Connect")
        .match_query(mockito::Matcher::UrlEncoded("secret".into(), "sec-1".into()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"Secret": "sec-1", "Code": "123456", "Authenticated": true}"#)
        .create();
    assert!(matches!(
        poll_quick_connect(&server.url(), "sec-1"),
        QuickConnectPoll::Authorized
    ));
    approved.assert();
}

#[test]
fn test_authenticate_with_quick_connect() {
    let mut server = mockito::Server::new();
    let mock = server
        .mock("POST", "/Users/AuthenticateWithQuickConnect")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"AccessToken": "tok-xyz", "User": {"Id": "u-1", "Name": "alice"}}"#)
        .create();

    let auth = authenticate_with_quick_connect(&server.url(), "dev-1", "sec-1").expect("auth");
    mock.assert();
    assert_eq!(
        auth,
        JellyfinAuth {
            access_token: "tok-xyz".to_string(),
            user_id: "u-1".to_string(),
            username: "alice".to_string(),
        }
    );
}

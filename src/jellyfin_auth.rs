//! Jellyfin Quick Connect authentication.
//!
//! Quick Connect mirrors the Plex/Trakt device flows: request a code, the user
//! enters it in their Jellyfin web UI (Settings -> Quick Connect), then we poll
//! until it's approved and exchange the secret for an access token. Unlike Plex,
//! Quick Connect is per-server, so every call targets the user's server URL.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use ureq::Agent;

use crate::utils::user_agent;

/// Product/client name reported to Jellyfin.
pub const JELLYFIN_CLIENT: &str = "Discrakt";

/// Generates a stable-enough device identifier for a login attempt.
pub fn generate_device_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("discrakt-{nanos:x}")
}

/// Builds the `Authorization: MediaBrowser ...` header Jellyfin expects.
pub fn auth_header(device_id: &str, token: Option<&str>) -> String {
    let version = env!("CARGO_PKG_VERSION");
    let mut header = format!(
        "MediaBrowser Client=\"{JELLYFIN_CLIENT}\", Device=\"{JELLYFIN_CLIENT}\", DeviceId=\"{device_id}\", Version=\"{version}\""
    );
    if let Some(token) = token {
        header.push_str(&format!(", Token=\"{token}\""));
    }
    header
}

fn jellyfin_agent() -> Agent {
    Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(15)))
        .user_agent(user_agent())
        .build()
        .into()
}

/// A Quick Connect request in progress.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct QuickConnectState {
    pub secret: String,
    pub code: String,
    #[serde(default)]
    pub authenticated: bool,
}

/// Result of polling a Quick Connect request.
#[derive(Debug, Clone)]
pub enum QuickConnectPoll {
    /// The user approved the code.
    Authorized,
    /// Still waiting for approval.
    Pending,
    /// A network or server error occurred.
    Error(String),
}

/// The credentials obtained after a successful Quick Connect.
#[derive(Debug, Clone, PartialEq)]
pub struct JellyfinAuth {
    pub access_token: String,
    pub user_id: String,
    pub username: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct AuthResult {
    access_token: String,
    user: JellyfinUser,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct JellyfinUser {
    id: String,
    name: String,
}

/// Initiates a Quick Connect request, returning the secret and the code the
/// user must enter in their Jellyfin web UI.
pub fn initiate_quick_connect(
    server_url: &str,
    device_id: &str,
) -> Result<QuickConnectState, String> {
    let base = server_url.trim_end_matches('/');
    let agent = jellyfin_agent();

    let response = agent
        .get(&format!("{base}/QuickConnect/Initiate"))
        .header("Accept", "application/json")
        .header("Authorization", &auth_header(device_id, None))
        .call();

    match response {
        Ok(mut resp) => resp
            .body_mut()
            .read_json::<QuickConnectState>()
            .map_err(|e| format!("Failed to parse Quick Connect response: {e}")),
        Err(ureq::Error::StatusCode(401 | 403)) => {
            Err("Quick Connect is not enabled on this server".to_string())
        }
        Err(ureq::Error::StatusCode(code)) => Err(format!("HTTP {code}")),
        Err(e) => Err(format!("Network error: {e}")),
    }
}

/// Polls a Quick Connect request once to see whether the user has approved it.
pub fn poll_quick_connect(server_url: &str, secret: &str) -> QuickConnectPoll {
    let base = server_url.trim_end_matches('/');
    let agent = jellyfin_agent();

    let response = agent
        .get(&format!("{base}/QuickConnect/Connect?secret={secret}"))
        .header("Accept", "application/json")
        .call();

    match response {
        Ok(mut resp) => match resp.body_mut().read_json::<QuickConnectState>() {
            Ok(state) if state.authenticated => QuickConnectPoll::Authorized,
            Ok(_) => QuickConnectPoll::Pending,
            Err(e) => QuickConnectPoll::Error(format!("Failed to parse Quick Connect: {e}")),
        },
        Err(ureq::Error::StatusCode(404)) => {
            QuickConnectPoll::Error("Quick Connect code expired".to_string())
        }
        Err(ureq::Error::StatusCode(code)) => QuickConnectPoll::Error(format!("HTTP {code}")),
        Err(e) => QuickConnectPoll::Error(format!("Network error: {e}")),
    }
}

/// Exchanges an approved Quick Connect secret for an access token + user id.
pub fn authenticate_with_quick_connect(
    server_url: &str,
    device_id: &str,
    secret: &str,
) -> Result<JellyfinAuth, String> {
    let base = server_url.trim_end_matches('/');
    let agent = jellyfin_agent();

    let response = agent
        .post(&format!("{base}/Users/AuthenticateWithQuickConnect"))
        .header("Accept", "application/json")
        .header("Authorization", &auth_header(device_id, None))
        .send_json(serde_json::json!({ "Secret": secret }));

    match response {
        Ok(mut resp) => resp
            .body_mut()
            .read_json::<AuthResult>()
            .map(|result| JellyfinAuth {
                access_token: result.access_token,
                user_id: result.user.id,
                username: result.user.name,
            })
            .map_err(|e| format!("Failed to parse auth response: {e}")),
        Err(ureq::Error::StatusCode(code)) => Err(format!("HTTP {code}")),
        Err(e) => Err(format!("Network error: {e}")),
    }
}

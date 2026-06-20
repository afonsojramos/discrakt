//! "Login with Plex" PIN flow and server discovery.
//!
//! Mirrors the structure of the Trakt device flow (request a code, poll until the
//! user authorizes, exchange for a token), then discovers the user's server so the
//! setup wizard can fill in the server URL and token automatically.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use ureq::Agent;

use crate::utils::user_agent;

/// Base URL for plex.tv account APIs.
pub const PLEX_TV_BASE_URL: &str = "https://plex.tv";

/// Product name reported to Plex (shown on the authorized-devices page).
pub const PLEX_PRODUCT: &str = "Discrakt";

/// A Plex login PIN.
#[derive(Deserialize, Debug, Clone)]
pub struct PlexPin {
    pub id: u64,
    pub code: String,
    #[serde(rename = "expiresIn")]
    pub expires_in: u64,
    #[serde(rename = "authToken")]
    pub auth_token: Option<String>,
}

/// Result of polling a login PIN.
#[derive(Debug, Clone)]
pub enum PlexPinPoll {
    /// The user authorized; contains the account auth token.
    Authorized(String),
    /// Still waiting for the user to authorize.
    Pending,
    /// A network or server error occurred.
    Error(String),
}

/// A discovered Plex server connection.
#[derive(Debug, Clone, PartialEq)]
pub struct PlexServer {
    /// A connectable URL (prefers `*.plex.direct` HTTPS so TLS verifies).
    pub uri: String,
    /// The per-server access token.
    pub access_token: String,
}

/// Generates a reasonably unique client identifier for this login attempt.
///
/// Plex requires a consistent `X-Plex-Client-Identifier` across the PIN request
/// and its polls; deriving it from the current time is sufficient for that.
pub fn generate_client_identifier() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("discrakt-{nanos:x}")
}

/// Builds the browser URL the user visits to approve the login.
pub fn build_auth_url(client_id: &str, code: &str) -> String {
    format!(
        "https://app.plex.tv/auth#?clientID={client_id}&code={code}&context%5Bdevice%5D%5Bproduct%5D={PLEX_PRODUCT}"
    )
}

fn plex_agent() -> Agent {
    Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(20)))
        .user_agent(user_agent())
        .build()
        .into()
}

/// Requests a new login PIN from Plex.
pub fn request_plex_pin(client_id: &str, base_url: Option<&str>) -> Result<PlexPin, String> {
    let base = base_url.unwrap_or(PLEX_TV_BASE_URL);
    let agent = plex_agent();

    let response = agent
        .post(&format!("{base}/api/v2/pins?strong=true"))
        .header("Accept", "application/json")
        .header("X-Plex-Product", PLEX_PRODUCT)
        .header("X-Plex-Client-Identifier", client_id)
        .send_empty();

    match response {
        Ok(mut resp) => resp
            .body_mut()
            .read_json::<PlexPin>()
            .map_err(|e| format!("Failed to parse PIN response: {e}")),
        Err(ureq::Error::StatusCode(code)) => Err(format!("HTTP {code}")),
        Err(e) => Err(format!("Network error: {e}")),
    }
}

/// Polls a login PIN once to see whether the user has authorized yet.
pub fn poll_plex_pin(client_id: &str, pin_id: u64, base_url: Option<&str>) -> PlexPinPoll {
    let base = base_url.unwrap_or(PLEX_TV_BASE_URL);
    let agent = plex_agent();

    let response = agent
        .get(&format!("{base}/api/v2/pins/{pin_id}"))
        .header("Accept", "application/json")
        .header("X-Plex-Client-Identifier", client_id)
        .call();

    match response {
        Ok(mut resp) => match resp.body_mut().read_json::<PlexPin>() {
            Ok(pin) => match pin.auth_token {
                Some(token) if !token.is_empty() => PlexPinPoll::Authorized(token),
                _ => PlexPinPoll::Pending,
            },
            Err(e) => PlexPinPoll::Error(format!("Failed to parse PIN: {e}")),
        },
        Err(ureq::Error::StatusCode(404)) => {
            PlexPinPoll::Error("PIN expired or not found".to_string())
        }
        Err(ureq::Error::StatusCode(code)) => PlexPinPoll::Error(format!("HTTP {code}")),
        Err(e) => PlexPinPoll::Error(format!("Network error: {e}")),
    }
}

#[derive(Deserialize)]
struct Resource {
    #[serde(default)]
    provides: String,
    #[serde(rename = "accessToken")]
    access_token: Option<String>,
    #[serde(default)]
    owned: bool,
    #[serde(default)]
    connections: Vec<Connection>,
}

#[derive(Deserialize)]
struct Connection {
    uri: String,
    #[serde(default)]
    local: bool,
    #[serde(default)]
    relay: bool,
}

/// Scores a connection for preference order: prefer owned, non-relay, local,
/// HTTPS. This only orders probing; reachability is what ultimately decides.
fn connection_score(owned: bool, conn: &Connection) -> u8 {
    let mut score = 0;
    if owned {
        score += 8;
    }
    if !conn.relay {
        score += 4;
    }
    if conn.local {
        score += 2;
    }
    if conn.uri.starts_with("https://") {
        score += 1;
    }
    score
}

/// Returns true if the connection responds at all (any HTTP status counts as
/// reachable; only network/timeout failures count as unreachable).
fn connection_reachable(agent: &Agent, uri: &str, token: &str) -> bool {
    match agent
        .get(&format!("{uri}/identity"))
        .header("X-Plex-Token", token)
        .call()
    {
        Ok(_) | Err(ureq::Error::StatusCode(_)) => true,
        Err(_) => false,
    }
}

/// Discovers a reachable Plex server connection for the authenticated account.
///
/// Plex advertises several connections (LAN, WAN, relay) without knowing which
/// the client can actually reach, so we order them by preference and then probe
/// each, returning the first that responds. A higher-preference connection that
/// times out (e.g. a LAN address from off the network) is skipped.
pub fn discover_plex_server(
    auth_token: &str,
    client_id: &str,
    base_url: Option<&str>,
) -> Result<PlexServer, String> {
    let base = base_url.unwrap_or(PLEX_TV_BASE_URL);
    let agent = plex_agent();

    let response = agent
        .get(&format!(
            "{base}/api/v2/resources?includeHttps=1&includeRelay=1"
        ))
        .header("Accept", "application/json")
        .header("X-Plex-Token", auth_token)
        .header("X-Plex-Client-Identifier", client_id)
        .call();

    let resources: Vec<Resource> = match response {
        Ok(mut resp) => resp
            .body_mut()
            .read_json()
            .map_err(|e| format!("Failed to parse resources: {e}"))?,
        Err(ureq::Error::StatusCode(code)) => return Err(format!("HTTP {code}")),
        Err(e) => return Err(format!("Network error: {e}")),
    };

    // Collect (score, uri, token) candidates, ordered by preference.
    let mut candidates: Vec<(u8, String, String)> = resources
        .iter()
        .filter(|r| r.provides.split(',').any(|p| p.trim() == "server"))
        .filter_map(|r| r.access_token.as_ref().map(|token| (r, token)))
        .flat_map(|(r, token)| {
            r.connections.iter().map(move |conn| {
                (
                    connection_score(r.owned, conn),
                    conn.uri.clone(),
                    token.clone(),
                )
            })
        })
        .collect();

    if candidates.is_empty() {
        return Err("No Plex servers found for this account".to_string());
    }
    candidates.sort_by_key(|b| std::cmp::Reverse(b.0));

    // Probe each in preference order; use the first that responds.
    let probe = Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(3)))
        .user_agent(user_agent())
        .build()
        .into();
    for (_, uri, token) in &candidates {
        if connection_reachable(&probe, uri, token) {
            return Ok(PlexServer {
                uri: uri.clone(),
                access_token: token.clone(),
            });
        }
    }

    // Nothing responded; fall back to the most-preferred so config is still
    // written (the user may be able to reach it later, or edit it manually).
    let (_, uri, token) = &candidates[0];
    tracing::warn!("No Plex connection responded to probing; using {}", uri);
    Ok(PlexServer {
        uri: uri.clone(),
        access_token: token.clone(),
    })
}

#[derive(Deserialize)]
struct PlexUser {
    #[serde(default)]
    username: String,
    #[serde(default)]
    title: String,
}

/// Fetches the authenticated account's username (used to filter sessions).
pub fn fetch_plex_username(
    auth_token: &str,
    client_id: &str,
    base_url: Option<&str>,
) -> Option<String> {
    let base = base_url.unwrap_or(PLEX_TV_BASE_URL);
    let agent = plex_agent();

    let response = agent
        .get(&format!("{base}/api/v2/user"))
        .header("Accept", "application/json")
        .header("X-Plex-Token", auth_token)
        .header("X-Plex-Client-Identifier", client_id)
        .call();

    let user: PlexUser = match response {
        Ok(mut resp) => resp.body_mut().read_json().ok()?,
        Err(_) => return None,
    };

    if !user.username.is_empty() {
        Some(user.username)
    } else if !user.title.is_empty() {
        Some(user.title)
    } else {
        None
    }
}

//! Local HTTP server for browser-based credential setup.

use std::net::{SocketAddr, TcpListener};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use configparser::ini::Ini;
use serde::{Deserialize, Serialize};
use tiny_http::{Response, Server, StatusCode};

use super::html;
use crate::utils::{
    poll_device_token, request_device_code, save_oauth_tokens, set_restrictive_permissions,
    DeviceTokenPollResult, TraktDeviceCode, DEFAULT_TRAKT_CLIENT_ID,
};

/// Maximum number of consecutive network errors before giving up.
const MAX_NETWORK_ERRORS: u32 = 10;

/// Maximum request body size (64KB limit).
const MAX_BODY_SIZE: usize = 64 * 1024;

/// Result of the setup process.
#[derive(Debug, Clone)]
pub struct SetupResult {
    /// Trakt username
    pub trakt_username: String,
    /// Trakt Client ID
    pub trakt_client_id: String,
}

/// Credentials submitted via the setup form.
#[derive(Debug, Deserialize)]
struct SubmittedCredentials {
    #[serde(rename = "traktUser")]
    trakt_user: String,
    #[serde(rename = "traktClientID", default)]
    trakt_client_id: String,
}

/// State of the OAuth authorization flow.
#[derive(Debug, Clone)]
enum OAuthState {
    /// No OAuth flow in progress.
    Idle,
    /// Waiting for user to authorize.
    Pending,
    /// User authorized successfully. Contains the time when success was achieved.
    Success(Instant),
    /// User denied authorization.
    Denied,
    /// Device code expired.
    Expired,
    /// An error occurred.
    Error(String),
}

/// Grace period after OAuth success to allow browser to poll and see the success status.
/// This should be longer than the polling interval (5 seconds) to ensure at least one poll.
const SUCCESS_GRACE_PERIOD: Duration = Duration::from_secs(8);

/// Response for device code info sent to the browser.
#[derive(Serialize)]
struct DeviceCodeResponse {
    user_code: String,
    verification_url: String,
    expires_in: u64,
    interval: u64,
}

/// Response for status endpoint.
#[derive(Serialize)]
struct StatusResponse {
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

/// Get the path to the config directory.
///
/// # Errors
///
/// Returns an error if the platform's config directory cannot be determined.
fn config_dir_path() -> Result<PathBuf, String> {
    dirs::config_dir()
        .map(|p| p.join("discrakt"))
        .ok_or_else(|| "Could not determine config directory".to_string())
}

/// Write credentials to the config file.
fn write_credentials(creds: &SubmittedCredentials) -> Result<PathBuf, String> {
    let config_dir = config_dir_path()?;

    // Create config directory if it doesn't exist
    if !config_dir.exists() {
        std::fs::create_dir_all(&config_dir)
            .map_err(|e| format!("Failed to create config directory: {e}"))?;
    }

    let config_path = config_dir.join("credentials.ini");

    // Check if config already exists
    let mut config = Ini::new_cs();
    if config_path.exists() {
        let _ = config.load(&config_path);
    }

    // Set the required fields
    config.setstr("Trakt API", "traktUser", Some(&creds.trakt_user));
    config.setstr("Trakt API", "traktClientID", Some(&creds.trakt_client_id));

    // Set default OAuth settings if not already present
    // Enable OAuth by default so the OAuth flow starts after setup completes
    if config.get("Trakt API", "enabledOAuth").is_none() {
        config.setstr("Trakt API", "enabledOAuth", Some("true"));
    }
    if config.get("Trakt API", "traktClientSecret").is_none() {
        config.setstr("Trakt API", "traktClientSecret", Some(""));
    }
    if config.get("Trakt API", "OAuthAccessToken").is_none() {
        config.setstr("Trakt API", "OAuthAccessToken", Some(""));
    }
    if config.get("Trakt API", "OAuthRefreshToken").is_none() {
        config.setstr("Trakt API", "OAuthRefreshToken", Some(""));
    }
    if config
        .get("Trakt API", "OAuthRefreshTokenExpiresAt")
        .is_none()
    {
        config.setstr("Trakt API", "OAuthRefreshTokenExpiresAt", Some(""));
    }

    config
        .write(&config_path)
        .map_err(|e| format!("Failed to write config file: {e}"))?;

    set_restrictive_permissions(&config_path);

    tracing::info!("Credentials saved to {:?}", config_path);
    Ok(config_path)
}

/// Find an available port for the server.
fn find_available_port() -> Option<u16> {
    // Try to bind to port 0, which lets the OS assign an available port
    TcpListener::bind("127.0.0.1:0")
        .ok()
        .and_then(|listener| listener.local_addr().ok())
        .map(|addr| addr.port())
}

/// Run the setup server and wait for credentials to be submitted.
///
/// This function:
/// 1. Starts a local HTTP server on a random port
/// 2. Opens the default browser to the setup page
/// 3. Waits for the user to submit credentials
/// 4. Starts the OAuth device flow
/// 5. Polls for OAuth authorization in the background
/// 6. Returns the setup result once authorized
///
/// # Errors
///
/// Returns an error if:
/// - The server fails to start
/// - The browser fails to open
/// - Writing credentials fails
/// - OAuth authorization fails
#[allow(clippy::too_many_lines)]
pub fn run_setup_server() -> Result<SetupResult, Box<dyn std::error::Error>> {
    let port = find_available_port().ok_or("Failed to find available port")?;
    let addr: SocketAddr = format!("127.0.0.1:{port}").parse()?;

    let server = Server::http(addr).map_err(|e| format!("Failed to start HTTP server: {e}"))?;

    tracing::info!("Setup server started at http://{}", addr);

    // Flag to signal when setup is complete
    let setup_complete = Arc::new(AtomicBool::new(false));
    let result: Arc<Mutex<Option<SetupResult>>> = Arc::new(Mutex::new(None));
    let oauth_state: Arc<Mutex<OAuthState>> = Arc::new(Mutex::new(OAuthState::Idle));
    // Track if a polling thread is already running to prevent duplicate spawns
    let polling_started = Arc::new(AtomicBool::new(false));

    // Open browser to setup page
    let url = format!("http://127.0.0.1:{port}");
    tracing::info!("Opening browser to {}", url);

    if webbrowser::open(&url).is_err() {
        tracing::warn!("Failed to open browser automatically");
        println!("\n========================================");
        println!("  Discrakt Setup");
        println!("========================================\n");
        println!("Please open your browser and navigate to:");
        println!("  {url}\n");
    }

    // Handle requests until setup is complete and grace period has passed.
    loop {
        // Check if setup is complete and grace period has elapsed
        // This allows the browser to poll /status and see the success state
        if setup_complete.load(Ordering::SeqCst) {
            if let Ok(state) = oauth_state.lock() {
                if let OAuthState::Success(success_time) = *state {
                    if success_time.elapsed() >= SUCCESS_GRACE_PERIOD {
                        break;
                    }
                }
            }
        }

        // Wait for a request with a short timeout so we can check setup_complete periodically
        let request = match server.recv_timeout(Duration::from_millis(500)) {
            Ok(Some(req)) => req,
            Ok(None) => continue, // Timeout, check setup_complete and try again
            Err(e) => {
                tracing::error!("Error receiving request: {}", e);
                continue;
            }
        };

        let mut request = request;
        let url = request.url().to_string();
        let method = request.method().to_string();

        tracing::debug!("Received {} request for {}", method, url);

        match (method.as_str(), url.as_str()) {
            ("GET", "/" | "/index.html") => {
                let html = html::setup_page();
                let response = Response::from_string(html).with_header(
                    tiny_http::Header::from_bytes(
                        &b"Content-Type"[..],
                        &b"text/html; charset=utf-8"[..],
                    )
                    .unwrap(),
                );
                let _ = request.respond(response);
            }

            ("POST", "/submit") => {
                // Check Content-Type header
                let content_type = request
                    .headers()
                    .iter()
                    .find(|h| {
                        h.field
                            .as_str()
                            .as_str()
                            .eq_ignore_ascii_case("content-type")
                    })
                    .map(|h| h.value.as_str().to_string());

                if !content_type
                    .as_ref()
                    .is_some_and(|ct| ct.starts_with("application/json"))
                {
                    let response = Response::from_string("Content-Type must be application/json")
                        .with_status_code(StatusCode(415));
                    let _ = request.respond(response);
                    continue;
                }

                // Check Content-Length header to prevent memory exhaustion
                let content_length = request
                    .headers()
                    .iter()
                    .find(|h| {
                        h.field
                            .as_str()
                            .as_str()
                            .eq_ignore_ascii_case("content-length")
                    })
                    .and_then(|h| h.value.as_str().parse::<usize>().ok());

                if let Some(len) = content_length {
                    if len > MAX_BODY_SIZE {
                        tracing::warn!(
                            "Request body too large: {} bytes (max {})",
                            len,
                            MAX_BODY_SIZE
                        );
                        let response = Response::from_string("Request body too large")
                            .with_status_code(StatusCode(413));
                        let _ = request.respond(response);
                        continue;
                    }
                }

                // Read the request body with size limit
                let body_result: Result<String, (StatusCode, String)> = {
                    let capacity = content_length.unwrap_or(1024).min(MAX_BODY_SIZE);
                    let mut body = Vec::with_capacity(capacity);
                    let reader = request.as_reader();

                    // Read in chunks to enforce size limit
                    let mut buf = [0u8; 4096];
                    let mut read_error = None;
                    loop {
                        match reader.read(&mut buf) {
                            Ok(0) => break, // EOF
                            Ok(n) => {
                                body.extend_from_slice(&buf[..n]);
                                if body.len() > MAX_BODY_SIZE {
                                    tracing::warn!(
                                        "Request body exceeded limit during reading: {} bytes",
                                        body.len()
                                    );
                                    read_error = Some((
                                        StatusCode(413),
                                        "Request body too large".to_string(),
                                    ));
                                    break;
                                }
                            }
                            Err(e) => {
                                tracing::error!("Failed to read request body: {}", e);
                                read_error =
                                    Some((StatusCode(400), "Failed to read request".to_string()));
                                break;
                            }
                        }
                    }

                    match read_error {
                        Some((code, msg)) => Err((code, msg)),
                        None => String::from_utf8(body).map_err(|e| {
                            tracing::error!("Request body is not valid UTF-8: {}", e);
                            (StatusCode(400), "Invalid UTF-8 in request body".to_string())
                        }),
                    }
                };

                let body = match body_result {
                    Ok(b) => b,
                    Err((code, msg)) => {
                        let response = Response::from_string(msg).with_status_code(code);
                        let _ = request.respond(response);
                        continue;
                    }
                };

                tracing::debug!("Received form data: {}", body);

                // Parse the JSON body using serde_json
                let creds: SubmittedCredentials = match serde_json::from_str(&body) {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::error!("Failed to parse JSON: {}", e);
                        let response = Response::from_string(format!("Invalid JSON: {e}"))
                            .with_status_code(StatusCode(400));
                        let _ = request.respond(response);
                        continue;
                    }
                };

                // Validate required fields (only trakt_user is required)
                if creds.trakt_user.is_empty() {
                    let response = Response::from_string("Trakt Username is required")
                        .with_status_code(StatusCode(400));
                    let _ = request.respond(response);
                    continue;
                }

                // Write credentials to config file
                if let Err(e) = write_credentials(&creds) {
                    tracing::error!("Failed to write credentials: {}", e);
                    let response = Response::from_string(format!("Failed to save: {e}"))
                        .with_status_code(StatusCode(500));
                    let _ = request.respond(response);
                    continue;
                }

                // Determine the client ID to use
                let client_id = if creds.trakt_client_id.is_empty() {
                    DEFAULT_TRAKT_CLIENT_ID.to_string()
                } else {
                    creds.trakt_client_id.clone()
                };

                // Store the result (will be returned after OAuth completes)
                if let Ok(mut result_guard) = result.lock() {
                    *result_guard = Some(SetupResult {
                        trakt_username: creds.trakt_user.clone(),
                        trakt_client_id: client_id.clone(),
                    });
                }

                // Start OAuth device flow
                match request_device_code(&client_id, None) {
                    Ok(device_code) => {
                        tracing::info!(
                            user_code = %device_code.user_code,
                            verification_url = %device_code.verification_url,
                            "Device code obtained, waiting for user authorization"
                        );

                        // Store device code info for polling
                        if let Ok(mut state) = oauth_state.lock() {
                            *state = OAuthState::Pending;
                        }

                        // Start background polling thread (only if not already started)
                        if polling_started
                            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                            .is_ok()
                        {
                            let oauth_state_clone = Arc::clone(&oauth_state);
                            let setup_complete_clone = Arc::clone(&setup_complete);
                            let device_code_clone = device_code.clone();
                            let client_id_clone = client_id.clone();

                            thread::spawn(move || {
                                poll_oauth_in_background(
                                    device_code_clone,
                                    client_id_clone,
                                    oauth_state_clone,
                                    setup_complete_clone,
                                );
                            });
                        } else {
                            tracing::warn!(
                                "Polling thread already started, ignoring duplicate request"
                            );
                        }

                        // Send response with device code info
                        let response_data = DeviceCodeResponse {
                            user_code: device_code.user_code,
                            verification_url: device_code.verification_url,
                            expires_in: device_code.expires_in,
                            interval: device_code.interval,
                        };

                        let response_json = serde_json::to_string(&response_data)
                            .unwrap_or_else(|_| r#"{"error":"serialization failed"}"#.to_string());

                        let response = Response::from_string(response_json).with_header(
                            tiny_http::Header::from_bytes(
                                &b"Content-Type"[..],
                                &b"application/json"[..],
                            )
                            .unwrap(),
                        );
                        let _ = request.respond(response);
                    }
                    Err(e) => {
                        tracing::error!("Failed to request device code: {}", e);
                        let response = Response::from_string(format!("OAuth error: {e}"))
                            .with_status_code(StatusCode(500));
                        let _ = request.respond(response);
                    }
                }
            }

            ("GET", "/status") => {
                // Return the current OAuth status
                // On mutex poisoning, reset to Idle state for safety
                let state = match oauth_state.lock() {
                    Ok(s) => s.clone(),
                    Err(poisoned) => {
                        tracing::warn!("OAuth state mutex was poisoned, resetting to Idle state");
                        // Return Idle state instead of potentially corrupted state
                        drop(poisoned.into_inner());
                        OAuthState::Idle
                    }
                };

                let response_data = match state {
                    OAuthState::Idle => StatusResponse {
                        status: "idle".to_string(),
                        message: None,
                    },
                    OAuthState::Pending => StatusResponse {
                        status: "pending".to_string(),
                        message: None,
                    },
                    OAuthState::Success(_) => StatusResponse {
                        status: "success".to_string(),
                        message: None,
                    },
                    OAuthState::Denied => StatusResponse {
                        status: "denied".to_string(),
                        message: None,
                    },
                    OAuthState::Expired => StatusResponse {
                        status: "expired".to_string(),
                        message: None,
                    },
                    OAuthState::Error(msg) => StatusResponse {
                        status: "error".to_string(),
                        message: Some(msg),
                    },
                };

                let response_json = serde_json::to_string(&response_data).unwrap_or_else(|_| {
                    r#"{"status":"error","message":"serialization failed"}"#.to_string()
                });

                let response = Response::from_string(response_json).with_header(
                    tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..])
                        .unwrap(),
                );
                let _ = request.respond(response);
            }

            ("GET", "/favicon.ico" | "/favicon.png") => {
                // Serve the Discrakt icon as favicon
                static ICON_BYTES: &[u8] = include_bytes!("../assets/icon.png");
                let response = Response::from_data(ICON_BYTES).with_header(
                    tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"image/png"[..]).unwrap(),
                );
                let _ = request.respond(response);
            }

            ("GET", "/logo.svg") => {
                // Serve the Discrakt wordmark logo
                static LOGO_BYTES: &[u8] = include_bytes!("../../assets/discrakt-wordmark.svg");
                let response = Response::from_data(LOGO_BYTES).with_header(
                    tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"image/svg+xml"[..])
                        .unwrap(),
                );
                let _ = request.respond(response);
            }

            _ => {
                let response = Response::from_string("Not Found").with_status_code(StatusCode(404));
                let _ = request.respond(response);
            }
        }
    }

    // Wait a bit for responses to be sent
    thread::sleep(Duration::from_millis(500));

    // Return the result
    result
        .lock()
        .ok()
        .and_then(|guard| guard.clone())
        .ok_or_else(|| "Setup was cancelled or failed".into())
}

/// Poll for OAuth authorization in the background.
#[allow(clippy::needless_pass_by_value)]
fn poll_oauth_in_background(
    device_code: TraktDeviceCode,
    client_id: String,
    oauth_state: Arc<Mutex<OAuthState>>,
    setup_complete: Arc<AtomicBool>,
) {
    let start_time = std::time::Instant::now();
    let timeout = Duration::from_secs(device_code.expires_in);
    let mut poll_interval = Duration::from_secs(device_code.interval);
    let mut consecutive_errors: u32 = 0;

    loop {
        // Check if we've exceeded the timeout
        if start_time.elapsed() >= timeout {
            tracing::error!("Device authorization timed out");
            if let Ok(mut state) = oauth_state.lock() {
                *state = OAuthState::Expired;
            }
            return;
        }

        // Wait for the specified interval before polling
        thread::sleep(poll_interval);

        match poll_device_token(&client_id, &device_code.device_code, None) {
            DeviceTokenPollResult::Success(token) => {
                tracing::info!("Successfully obtained OAuth tokens via device flow");

                // Save the tokens to config
                save_oauth_tokens(&token);

                // Update state to success with timestamp so the server knows when
                // to shut down (after grace period for browser to poll)
                if let Ok(mut state) = oauth_state.lock() {
                    *state = OAuthState::Success(Instant::now());
                }

                // Signal that setup is complete (server will wait for grace period)
                setup_complete.store(true, Ordering::SeqCst);
                return;
            }
            DeviceTokenPollResult::Pending => {
                tracing::debug!("Authorization pending, continuing to poll...");
                consecutive_errors = 0; // Reset error counter on successful communication
            }
            DeviceTokenPollResult::Denied => {
                tracing::error!("User denied authorization");
                if let Ok(mut state) = oauth_state.lock() {
                    *state = OAuthState::Denied;
                }
                return;
            }
            DeviceTokenPollResult::Expired => {
                tracing::error!("Device code expired");
                if let Ok(mut state) = oauth_state.lock() {
                    *state = OAuthState::Expired;
                }
                return;
            }
            DeviceTokenPollResult::AlreadyUsed => {
                tracing::error!("Device code already used");
                if let Ok(mut state) = oauth_state.lock() {
                    *state = OAuthState::Error("Device code already used".to_string());
                }
                return;
            }
            DeviceTokenPollResult::InvalidCode => {
                tracing::error!("Invalid device code");
                if let Ok(mut state) = oauth_state.lock() {
                    *state = OAuthState::Error("Invalid device code".to_string());
                }
                return;
            }
            DeviceTokenPollResult::SlowDown => {
                tracing::warn!("Rate limited, slowing down polling");
                poll_interval = poll_interval.saturating_mul(2).min(Duration::from_secs(30));
                consecutive_errors = 0;
            }
            DeviceTokenPollResult::Error(e) => {
                consecutive_errors += 1;
                tracing::error!(
                    "Error during token poll ({}/{}): {}",
                    consecutive_errors,
                    MAX_NETWORK_ERRORS,
                    e
                );

                // After too many consecutive errors, give up
                if consecutive_errors >= MAX_NETWORK_ERRORS {
                    tracing::error!("Too many consecutive network errors, giving up");
                    if let Ok(mut state) = oauth_state.lock() {
                        *state = OAuthState::Error("Network connectivity issues".to_string());
                    }
                    return;
                }

                // Exponential backoff for network errors
                poll_interval = poll_interval.saturating_mul(2).min(Duration::from_secs(30));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_json_body() {
        let json =
            r#"{"traktUser":"testuser","traktClientID":"abc123def456","discordApplicationID":""}"#;
        let result: SubmittedCredentials = serde_json::from_str(json).unwrap();
        assert_eq!(result.trakt_user, "testuser");
        assert_eq!(result.trakt_client_id, "abc123def456");
    }

    #[test]
    fn test_parse_json_body_with_escaped_chars() {
        let json = r#"{"traktUser":"test\"user","traktClientID":"abc"}"#;
        let result: SubmittedCredentials = serde_json::from_str(json).unwrap();
        assert_eq!(result.trakt_user, "test\"user");
    }

    #[test]
    fn test_parse_json_body_with_empty_client_id() {
        let json = r#"{"traktUser":"testuser","traktClientID":""}"#;
        let result: SubmittedCredentials = serde_json::from_str(json).unwrap();
        assert_eq!(result.trakt_user, "testuser");
        assert_eq!(result.trakt_client_id, ""); // Empty client ID is allowed
    }

    #[test]
    fn test_parse_json_body_without_client_id() {
        let json = r#"{"traktUser":"testuser"}"#;
        let result: SubmittedCredentials = serde_json::from_str(json).unwrap();
        assert_eq!(result.trakt_user, "testuser");
        assert_eq!(result.trakt_client_id, ""); // Missing client ID defaults to empty
    }

    #[test]
    fn test_status_response_serialization() {
        let response = StatusResponse {
            status: "success".to_string(),
            message: None,
        };
        let json = serde_json::to_string(&response).unwrap();
        assert_eq!(json, r#"{"status":"success"}"#);

        let response_with_msg = StatusResponse {
            status: "error".to_string(),
            message: Some("Something went wrong".to_string()),
        };
        let json = serde_json::to_string(&response_with_msg).unwrap();
        assert!(json.contains("\"status\":\"error\""));
        assert!(json.contains("\"message\":\"Something went wrong\""));
    }

    #[test]
    fn test_device_code_response_serialization() {
        let response = DeviceCodeResponse {
            user_code: "ABC123".to_string(),
            verification_url: "https://trakt.tv/activate".to_string(),
            expires_in: 600,
            interval: 5,
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"user_code\":\"ABC123\""));
        assert!(json.contains("\"expires_in\":600"));
        assert!(json.contains("\"interval\":5"));
    }
}

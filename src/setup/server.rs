//! Local HTTP server for browser-based credential setup.

use std::net::{SocketAddr, TcpListener};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use configparser::ini::Ini;
use tiny_http::{Response, Server, StatusCode};

use super::html;
use crate::utils::{
    poll_device_token, request_device_code, save_oauth_tokens, DeviceTokenPollResult,
    TraktDeviceCode, DEFAULT_TRAKT_CLIENT_ID,
};

/// Default Discord Application ID for Discrakt.
const DEFAULT_DISCORD_APP_ID: &str = "826189107046121572";

/// Result of the setup process.
#[derive(Debug, Clone)]
pub struct SetupResult {
    /// Trakt username
    pub trakt_username: String,
    /// Trakt Client ID
    pub trakt_client_id: String,
    /// Discord Application ID (uses default if not provided)
    pub discord_app_id: String,
}

/// Credentials submitted via the setup form.
#[derive(Debug)]
struct SubmittedCredentials {
    trakt_user: String,
    trakt_client_id: String,
    discord_app_id: Option<String>,
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

/// Parse JSON body from the form submission.
fn parse_json_body(body: &str) -> Option<SubmittedCredentials> {
    // Simple JSON parsing without pulling in serde_json
    // Expected format: {"traktUser":"...","traktClientID":"...","discordApplicationID":"..."}

    let body = body.trim();
    if !body.starts_with('{') || !body.ends_with('}') {
        return None;
    }

    let inner = &body[1..body.len() - 1];

    let mut trakt_user = None;
    let mut trakt_client_id = None;
    let mut discord_app_id = None;

    // Split by comma, handling quoted strings
    for part in split_json_fields(inner) {
        let part = part.trim();
        if let Some((key, value)) = parse_json_field(part) {
            match key {
                "traktUser" => trakt_user = Some(value),
                "traktClientID" => trakt_client_id = Some(value),
                "discordApplicationID" => {
                    if !value.is_empty() {
                        discord_app_id = Some(value);
                    }
                }
                _ => {}
            }
        }
    }

    Some(SubmittedCredentials {
        trakt_user: trakt_user?,
        // trakt_client_id can be empty or missing; default to empty string
        trakt_client_id: trakt_client_id.unwrap_or_default(),
        discord_app_id,
    })
}

/// Split JSON fields by comma, respecting quoted strings.
fn split_json_fields(s: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    let mut escape_next = false;

    for c in s.chars() {
        if escape_next {
            current.push(c);
            escape_next = false;
            continue;
        }

        match c {
            '\\' => {
                escape_next = true;
                current.push(c);
            }
            '"' => {
                in_string = !in_string;
                current.push(c);
            }
            ',' if !in_string => {
                fields.push(current.clone());
                current.clear();
            }
            _ => current.push(c),
        }
    }

    if !current.is_empty() {
        fields.push(current);
    }

    fields
}

/// Parse a single JSON field like "key":"value".
fn parse_json_field(s: &str) -> Option<(&str, String)> {
    let s = s.trim();

    // Find the colon separator
    let colon_pos = s.find(':')?;
    let key_part = s[..colon_pos].trim();
    let value_part = s[colon_pos + 1..].trim();

    // Extract key (remove quotes)
    let key = key_part.trim_matches('"');

    // Extract value (remove quotes and handle escapes)
    let value = if value_part.starts_with('"') && value_part.ends_with('"') && value_part.len() >= 2
    {
        unescape_json_string(&value_part[1..value_part.len() - 1])
    } else {
        value_part.to_string()
    };

    Some((key, value))
}

/// Unescape a JSON string value.
fn unescape_json_string(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(&next) = chars.peek() {
                match next {
                    '"' | '\\' | '/' => {
                        result.push(chars.next().unwrap());
                    }
                    'n' => {
                        chars.next();
                        result.push('\n');
                    }
                    'r' => {
                        chars.next();
                        result.push('\r');
                    }
                    't' => {
                        chars.next();
                        result.push('\t');
                    }
                    _ => result.push(c),
                }
            } else {
                result.push(c);
            }
        } else {
            result.push(c);
        }
    }

    result
}

/// Get the path to the config directory.
fn config_dir_path() -> PathBuf {
    dirs::config_dir()
        .expect("Could not determine config directory")
        .join("discrakt")
}

/// Write credentials to the config file.
fn write_credentials(creds: &SubmittedCredentials) -> Result<PathBuf, String> {
    let config_dir = config_dir_path();

    // Create config directory if it doesn't exist
    if !config_dir.exists() {
        std::fs::create_dir_all(&config_dir)
            .map_err(|e| format!("Failed to create config directory: {}", e))?;
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

    // Set Discord App ID if provided, otherwise use default
    if let Some(ref discord_id) = creds.discord_app_id {
        config.setstr("Discord", "applicationID", Some(discord_id));
    }

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
    if config.get("Trakt API", "OAuthRefreshTokenExpiresAt").is_none() {
        config.setstr("Trakt API", "OAuthRefreshTokenExpiresAt", Some(""));
    }

    config
        .write(&config_path)
        .map_err(|e| format!("Failed to write config file: {}", e))?;

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

/// Format a simple JSON response without serde_json.
fn json_response(fields: &[(&str, &str)]) -> String {
    let mut json = String::from("{");
    for (i, (key, value)) in fields.iter().enumerate() {
        if i > 0 {
            json.push(',');
        }
        // Simple JSON string escaping
        let escaped_value = value.replace('\\', "\\\\").replace('"', "\\\"");
        json.push_str(&format!("\"{}\":\"{}\"", key, escaped_value));
    }
    json.push('}');
    json
}

/// Format a JSON response with numeric fields.
fn json_response_with_numbers(
    string_fields: &[(&str, &str)],
    number_fields: &[(&str, u64)],
) -> String {
    let mut json = String::from("{");
    let mut first = true;

    for (key, value) in string_fields {
        if !first {
            json.push(',');
        }
        first = false;
        let escaped_value = value.replace('\\', "\\\\").replace('"', "\\\"");
        json.push_str(&format!("\"{}\":\"{}\"", key, escaped_value));
    }

    for (key, value) in number_fields {
        if !first {
            json.push(',');
        }
        first = false;
        json.push_str(&format!("\"{}\":{}", key, value));
    }

    json.push('}');
    json
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
pub fn run_setup_server() -> Result<SetupResult, Box<dyn std::error::Error>> {
    let port = find_available_port().ok_or("Failed to find available port")?;
    let addr: SocketAddr = format!("127.0.0.1:{}", port).parse()?;

    let server = Server::http(addr).map_err(|e| format!("Failed to start HTTP server: {}", e))?;

    tracing::info!("Setup server started at http://{}", addr);

    // Flag to signal when setup is complete
    let setup_complete = Arc::new(AtomicBool::new(false));
    let result: Arc<Mutex<Option<SetupResult>>> = Arc::new(Mutex::new(None));
    let oauth_state: Arc<Mutex<OAuthState>> = Arc::new(Mutex::new(OAuthState::Idle));

    // Open browser to setup page
    let url = format!("http://127.0.0.1:{}", port);
    tracing::info!("Opening browser to {}", url);

    if webbrowser::open(&url).is_err() {
        tracing::warn!("Failed to open browser automatically");
        println!("\n========================================");
        println!("  Discrakt Setup");
        println!("========================================\n");
        println!("Please open your browser and navigate to:");
        println!("  {}\n", url);
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
            ("GET", "/") | ("GET", "/index.html") => {
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
                // Read the request body
                let mut body = String::new();
                if let Err(e) = request.as_reader().read_to_string(&mut body) {
                    tracing::error!("Failed to read request body: {}", e);
                    let response = Response::from_string("Failed to read request")
                        .with_status_code(StatusCode(400));
                    let _ = request.respond(response);
                    continue;
                }

                tracing::debug!("Received form data: {}", body);

                // Parse the JSON body
                match parse_json_body(&body) {
                    Some(creds) => {
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
                            let response =
                                Response::from_string(format!("Failed to save: {}", e))
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

                        let discord_id = creds
                            .discord_app_id
                            .clone()
                            .unwrap_or_else(|| DEFAULT_DISCORD_APP_ID.to_string());

                        // Store the result (will be returned after OAuth completes)
                        if let Ok(mut result_guard) = result.lock() {
                            *result_guard = Some(SetupResult {
                                trakt_username: creds.trakt_user.clone(),
                                trakt_client_id: client_id.clone(),
                                discord_app_id: discord_id,
                            });
                        }

                        // Start OAuth device flow
                        match request_device_code(&client_id) {
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

                                // Start background polling thread
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

                                // Send response with device code info
                                let response_json = json_response_with_numbers(
                                    &[
                                        ("user_code", &device_code.user_code),
                                        ("verification_url", &device_code.verification_url),
                                    ],
                                    &[
                                        ("expires_in", device_code.expires_in),
                                        ("interval", device_code.interval),
                                    ],
                                );

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
                                let response =
                                    Response::from_string(format!("OAuth error: {}", e))
                                        .with_status_code(StatusCode(500));
                                let _ = request.respond(response);
                            }
                        }
                    }
                    None => {
                        tracing::error!("Failed to parse form data");
                        let response =
                            Response::from_string("Invalid form data").with_status_code(StatusCode(400));
                        let _ = request.respond(response);
                    }
                }
            }

            ("GET", "/status") => {
                // Return the current OAuth status
                let state = oauth_state.lock().map(|s| s.clone()).unwrap_or(OAuthState::Idle);

                let response_json = match state {
                    OAuthState::Idle => json_response(&[("status", "idle")]),
                    OAuthState::Pending => json_response(&[("status", "pending")]),
                    OAuthState::Success(_) => json_response(&[("status", "success")]),
                    OAuthState::Denied => json_response(&[("status", "denied")]),
                    OAuthState::Expired => json_response(&[("status", "expired")]),
                    OAuthState::Error(ref msg) => {
                        json_response(&[("status", "error"), ("message", msg)])
                    }
                };

                let response = Response::from_string(response_json).with_header(
                    tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..])
                        .unwrap(),
                );
                let _ = request.respond(response);
            }

            ("GET", "/favicon.ico") | ("GET", "/favicon.png") => {
                // Serve the Discrakt icon as favicon
                static ICON_BYTES: &[u8] = include_bytes!("../assets/icon.png");
                let response = Response::from_data(ICON_BYTES).with_header(
                    tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"image/png"[..])
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
fn poll_oauth_in_background(
    device_code: TraktDeviceCode,
    client_id: String,
    oauth_state: Arc<Mutex<OAuthState>>,
    setup_complete: Arc<AtomicBool>,
) {
    let start_time = std::time::Instant::now();
    let timeout = Duration::from_secs(device_code.expires_in);
    let mut poll_interval = Duration::from_secs(device_code.interval);

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

        match poll_device_token(&client_id, &device_code.device_code) {
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
                continue;
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
                poll_interval *= 2;
                continue;
            }
            DeviceTokenPollResult::Error(e) => {
                tracing::error!("Error during token poll: {}", e);
                // Network errors might be temporary, continue polling
                continue;
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
        let result = parse_json_body(json).unwrap();
        assert_eq!(result.trakt_user, "testuser");
        assert_eq!(result.trakt_client_id, "abc123def456");
        assert!(result.discord_app_id.is_none()); // Empty string becomes None
    }

    #[test]
    fn test_parse_json_body_with_discord_id() {
        let json = r#"{"traktUser":"user","traktClientID":"client","discordApplicationID":"123456789012345678"}"#;
        let result = parse_json_body(json).unwrap();
        assert_eq!(
            result.discord_app_id,
            Some("123456789012345678".to_string())
        );
    }

    #[test]
    fn test_parse_json_body_with_escaped_chars() {
        let json =
            r#"{"traktUser":"test\"user","traktClientID":"abc","discordApplicationID":""}"#;
        let result = parse_json_body(json).unwrap();
        assert_eq!(result.trakt_user, "test\"user");
    }

    #[test]
    fn test_parse_json_body_with_empty_client_id() {
        let json = r#"{"traktUser":"testuser","traktClientID":"","discordApplicationID":""}"#;
        let result = parse_json_body(json).unwrap();
        assert_eq!(result.trakt_user, "testuser");
        assert_eq!(result.trakt_client_id, ""); // Empty client ID is allowed
        assert!(result.discord_app_id.is_none());
    }

    #[test]
    fn test_parse_json_body_without_client_id() {
        let json = r#"{"traktUser":"testuser","discordApplicationID":""}"#;
        let result = parse_json_body(json).unwrap();
        assert_eq!(result.trakt_user, "testuser");
        assert_eq!(result.trakt_client_id, ""); // Missing client ID defaults to empty
        assert!(result.discord_app_id.is_none());
    }

    #[test]
    fn test_unescape_json_string() {
        assert_eq!(unescape_json_string(r#"hello\"world"#), "hello\"world");
        assert_eq!(unescape_json_string(r#"line1\nline2"#), "line1\nline2");
        assert_eq!(unescape_json_string(r#"tab\there"#), "tab\there");
    }

    #[test]
    fn test_json_response() {
        let json = json_response(&[("status", "success"), ("message", "OK")]);
        assert!(json.contains("\"status\":\"success\""));
        assert!(json.contains("\"message\":\"OK\""));
    }

    #[test]
    fn test_json_response_with_numbers() {
        let json = json_response_with_numbers(
            &[("user_code", "ABC123")],
            &[("expires_in", 600), ("interval", 5)],
        );
        assert!(json.contains("\"user_code\":\"ABC123\""));
        assert!(json.contains("\"expires_in\":600"));
        assert!(json.contains("\"interval\":5"));
    }
}

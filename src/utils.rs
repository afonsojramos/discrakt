use chrono::{DateTime, FixedOffset, SecondsFormat, Utc};
use configparser::ini::Ini;
use serde::Deserialize;
use std::{env, path::PathBuf, sync::OnceLock, thread, time::Duration};
use ureq::AgentBuilder;

use crate::setup;

const REFRESH_TOKEN_TTL_SECS: u64 = 60 * 60 * 24 * 30 * 3; // 3 months

/// Response from the Trakt device code endpoint.
#[derive(Deserialize, Debug, Clone)]
pub struct TraktDeviceCode {
    pub device_code: String,
    pub user_code: String,
    pub verification_url: String,
    pub expires_in: u64,
    pub interval: u64,
}

/// Default Trakt Client ID for Discrakt.
/// Users can override this by providing their own Client ID in the setup form or config file.
pub const DEFAULT_TRAKT_CLIENT_ID: &str = "32a43d99b2f5866c2bc52d2b189b842b66459a60d7ddbb370a265864d4251115";

static USER_AGENT: OnceLock<String> = OnceLock::new();

pub fn user_agent() -> &'static str {
    USER_AGENT
        .get_or_init(|| format!("{}/{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION")))
        .as_str()
}

#[derive(Deserialize, Debug, Clone)]
pub struct TraktAccessToken {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u64,
    pub refresh_token: String,
    pub scope: String,
    pub created_at: u64,
}

use crate::trakt::TraktWatchingResponse;

pub struct Env {
    pub discord_client_id: String,
    pub trakt_username: String,
    pub trakt_client_id: String,
    pub trakt_oauth_enabled: bool,
    pub trakt_access_token: Option<String>,
    pub trakt_refresh_token: Option<String>,
    pub trakt_refresh_token_expires_at: Option<u64>,
    pub tmdb_token: String,
}

pub struct WatchStats {
    pub watch_percentage: String,
    pub start_date: DateTime<FixedOffset>,
    pub end_date: DateTime<FixedOffset>,
}

/// Result of polling for a device token.
#[derive(Debug, Clone)]
pub enum DeviceTokenPollResult {
    /// Successfully obtained tokens.
    Success(TraktAccessToken),
    /// User has not yet authorized, keep polling.
    Pending,
    /// User denied authorization.
    Denied,
    /// Device code has expired.
    Expired,
    /// Device code was already used.
    AlreadyUsed,
    /// Invalid device code.
    InvalidCode,
    /// Rate limited, should slow down.
    SlowDown,
    /// Network or other error.
    Error(String),
}

/// Request a device code from Trakt for OAuth authorization.
///
/// This is the first step of the device OAuth flow. Returns the device code info
/// that should be displayed to the user.
pub fn request_device_code(trakt_client_id: &str) -> Result<TraktDeviceCode, String> {
    let agent = AgentBuilder::new()
        .timeout_read(Duration::from_secs(10))
        .timeout_write(Duration::from_secs(10))
        .build();

    let response = agent
        .post("https://api.trakt.tv/oauth/device/code")
        .set("Content-Type", "application/json")
        .set("User-Agent", user_agent())
        .send_json(ureq::json!({
            "client_id": trakt_client_id,
        }));

    match response {
        Ok(resp) => resp
            .into_json::<TraktDeviceCode>()
            .map_err(|e| format!("Failed to parse device code response: {}", e)),
        Err(ureq::Error::Status(code, resp)) => {
            let error_body = resp.into_string().unwrap_or_default();
            Err(format!("HTTP {}: {}", code, error_body))
        }
        Err(e) => Err(format!("Network error: {}", e)),
    }
}

/// Poll for a device token once.
///
/// This should be called repeatedly at the interval specified in the device code response.
/// Returns the poll result indicating success, pending, or an error condition.
pub fn poll_device_token(
    trakt_client_id: &str,
    device_code: &str,
) -> DeviceTokenPollResult {
    let agent = AgentBuilder::new()
        .timeout_read(Duration::from_secs(10))
        .timeout_write(Duration::from_secs(10))
        .build();

    let response = agent
        .post("https://api.trakt.tv/oauth/device/token")
        .set("Content-Type", "application/json")
        .set("User-Agent", user_agent())
        .send_json(ureq::json!({
            "code": device_code,
            "client_id": trakt_client_id,
        }));

    match response {
        Ok(resp) => {
            match resp.into_json::<TraktAccessToken>() {
                Ok(token) => DeviceTokenPollResult::Success(token),
                Err(e) => DeviceTokenPollResult::Error(format!("Failed to parse token: {}", e)),
            }
        }
        Err(ureq::Error::Status(400, _)) => DeviceTokenPollResult::Pending,
        Err(ureq::Error::Status(404, _)) => DeviceTokenPollResult::InvalidCode,
        Err(ureq::Error::Status(409, _)) => DeviceTokenPollResult::AlreadyUsed,
        Err(ureq::Error::Status(410, _)) => DeviceTokenPollResult::Expired,
        Err(ureq::Error::Status(418, _)) => DeviceTokenPollResult::Denied,
        Err(ureq::Error::Status(429, _)) => DeviceTokenPollResult::SlowDown,
        Err(ureq::Error::Status(code, resp)) => {
            let body = resp.into_string().unwrap_or_default();
            DeviceTokenPollResult::Error(format!("HTTP {}: {}", code, body))
        }
        Err(e) => DeviceTokenPollResult::Error(format!("Network error: {}", e)),
    }
}

/// Save OAuth tokens to the config file.
pub fn save_oauth_tokens(token: &TraktAccessToken) {
    set_oauth_tokens(token);
}

impl Env {
    pub fn check_oauth(&mut self) {
        if !self.trakt_oauth_enabled {
            return;
        }

        // Check if we have no access token
        if self.trakt_access_token.is_none() || self.trakt_access_token.as_ref().unwrap().is_empty()
        {
            tracing::info!("No OAuth access token found, starting authorization flow");
            self.authorize_app();
            return;
        }

        // Check if the refresh token is expired (this is what you were originally checking)
        if let Some(refresh_expires_at) = self.trakt_refresh_token_expires_at {
            let now = Utc::now().timestamp() as u64;
            if now >= refresh_expires_at {
                tracing::info!("OAuth refresh token has expired, need to reauthorize");
                self.authorize_app();
            } else {
                // Try to refresh the access token proactively
                tracing::info!("Refresh token is still valid, refreshing access token");
                self.exchange_refresh_token_for_access_token();
            }
        } else {
            tracing::warn!(
                "No refresh token expiry time found, unable to determine if refresh token is valid"
            );
        }
    }

    /// Initiates the Trakt Device OAuth flow.
    ///
    /// This flow does not require a client secret:
    /// 1. Request a device code from Trakt
    /// 2. Display the user code and open the verification URL
    /// 3. Poll for token until user authorizes or timeout
    fn authorize_app(&mut self) {
        tracing::info!("Starting Trakt Device OAuth flow");

        // Step 1: Request device code
        let device_code = match request_device_code(&self.trakt_client_id) {
            Ok(code) => code,
            Err(e) => {
                tracing::error!("Failed to request device code: {}", e);
                return;
            }
        };

        tracing::info!(
            user_code = %device_code.user_code,
            verification_url = %device_code.verification_url,
            expires_in = device_code.expires_in,
            "Device code obtained"
        );

        // Step 2: Display code to user and open browser
        // Note: In Windows release builds, the console is hidden, so this output
        // may not be visible. The browser-based setup flow should be used instead.
        println!("\n========================================");
        println!("  Trakt Authorization Required");
        println!("========================================\n");
        println!("  1. Go to: {}", device_code.verification_url);
        println!("  2. Enter code: {}\n", device_code.user_code);
        println!("  Waiting for authorization...\n");

        if webbrowser::open(&device_code.verification_url).is_err() {
            tracing::warn!("Failed to open browser automatically");
            println!("  (Please open the URL manually in your browser)\n");
        }

        // Step 3: Poll for token
        self.poll_for_device_token(&device_code);
    }

    /// Polls the Trakt device token endpoint until authorization is complete.
    fn poll_for_device_token(&mut self, device_code: &TraktDeviceCode) {
        let start_time = std::time::Instant::now();
        let timeout = Duration::from_secs(device_code.expires_in);
        let mut poll_interval = Duration::from_secs(device_code.interval);

        loop {
            // Check if we've exceeded the timeout
            if start_time.elapsed() >= timeout {
                tracing::error!("Device authorization timed out");
                println!("  Authorization timed out. Please restart Discrakt to try again.\n");
                return;
            }

            // Wait for the specified interval before polling
            thread::sleep(poll_interval);

            match poll_device_token(&self.trakt_client_id, &device_code.device_code) {
                DeviceTokenPollResult::Success(token) => {
                    tracing::info!("Successfully obtained OAuth tokens via device flow");
                    self.trakt_access_token = Some(token.access_token.clone());
                    self.trakt_refresh_token = Some(token.refresh_token.clone());

                    // Update in-memory expiry (90 days from now)
                    let now = Utc::now().timestamp() as u64;
                    self.trakt_refresh_token_expires_at = Some(now + REFRESH_TOKEN_TTL_SECS);

                    tracing::debug!(
                        token_type = %token.token_type,
                        expires_in = token.expires_in,
                        scope = %token.scope,
                        "OAuth token response received"
                    );

                    set_oauth_tokens(&token);

                    println!("  Authorization successful!\n");
                    tracing::info!(
                        expires_at = %DateTime::from_timestamp(
                            self.trakt_refresh_token_expires_at.unwrap() as i64, 0
                        )
                        .unwrap()
                        .to_rfc3339_opts(SecondsFormat::Secs, true),
                        "Tokens obtained successfully"
                    );
                    return;
                }
                DeviceTokenPollResult::Pending => {
                    tracing::debug!("Authorization pending, continuing to poll...");
                    continue;
                }
                DeviceTokenPollResult::InvalidCode => {
                    tracing::error!("Invalid device code");
                    println!("  Error: Invalid device code. Please restart Discrakt.\n");
                    return;
                }
                DeviceTokenPollResult::AlreadyUsed => {
                    tracing::error!("Device code already used");
                    println!("  Error: Code already used. Please restart Discrakt.\n");
                    return;
                }
                DeviceTokenPollResult::Expired => {
                    tracing::error!("Device code expired");
                    println!("  Error: Code expired. Please restart Discrakt.\n");
                    return;
                }
                DeviceTokenPollResult::Denied => {
                    tracing::error!("User denied authorization");
                    println!("  Authorization was denied. Please restart Discrakt.\n");
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

    /// Refreshes the OAuth access token using the refresh token.
    ///
    /// For device flow tokens, the refresh can be done without client_secret.
    /// If refresh fails, falls back to full device authorization flow.
    fn exchange_refresh_token_for_access_token(&mut self) {
        let refresh_token = match &self.trakt_refresh_token {
            Some(token) if !token.is_empty() => token.clone(),
            _ => {
                tracing::warn!("No refresh token available, need to reauthorize");
                self.authorize_app();
                return;
            }
        };

        tracing::info!("Attempting to refresh OAuth access token");

        let agent = AgentBuilder::new()
            .timeout_read(Duration::from_secs(5))
            .timeout_write(Duration::from_secs(5))
            .build();

        let response = match agent
            .post("https://api.trakt.tv/oauth/token")
            .set("Content-Type", "application/json")
            .set("User-Agent", user_agent())
            .send_json(ureq::json!({
                "refresh_token": refresh_token,
                "client_id": self.trakt_client_id,
                "grant_type": "refresh_token",
            }))
        {
            Ok(response) => response,
            Err(ureq::Error::Status(400, response)) => {
                tracing::warn!("Refresh token is invalid or expired, need to reauthorize");
                if let Ok(error_body) = response.into_string() {
                    tracing::debug!("Refresh error details: {}", error_body);
                }
                self.authorize_app();
                return;
            }
            Err(ureq::Error::Status(code, response)) => {
                tracing::error!("Failed to refresh token: HTTP {}", code);
                if let Ok(error_body) = response.into_string() {
                    tracing::error!("Error details: {}", error_body);
                }
                // On other errors, try reauthorization
                self.authorize_app();
                return;
            }
            Err(e) => {
                tracing::error!("Network error during token refresh: {}", e);
                return;
            }
        };

        let json_response: Option<TraktAccessToken> = response.into_json().unwrap_or_default();

        if let Some(json_response) = json_response {
            tracing::info!("Successfully refreshed OAuth access token");
            self.trakt_access_token = Some(json_response.access_token.clone());
            self.trakt_refresh_token = Some(json_response.refresh_token.clone());

            // Update in-memory expiry (90 days from now)
            let now = Utc::now().timestamp() as u64;
            self.trakt_refresh_token_expires_at = Some(now + REFRESH_TOKEN_TTL_SECS);

            set_oauth_tokens(&json_response);

            tracing::info!(
                expires_at = %DateTime::from_timestamp(self.trakt_refresh_token_expires_at.unwrap() as i64, 0)
                    .unwrap()
                    .to_rfc3339_opts(SecondsFormat::Secs, true),
                "Token refreshed successfully"
            );
        } else {
            tracing::error!("Failed to parse refresh token response from Trakt API");
            tracing::warn!("Will attempt full reauthorization");
            self.authorize_app();
        }
    }
}

fn config_dir_path() -> PathBuf {
    dirs::config_dir()
        .expect("Could not determine config directory")
        .join("discrakt")
}

fn find_config_file() -> Option<PathBuf> {
    let config_path = config_dir_path();
    let mut exe_path = env::current_exe().unwrap();
    exe_path.pop();

    let locations = vec![config_path, exe_path];

    for location in &locations {
        let config_file = location.join("credentials.ini");
        if config_file.exists() {
            return Some(config_file);
        }
    }
    tracing::error!(
        "Could not find credentials.ini in {:?}",
        locations
            .iter()
            .map(|loc| loc.to_str().to_owned().unwrap())
            .collect::<Vec<_>>()
    );
    None
}

/// Run the browser-based setup flow for first-time configuration.
///
/// This starts a local HTTP server and opens a browser to collect credentials.
/// Returns the setup result on success, or exits the process on failure/cancellation.
fn run_browser_setup() -> setup::SetupResult {
    tracing::info!("Starting browser-based setup flow");

    match setup::run_setup_server() {
        Ok(result) => {
            tracing::info!("Setup completed successfully for user: {}", result.trakt_username);
            result
        }
        Err(e) => {
            tracing::error!("Setup failed: {}", e);
            eprintln!("\nSetup was cancelled or failed. Please restart Discrakt to try again.");
            std::process::exit(1);
        }
    }
}

pub fn load_config() -> Env {
    let mut config = Ini::new();
    let config_file = find_config_file();

    // Check if we need to run browser-based setup
    // Only trakt_username is strictly required; trakt_client_id has a default
    let needs_setup = match &config_file {
        None => true,
        Some(path) => {
            if config.load(path).is_err() {
                true
            } else {
                let trakt_username = config.get("Trakt API", "traktUser");
                trakt_username.as_ref().is_none_or(|s| s.is_empty())
            }
        }
    };

    if needs_setup {
        tracing::info!("Credentials missing or incomplete, starting browser setup");

        // Run browser-based setup
        let setup_result = run_browser_setup();

        // Re-read the config file after setup
        let config_path = find_config_file().expect("Config file should exist after setup");
        config.load(&config_path).expect("Failed to load credentials.ini after setup");

        // Return config using setup result values (they're authoritative)
        // Use default Trakt Client ID if setup result has empty value
        let trakt_client_id = if setup_result.trakt_client_id.is_empty() {
            DEFAULT_TRAKT_CLIENT_ID.to_string()
        } else {
            setup_result.trakt_client_id
        };

        return Env {
            discord_client_id: setup_result.discord_app_id,
            trakt_username: setup_result.trakt_username,
            trakt_client_id,
            trakt_oauth_enabled: config
                .getbool("Trakt API", "enabledOAuth")
                .unwrap_or(Some(false))
                .unwrap_or(false),
            trakt_access_token: config.get("Trakt API", "OAuthAccessToken"),
            trakt_refresh_token: config.get("Trakt API", "OAuthRefreshToken"),
            trakt_refresh_token_expires_at: config
                .getuint("Trakt API", "OAuthRefreshTokenExpiresAt")
                .unwrap_or_default(),
            tmdb_token: "21b815a75fec5f1e707e3da1b9b2d7e3".to_string(),
        };
    }

    // Config file exists and has required fields
    let path = config_file.unwrap();
    config.load(&path).expect("Failed to load credentials.ini");

    let trakt_username = config.get("Trakt API", "traktUser").expect("traktUser not found");

    // Use default Trakt Client ID if not provided or empty
    let trakt_client_id = config
        .get("Trakt API", "traktClientID")
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_TRAKT_CLIENT_ID.to_string());

    // Check for custom Discord App ID
    let discord_client_id = config
        .get("Discord", "applicationID")
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "826189107046121572".to_string());

    Env {
        discord_client_id,
        trakt_username,
        trakt_client_id,
        trakt_oauth_enabled: config
            .getbool("Trakt API", "enabledOAuth")
            .unwrap_or(Some(false))
            .unwrap_or(false),
        trakt_access_token: config.get("Trakt API", "OAuthAccessToken"),
        trakt_refresh_token: config.get("Trakt API", "OAuthRefreshToken"),
        trakt_refresh_token_expires_at: config
            .getuint("Trakt API", "OAuthRefreshTokenExpiresAt")
            .unwrap_or_default(),
        tmdb_token: "21b815a75fec5f1e707e3da1b9b2d7e3".to_string(),
    }
}

fn set_oauth_tokens(json_response: &TraktAccessToken) {
    let mut config = Ini::new_cs();
    let config_file = find_config_file();

    let path = config_file.expect("Could not find credentials.ini");

    config
        .load(path.clone())
        .expect("Failed to load credentials.ini");
    config.setstr(
        "Trakt API",
        "OAuthAccessToken",
        Some(json_response.access_token.as_str()),
    );
    config.setstr(
        "Trakt API",
        "OAuthRefreshToken",
        Some(json_response.refresh_token.as_str()),
    );

    // Store refresh token expiry as now + 3 months
    let now = Utc::now().timestamp() as u64;
    let refresh_token_expires_at = now + REFRESH_TOKEN_TTL_SECS;

    config.set(
        "Trakt API",
        "OAuthRefreshTokenExpiresAt",
        Some(refresh_token_expires_at.to_string()),
    );
    config.write(path).expect("Failed to write credentials.ini");
}

pub fn get_watch_stats(trakt_response: &TraktWatchingResponse) -> WatchStats {
    let start_date = DateTime::parse_from_rfc3339(&trakt_response.started_at).unwrap();
    let end_date = DateTime::parse_from_rfc3339(&trakt_response.expires_at).unwrap();
    let percentage = Utc::now().signed_duration_since(start_date).num_seconds() as f32
        / end_date.signed_duration_since(start_date).num_seconds() as f32;
    let watch_percentage = format!("{:.2}%", percentage * 100.0);

    WatchStats {
        watch_percentage,
        start_date,
        end_date,
    }
}

pub enum MediaType {
    Show,
    Movie,
}

impl MediaType {
    pub fn as_str(&self) -> &'static str {
        match self {
            MediaType::Show => "episode",
            MediaType::Movie => "movie",
        }
    }
}

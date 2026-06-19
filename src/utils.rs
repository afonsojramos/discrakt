use chrono::{DateTime, FixedOffset, SecondsFormat, Utc};
use configparser::ini::Ini;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use serde::Deserialize;
use serde_json::json;
use std::{env, path::PathBuf, sync::OnceLock, thread, time::Duration};
use sys_locale::get_locale;
use ureq::Agent;

use crate::setup;
use crate::trakt::DEFAULT_TRAKT_BASE_URL;

/// Refresh token time-to-live in seconds (3 months).
/// Trakt refresh tokens are valid for 3 months from creation.
/// See: https://trakt.docs.apiary.io/#reference/authentication-oauth
const REFRESH_TOKEN_TTL_SECS: u64 = 60 * 60 * 24 * 30 * 3;

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
///
/// This is the official Discrakt application registered with Trakt.tv.
/// It is intentionally embedded in the source code for ease of setup.
///
/// **Rate limits are per-user, not per-client-id**, so all users sharing this
/// client ID have independent rate limits based on their OAuth tokens.
/// See: https://trakt.docs.apiary.io/#introduction/rate-limiting
///
/// Users can override this by providing their own Client ID in the setup form
/// or config file if they prefer to use their own Trakt application.
pub const DEFAULT_TRAKT_CLIENT_ID: &str =
    "32a43d99b2f5866c2bc52d2b189b842b66459a60d7ddbb370a265864d4251115";

/// Default Discord Application ID for Movies.
///
/// This is the official Discrakt Discord application for movie Rich Presence.
/// It is intentionally embedded for ease of setup. Users can override this in the
/// config file if they want to use their own Discord application with custom assets.
pub const DEFAULT_DISCORD_APP_ID_MOVIE: &str = "1446321426893111436";

/// Default Discord Application ID for TV Shows.
///
/// This is the official Discrakt Discord application for TV show Rich Presence.
pub const DEFAULT_DISCORD_APP_ID_SHOW: &str = "1446117100568445001";

/// Default Discord Application ID (used when media type is unknown).
/// Defaults to the movie application.
pub const DEFAULT_DISCORD_APP_ID: &str = DEFAULT_DISCORD_APP_ID_MOVIE;

/// Default TMDB API token for fetching movie/show artwork.
///
/// This is a public API token registered for Discrakt. TMDB API tokens are
/// designed to be embedded in client applications and have generous rate limits.
/// See: https://developer.themoviedb.org/docs/faq
pub const DEFAULT_TMDB_TOKEN: &str = "21b815a75fec5f1e707e3da1b9b2d7e3";

/// Detects the system language and maps it to a supported TMDB language code.
///
/// First attempts an exact match (e.g., "pt-BR" matches "pt-BR"), then falls
/// back to prefix matching (e.g., "pt" matches "pt-PT"). This ensures users
/// get their regional variant when available.
///
/// Falls back to "en-US" if the system language is not recognized or supported.
fn detect_system_language() -> String {
    let system_lang = get_locale().unwrap_or_else(|| "en-US".to_string());
    // Normalize separator: convert underscore to hyphen for consistent matching
    let normalized = system_lang.replace('_', "-");

    // Try exact match first (e.g., "pt-BR" -> "pt-BR")
    if let Some((_, code)) = LANGUAGES.iter().find(|(_, code)| *code == normalized) {
        return code.to_string();
    }

    // Fall back to prefix match (e.g., "pt" -> "pt-PT")
    // Use precise prefix matching to avoid false positives (e.g., "e" matching "el-GR")
    let prefix = normalized.split('-').next().unwrap_or("en");
    LANGUAGES
        .iter()
        .find(|(_, code)| code.split('-').next() == Some(prefix))
        .map(|(_, code)| code.to_string())
        .unwrap_or_else(|| "en-US".to_string())
}

/// Supported languages for the tray menu and TMDB title localization.
///
/// Each entry is a tuple of `(display_name, tmdb_language_code)`:
/// - `display_name`: Human-readable name shown in the tray menu (in native language)
/// - `tmdb_language_code`: TMDB API language code in the format `xx-YY` (ISO 639-1 + ISO 3166-1)
///
/// # Adding New Languages
///
/// To add a new language:
/// 1. Find the TMDB language code from <https://developer.themoviedb.org/docs/languages>
/// 2. Add a tuple with the native language name and TMDB code
/// 3. Language codes must follow the `xx-YY` format (e.g., "pt-BR", "zh-CN")
///
/// # Examples
///
/// ```
/// use discrakt::utils::LANGUAGES;
///
/// // Find English display name and code
/// let english = LANGUAGES.iter().find(|(_, code)| *code == "en-US");
/// assert_eq!(english, Some(&("English", "en-US")));
/// ```
pub const LANGUAGES: &[(&str, &str)] = &[
    ("English", "en-US"),
    ("Français", "fr-FR"),
    ("Español", "es-ES"),
    ("Deutsch", "de-DE"),
    ("Italiano", "it-IT"),
    ("Português", "pt-PT"),
    ("Português (Brasil)", "pt-BR"),
    ("Русский", "ru-RU"),
    ("日本語 (Japanese)", "ja-JP"),
    ("简体中文 (Chinese)", "zh-CN"),
    ("한국어 (Korean)", "ko-KR"),
    ("Nederlands", "nl-NL"),
    ("Polski", "pl-PL"),
    ("Türkçe", "tr-TR"),
    ("Svenska", "sv-SE"),
    ("Dansk", "da-DK"),
    ("Norsk", "no-NO"),
    ("Suomi", "fi-FI"),
    ("Čeština", "cs-CZ"),
    ("Magyar", "hu-HU"),
    ("Ελληνικά", "el-GR"),
    ("Română", "ro-RO"),
    ("Hrvatski", "hr-HR"),
    ("Slovenský", "sk-SK"),
    ("Thai", "th-TH"),
    ("Vietnamese", "vi-VN"),
    ("Indonesian", "id-ID"),
    ("Ukrainian", "uk-UA"),
    ("Arabic", "ar-SA"),
    ("Hebrew", "he-IL"),
    ("Hindi", "hi-IN"),
];

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

use crate::source::Watching;

/// Which tracking source Discrakt polls for "currently watching" status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SourceKind {
    #[default]
    Trakt,
    Plex,
    Jellyfin,
}

pub struct Env {
    /// The selected tracking source.
    pub source: SourceKind,
    pub trakt_username: String,
    pub trakt_client_id: String,
    pub trakt_oauth_enabled: bool,
    pub trakt_access_token: Option<String>,
    pub trakt_refresh_token: Option<String>,
    pub trakt_refresh_token_expires_at: Option<u64>,
    /// Base URL of the Plex Media Server (Plex source only).
    pub plex_server_url: String,
    /// Plex authentication token (Plex source only).
    pub plex_token: String,
    /// Plex account username to mirror (Plex source only).
    pub plex_username: String,
    /// Base URL of the Jellyfin server (Jellyfin source only).
    pub jellyfin_server_url: String,
    /// Jellyfin access token (Jellyfin source only).
    pub jellyfin_access_token: String,
    /// Jellyfin device id for the auth header (Jellyfin source only).
    pub jellyfin_device_id: String,
    /// Jellyfin user id to mirror (Jellyfin source only).
    pub jellyfin_user_id: String,
    /// Jellyfin username to mirror when no user id is set.
    pub jellyfin_username: String,
    pub tmdb_token: String,
    pub tmdb_language: String,
}

pub struct WatchStats {
    pub watch_percentage: String,
    pub start_date: DateTime<FixedOffset>,
    pub end_date: DateTime<FixedOffset>,
    /// Runtime in minutes from Trakt (None if unavailable, using session times as fallback).
    pub runtime_minutes: Option<u16>,
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
///
/// # Arguments
/// * `trakt_client_id` - The Trakt client ID
/// * `base_url` - Optional base URL override (defaults to https://api.trakt.tv)
pub fn request_device_code(
    trakt_client_id: &str,
    base_url: Option<&str>,
) -> Result<TraktDeviceCode, String> {
    let base = base_url.unwrap_or(DEFAULT_TRAKT_BASE_URL);
    let config = Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(20)))
        .user_agent(user_agent())
        .build();
    let agent: Agent = config.into();

    let response = agent
        .post(&format!("{}/oauth/device/code", base))
        .header("Content-Type", "application/json")
        .send_json(json!({
            "client_id": trakt_client_id,
        }));

    match response {
        Ok(mut resp) => resp
            .body_mut()
            .read_json::<TraktDeviceCode>()
            .map_err(|e| format!("Failed to parse device code response: {}", e)),
        Err(ureq::Error::StatusCode(code)) => Err(format!("HTTP {}", code)),
        Err(e) => Err(format!("Network error: {}", e)),
    }
}

/// Poll for a device token once.
///
/// This should be called repeatedly at the interval specified in the device code response.
/// Returns the poll result indicating success, pending, or an error condition.
///
/// # Arguments
/// * `trakt_client_id` - The Trakt client ID
/// * `device_code` - The device code from the initial request
/// * `base_url` - Optional base URL override (defaults to https://api.trakt.tv)
pub fn poll_device_token(
    trakt_client_id: &str,
    device_code: &str,
    base_url: Option<&str>,
) -> DeviceTokenPollResult {
    let base = base_url.unwrap_or(DEFAULT_TRAKT_BASE_URL);
    let config = Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(20)))
        .user_agent(user_agent())
        .build();
    let agent: Agent = config.into();

    let response = agent
        .post(&format!("{}/oauth/device/token", base))
        .header("Content-Type", "application/json")
        .send_json(json!({
            "code": device_code,
            "client_id": trakt_client_id,
        }));

    match response {
        Ok(mut resp) => match resp.body_mut().read_json::<TraktAccessToken>() {
            Ok(token) => DeviceTokenPollResult::Success(token),
            Err(e) => DeviceTokenPollResult::Error(format!("Failed to parse token: {}", e)),
        },
        Err(ureq::Error::StatusCode(400)) => DeviceTokenPollResult::Pending,
        Err(ureq::Error::StatusCode(404)) => DeviceTokenPollResult::InvalidCode,
        Err(ureq::Error::StatusCode(409)) => DeviceTokenPollResult::AlreadyUsed,
        Err(ureq::Error::StatusCode(410)) => DeviceTokenPollResult::Expired,
        Err(ureq::Error::StatusCode(418)) => DeviceTokenPollResult::Denied,
        Err(ureq::Error::StatusCode(429)) => DeviceTokenPollResult::SlowDown,
        Err(ureq::Error::StatusCode(code)) => {
            DeviceTokenPollResult::Error(format!("HTTP {}", code))
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
        let device_code = match request_device_code(&self.trakt_client_id, None) {
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

        let encoded_code = utf8_percent_encode(&device_code.user_code, NON_ALPHANUMERIC);
        let auto_url = format!("{}?code={}", device_code.verification_url, encoded_code);

        // Step 2: Display code to user and open browser
        // Note: In Windows release builds, the console is hidden, so this output
        // may not be visible. The browser-based setup flow should be used instead.
        println!("\n========================================");
        println!("  Trakt Authorization Required");
        println!("========================================\n");
        println!("  Open this link to authorize:");
        println!("  {}\n", auto_url);
        println!("  (Verification Code: {})\n", device_code.user_code);
        println!("  Waiting for authorization...\n");

        if webbrowser::open(&auto_url).is_err() {
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

            match poll_device_token(&self.trakt_client_id, &device_code.device_code, None) {
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
                }
                DeviceTokenPollResult::Error(e) => {
                    tracing::error!("Error during token poll: {}", e);
                    // Network errors might be temporary, continue polling
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

        let config = Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(10)))
            .user_agent(user_agent())
            .build();
        let agent: Agent = config.into();

        let mut response = match agent
            .post("https://api.trakt.tv/oauth/token")
            .header("Content-Type", "application/json")
            .send_json(json!({
                "refresh_token": refresh_token,
                "client_id": self.trakt_client_id,
                "grant_type": "refresh_token",
            })) {
            Ok(response) => response,
            Err(ureq::Error::StatusCode(400)) => {
                tracing::warn!("Refresh token is invalid or expired, need to reauthorize");
                self.authorize_app();
                return;
            }
            Err(ureq::Error::StatusCode(code)) => {
                tracing::error!("Failed to refresh token: HTTP {}", code);
                // On other errors, try reauthorization
                self.authorize_app();
                return;
            }
            Err(e) => {
                tracing::error!("Network error during token refresh: {}", e);
                return;
            }
        };

        let json_response: Option<TraktAccessToken> = match response.body_mut().read_json() {
            Ok(token) => Some(token),
            Err(e) => {
                tracing::error!("Failed to parse token refresh response: {}", e);
                None
            }
        };

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

/// Returns the application config directory path.
///
/// On Windows: `%APPDATA%\discrakt`
/// On macOS: `~/Library/Application Support/discrakt`
/// On Linux: `~/.config/discrakt`
pub fn config_dir_path() -> PathBuf {
    dirs::config_dir()
        .expect("Could not determine config directory")
        .join("discrakt")
}

/// Returns the directory where logs should be written.
///
/// Follows the same lookup order as credentials.ini:
/// 1. Directory containing the executable (if credentials.ini exists there)
/// 2. Platform config directory (%APPDATA%\discrakt, etc.)
///
/// This ensures logs are written alongside credentials.ini for easier discovery.
pub fn log_dir_path() -> PathBuf {
    // Check if credentials.ini exists next to the executable
    if let Ok(mut exe_path) = env::current_exe() {
        exe_path.pop();
        if exe_path.join("credentials.ini").exists() {
            return exe_path;
        }
    }

    // Fall back to config directory
    config_dir_path()
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
///
/// # Errors
///
/// Returns an error if the setup server fails to start or the user cancels setup.
fn run_browser_setup() -> Result<setup::SetupResult, String> {
    tracing::info!("Starting browser-based setup flow");

    match setup::run_setup_server() {
        Ok(result) => {
            tracing::info!(
                "Setup completed successfully for user: {}",
                result.trakt_username
            );
            Ok(result)
        }
        Err(e) => {
            tracing::error!("Setup failed: {}", e);
            Err(format!(
                "Setup was cancelled or failed: {}. Please restart Discrakt to try again.",
                e
            ))
        }
    }
}

/// Reads Plex settings (server URL, token, username) from a loaded config.
fn read_plex_config(config: &Ini) -> (String, String, String) {
    (
        config.get("Plex", "serverUrl").unwrap_or_default(),
        config.get("Plex", "token").unwrap_or_default(),
        config.get("Plex", "username").unwrap_or_default(),
    )
}

/// Returns true when the config has a usable Jellyfin source.
fn jellyfin_configured(config: &Ini) -> bool {
    config
        .get("Jellyfin", "serverUrl")
        .is_some_and(|s| !s.is_empty())
        && config
            .get("Jellyfin", "accessToken")
            .is_some_and(|s| !s.is_empty())
}

/// Determines the active source, honoring an explicit `[Discrakt] source`
/// override and otherwise falling back to whichever source is configured.
fn determine_source(
    config: &Ini,
    trakt_configured: bool,
    plex_configured: bool,
    jellyfin_configured: bool,
) -> SourceKind {
    match config
        .get("Discrakt", "source")
        .map(|s| s.trim().to_lowercase())
        .as_deref()
    {
        Some("plex") => SourceKind::Plex,
        Some("jellyfin") => SourceKind::Jellyfin,
        Some("trakt") => SourceKind::Trakt,
        // No explicit choice: prefer Trakt, then Plex, then Jellyfin.
        _ if trakt_configured => SourceKind::Trakt,
        _ if plex_configured => SourceKind::Plex,
        _ if jellyfin_configured => SourceKind::Jellyfin,
        _ => SourceKind::Trakt,
    }
}

/// Load configuration from the credentials file.
///
/// # Errors
///
/// Returns an error if:
/// - Browser setup is required but fails
/// - The config file cannot be read after setup
/// - Required fields are missing from the config
pub fn load_config() -> Result<Env, String> {
    let mut config = Ini::new();
    let config_file = find_config_file();

    // Setup is needed only when neither a Trakt nor a Plex source is configured.
    let loaded = match &config_file {
        Some(path) => config.load(path).is_ok(),
        None => false,
    };
    // Trakt is configured when there's a username (no-OAuth public profile) or
    // an OAuth access token from the login flow (username then unnecessary).
    let trakt_configured = loaded
        && (config
            .get("Trakt API", "traktUser")
            .is_some_and(|s| !s.is_empty())
            || config
                .get("Trakt API", "OAuthAccessToken")
                .is_some_and(|s| !s.is_empty()));
    let (server_url, token, _username) = if loaded {
        read_plex_config(&config)
    } else {
        Default::default()
    };
    let plex_configured = !server_url.is_empty() && !token.is_empty();
    let has_jellyfin = loaded && jellyfin_configured(&config);

    if !trakt_configured && !plex_configured && !has_jellyfin {
        tracing::info!("Credentials missing or incomplete, starting browser setup");

        // Run browser-based setup, then re-read the config it wrote.
        run_browser_setup()?;
        let config_path =
            find_config_file().ok_or_else(|| "Config file should exist after setup".to_string())?;
        config = Ini::new();
        config
            .load(&config_path)
            .map_err(|e| format!("Failed to load credentials.ini after setup: {}", e))?;
    }

    let (plex_server_url, plex_token, plex_username) = read_plex_config(&config);
    let trakt_username = config.get("Trakt API", "traktUser").unwrap_or_default();

    // Use default Trakt Client ID if not provided or empty
    let trakt_client_id = config
        .get("Trakt API", "traktClientID")
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_TRAKT_CLIENT_ID.to_string());

    let tmdb_language = config
        .get("Trakt API", "language")
        .filter(|s| !s.is_empty())
        .unwrap_or_else(detect_system_language);

    let trakt_has_oauth = config
        .get("Trakt API", "OAuthAccessToken")
        .is_some_and(|s| !s.is_empty());
    let source = determine_source(
        &config,
        !trakt_username.is_empty() || trakt_has_oauth,
        !plex_server_url.is_empty() && !plex_token.is_empty(),
        jellyfin_configured(&config),
    );

    Ok(Env {
        source,
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
        plex_server_url,
        plex_token,
        plex_username,
        jellyfin_server_url: config.get("Jellyfin", "serverUrl").unwrap_or_default(),
        jellyfin_access_token: config.get("Jellyfin", "accessToken").unwrap_or_default(),
        jellyfin_device_id: config.get("Jellyfin", "deviceId").unwrap_or_default(),
        jellyfin_user_id: config.get("Jellyfin", "userId").unwrap_or_default(),
        jellyfin_username: config.get("Jellyfin", "username").unwrap_or_default(),
        tmdb_token: DEFAULT_TMDB_TOKEN.to_string(),
        tmdb_language,
    })
}

fn set_oauth_tokens(json_response: &TraktAccessToken) {
    let path = match find_config_file() {
        Some(p) => p,
        None => {
            tracing::error!("Could not find credentials.ini to save OAuth tokens");
            return;
        }
    };

    let mut config = Ini::new_cs();
    if let Err(e) = config.load(&path) {
        tracing::error!("Failed to load credentials.ini: {}", e);
        return;
    }

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

    if let Err(e) = config.write(&path) {
        tracing::error!("Failed to write credentials.ini: {}", e);
        return;
    }

    set_restrictive_permissions(&path);
}

/// Set restrictive file permissions (0600) on Unix to protect sensitive files.
#[cfg(unix)]
pub fn set_restrictive_permissions(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    let permissions = std::fs::Permissions::from_mode(0o600);
    if let Err(e) = std::fs::set_permissions(path, permissions) {
        tracing::warn!(
            "Failed to set restrictive permissions on {}: {}",
            path.display(),
            e
        );
    }
}

/// No-op on non-Unix platforms.
#[cfg(not(unix))]
pub fn set_restrictive_permissions(_path: &std::path::Path) {}

pub fn get_watch_stats(watching: &Watching) -> WatchStats {
    let end_date = watching.expires_at;

    // Prefer a precise window derived from the known runtime; otherwise fall
    // back to the source-provided session times.
    let (start_date, end_date) = match watching.runtime_minutes {
        Some(minutes) => {
            let duration = chrono::Duration::minutes(i64::from(minutes));
            (end_date - duration, end_date)
        }
        None => {
            tracing::trace!("No runtime available, using source session times as fallback");
            (watching.started_at, end_date)
        }
    };

    // Prevent division by zero if dates are somehow equal
    let total_seconds = end_date
        .signed_duration_since(start_date)
        .num_seconds()
        .max(1);
    // Clamp to [0, 1] so clock skew or a stale position never reports a
    // nonsensical percentage (e.g. negative or above 100%).
    let percentage = (Utc::now().signed_duration_since(start_date).num_seconds() as f32
        / total_seconds as f32)
        .clamp(0.0, 1.0);
    let watch_percentage = format!("{:.2}%", percentage * 100.0);

    WatchStats {
        watch_percentage,
        start_date,
        end_date,
        runtime_minutes: watching.runtime_minutes,
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

/// Detects if the system is using light mode.
pub fn is_light_mode() -> bool {
    match dark_light::detect() {
        Ok(dark_light::Mode::Light) => true,
        Ok(dark_light::Mode::Unspecified) => {
            // Default to dark mode (white icon) when unspecified
            false
        }
        Ok(dark_light::Mode::Dark) => false,
        Err(_) => {
            // On error, default to dark mode (white icon)
            false
        }
    }
}

/// Creates an inverted (dark) version of the icon for light mode.
/// Preserves alpha channel while inverting RGB values.
pub fn create_dark_icon(image: &image::RgbaImage) -> image::RgbaImage {
    let mut dark = image.clone();
    for pixel in dark.pixels_mut() {
        // Invert RGB, keep alpha
        pixel[0] = 255 - pixel[0]; // R
        pixel[1] = 255 - pixel[1]; // G
        pixel[2] = 255 - pixel[2]; // B
                                   // pixel[3] = alpha, keep as-is
    }
    dark
}

/// Saves the language preference to the config file.
pub fn save_language_preference(language: &str) {
    if let Some(path) = find_config_file() {
        let mut config = Ini::new_cs();
        if let Err(e) = config.load(&path) {
            tracing::debug!("Could not load existing config (creating new): {}", e);
        }
        config.setstr("Trakt API", "language", Some(language));
        if let Err(e) = config.write(&path) {
            tracing::error!("Failed to save language preference: {}", e);
        }
    } else {
        tracing::error!("Failed to save language preference: config file not found");
    }
}

#[cfg(test)]
mod tests {
    use super::{determine_source, read_plex_config, SourceKind};
    use configparser::ini::Ini;

    fn parse(contents: &str) -> Ini {
        let mut config = Ini::new();
        config.read(contents.to_string()).expect("valid ini");
        config
    }

    #[test]
    fn determine_source_defaults_to_trakt() {
        let config = parse("[Trakt API]\ntraktUser=alice\n");
        assert_eq!(
            determine_source(&config, true, false, false),
            SourceKind::Trakt
        );
    }

    #[test]
    fn determine_source_uses_plex_when_only_plex_configured() {
        let config = parse("[Plex]\nserverUrl=http://host:32400\ntoken=abc\n");
        assert_eq!(
            determine_source(&config, false, true, false),
            SourceKind::Plex
        );
    }

    #[test]
    fn determine_source_uses_jellyfin_when_only_jellyfin_configured() {
        let config = parse("[Jellyfin]\nserverUrl=http://host:8096\naccessToken=abc\n");
        assert_eq!(
            determine_source(&config, false, false, true),
            SourceKind::Jellyfin
        );
    }

    #[test]
    fn determine_source_honors_explicit_override() {
        let config = parse("[Discrakt]\nsource=plex\n[Trakt API]\ntraktUser=alice\n");
        // Both configured, but the explicit override wins.
        assert_eq!(
            determine_source(&config, true, true, false),
            SourceKind::Plex
        );

        let config = parse("[Discrakt]\nsource=jellyfin\n[Trakt API]\ntraktUser=alice\n");
        assert_eq!(
            determine_source(&config, true, false, true),
            SourceKind::Jellyfin
        );

        let config = parse("[Discrakt]\nsource=trakt\n[Plex]\nserverUrl=http://h\ntoken=t\n");
        assert_eq!(
            determine_source(&config, false, true, false),
            SourceKind::Trakt
        );
    }

    #[test]
    fn read_plex_config_reads_all_fields() {
        let config = parse("[Plex]\nserverUrl=http://host:32400\ntoken=abc\nusername=alice\n");
        assert_eq!(
            read_plex_config(&config),
            (
                "http://host:32400".to_string(),
                "abc".to_string(),
                "alice".to_string()
            )
        );
    }

    #[test]
    fn read_plex_config_defaults_to_empty() {
        let config = parse("[Trakt API]\ntraktUser=alice\n");
        assert_eq!(
            read_plex_config(&config),
            (String::new(), String::new(), String::new())
        );
    }
}

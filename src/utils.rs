use chrono::{DateTime, FixedOffset, SecondsFormat, Utc};
use configparser::ini::Ini;
use serde::Deserialize;
use std::{env, io, path::PathBuf, time::Duration};
use ureq::AgentBuilder;

const REFRESH_TOKEN_TTL_SECS: u64 = 60 * 60 * 24 * 30 * 3; // 3 months

#[derive(Deserialize, Debug)]
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
    pub trakt_client_secret: Option<String>,
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

pub fn log(message: &str) {
    tracing::info!("{}", message);
}

impl Env {
    pub fn check_oauth(&mut self) {
        if !self.trakt_oauth_enabled {
            return;
        }

        // Check if we have no access token
        if self.trakt_access_token.is_none() || self.trakt_access_token.as_ref().unwrap().is_empty()
        {
            log("No OAuth access token found, starting authorization flow");
            self.authorize_app();
            return;
        }

        // Check if the refresh token is expired (this is what you were originally checking)
        if let Some(refresh_expires_at) = self.trakt_refresh_token_expires_at {
            let now = Utc::now().timestamp() as u64;
            if now >= refresh_expires_at {
                log("OAuth refresh token has expired, need to reauthorize");
                self.authorize_app();
            } else {
                // Try to refresh the access token proactively
                log("Refresh token is still valid, refreshing access token");
                self.exchange_refresh_token_for_access_token();
            }
        } else {
            log(
                "No refresh token expiry time found, unable to determine if refresh token is valid",
            );
        }
    }

    fn authorize_app(&mut self) {
        log("Opening browser for OAuth authorization");
        if webbrowser::open(
            &format!("https://trakt.tv/oauth/authorize?response_type=code&client_id={}&redirect_uri=urn:ietf:wg:oauth:2.0:oob", self.trakt_client_id)
        ).is_err() {
            log("Failed to open webbrowser to authorize discrakt");
            return;
        };
        self.exchange_code_for_access_token();
    }

    fn exchange_code_for_access_token(&mut self) {
        // read OAuth code from command line
        print!("Enter code from website: ");
        io::Write::flush(&mut io::stdout()).expect("Failed to flush stdout");
        let mut code = String::new();
        io::stdin()
            .read_line(&mut code)
            .expect("Failed to read line");
        let code = code.trim();

        log("Exchanging authorization code for access token");

        let agent = AgentBuilder::new()
            .timeout_read(Duration::from_secs(5))
            .timeout_write(Duration::from_secs(5))
            .build();

        let response = match agent
            .post("https://api.trakt.tv/oauth/token")
            .set("Content-Type", "application/json")
            .send_json(ureq::json!({
                "code": code,
                "client_id": self.trakt_client_id,
                "client_secret": self.trakt_client_secret.as_ref().expect("client_secret not found"),
                "redirect_uri": "urn:ietf:wg:oauth:2.0:oob",
                "grant_type": "authorization_code",
            }))
        {
            Ok(response) => response,
            Err(ureq::Error::Status(code, response)) => {
                log(&format!("Failed to exchange authorization code: HTTP {}", code));
                if let Ok(error_body) = response.into_string() {
                    log(&format!("Error details: {}", error_body));
                }
                return;
            }
            Err(e) => {
                log(&format!("Network error during token exchange: {}", e));
                return;
            }
        };

        let json_response: Option<TraktAccessToken> = response.into_json().unwrap_or_default();

        if let Some(json_response) = json_response {
            log("Successfully obtained OAuth tokens");
            self.trakt_access_token = Some(json_response.access_token.clone());
            self.trakt_refresh_token = Some(json_response.refresh_token.clone());

            tracing::debug!("Response: {:?}", json_response);

            set_oauth_tokens(&json_response);

            log(&format!(
                "Tokens obtained successfully, refresh token expires at: {}",
                DateTime::from_timestamp(self.trakt_refresh_token_expires_at.unwrap() as i64, 0)
                    .unwrap()
                    .to_rfc3339_opts(SecondsFormat::Secs, true)
            ));
        } else {
            log("Failed to parse token response from Trakt API");
        }
    }

    fn exchange_refresh_token_for_access_token(&mut self) {
        let refresh_token = match &self.trakt_refresh_token {
            Some(token) if !token.is_empty() => token.clone(),
            _ => {
                log("No refresh token available, need to reauthorize");
                self.authorize_app();
                return;
            }
        };

        log("Attempting to refresh OAuth access token");

        let agent = AgentBuilder::new()
            .timeout_read(Duration::from_secs(5))
            .timeout_write(Duration::from_secs(5))
            .build();

        let response = match agent
            .post("https://api.trakt.tv/oauth/token")
            .set("Content-Type", "application/json")
            .send_json(ureq::json!({
                "refresh_token": refresh_token,
                "client_id": self.trakt_client_id,
                "client_secret": self.trakt_client_secret.as_ref().expect("client_secret not found"),
                "redirect_uri": "urn:ietf:wg:oauth:2.0:oob",
                "grant_type": "refresh_token",
            }))
        {
            Ok(response) => response,
            Err(ureq::Error::Status(400, response)) => {
                log("Refresh token is invalid or expired, need to reauthorize");
                if let Ok(error_body) = response.into_string() {
                    log(&format!("Error details: {}", error_body));
                }
                self.authorize_app();
                return;
            }
            Err(ureq::Error::Status(code, response)) => {
                log(&format!("Failed to refresh token: HTTP {}", code));
                if let Ok(error_body) = response.into_string() {
                    log(&format!("Error details: {}", error_body));
                }
                return;
            }
            Err(e) => {
                log(&format!("Network error during token refresh: {}", e));
                return;
            }
        };

        let json_response: Option<TraktAccessToken> = response.into_json().unwrap_or_default();

        if let Some(json_response) = json_response {
            log("Successfully refreshed OAuth access token");
            self.trakt_access_token = Some(json_response.access_token.clone());
            self.trakt_refresh_token = Some(json_response.refresh_token.clone());

            // Calculate refresh token expiry (3 months from now)
            let now = Utc::now().timestamp() as u64;
            self.trakt_refresh_token_expires_at = Some(now + 60 * 60 * 24 * 30 * 3); // 3 months

            set_oauth_tokens(&json_response);

            log(&format!(
                "Token refreshed successfully, new refresh token expires at: {}",
                DateTime::from_timestamp(self.trakt_refresh_token_expires_at.unwrap() as i64, 0)
                    .unwrap()
                    .to_rfc3339_opts(SecondsFormat::Secs, true)
            ));
        } else {
            log("Failed to parse refresh token response from Trakt API");
            log("Will attempt full reauthorization");
            self.authorize_app();
        }
    }
}

fn find_config_file() -> Option<PathBuf> {
    let config_path = dirs::config_dir().unwrap().join("discrakt");
    let mut exe_path = env::current_exe().unwrap();
    exe_path.pop();

    let locations = vec![config_path, exe_path];

    for location in &locations {
        let config_file = location.join("credentials.ini");
        if config_file.exists() {
            return Some(config_file);
        }
    }
    log(&format!(
        "Could not find credentials.ini in {:?}",
        locations
            .iter()
            .map(|loc| loc.to_str().to_owned().unwrap())
            .collect::<Vec<_>>()
    ));
    None
}

pub fn load_config() -> Env {
    let mut config = Ini::new();
    let config_file = find_config_file();

    let path = config_file.expect("Could not find credentials.ini");
    config.load(path).expect("Failed to load credentials.ini");

    Env {
        discord_client_id: "826189107046121572".to_string(),
        trakt_username: config
            .get("Trakt API", "traktUser")
            .expect("traktUser not found"),
        trakt_client_id: config
            .get("Trakt API", "traktClientID")
            .expect("traktClientID not found"),
        trakt_oauth_enabled: config
            .getbool("Trakt API", "enabledOAuth")
            .expect("enableOAuth not found")
            .unwrap_or(false),
        trakt_client_secret: config.get("Trakt API", "traktClientSecret"),
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

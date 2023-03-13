use std::{io, time::Duration};

use chrono::{SecondsFormat, Utc};
use configparser::ini::Ini;
use serde::Deserialize;
use ureq::AgentBuilder;

#[derive(Deserialize)]
pub struct TraktAccessToken {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u64,
    pub refresh_token: String,
    pub scope: String,
    pub created_at: u64,
}

pub struct Env {
    pub discord_token: String,
    pub trakt_username: String,
    pub trakt_client_id: String,
    pub trakt_oauth_enabled: bool,
    pub trakt_client_secret: Option<String>,
    pub trakt_access_token: Option<String>,
    pub trakt_refresh_token: Option<String>,
    pub trakt_refresh_token_expires_at: Option<u64>,
}

impl Env {
    pub fn check_oauth(&mut self) {
        if self.trakt_oauth_enabled {
            if self.trakt_access_token.is_none()
                || self.trakt_access_token.as_ref().unwrap().is_empty()
            {
                self.authorize_app();
            } else if let Some(expires_at) = self.trakt_refresh_token_expires_at {
                if Utc::now().timestamp() as u64 > expires_at {
                    self.exchange_refresh_token_for_access_token();
                }
            }
        }
    }

    fn authorize_app(&mut self) {
        if webbrowser::open(
            &format!("https://trakt.tv/oauth/authorize?response_type=code&client_id={}&redirect_uri=urn:ietf:wg:oauth:2.0:oob", self.trakt_client_id)
        ).is_err() {
            eprintln!("Failed to open webbrowser to authorize discrakt");
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
            Err(_) => return,
        };

        let json_response: Option<TraktAccessToken> = match response.into_json() {
            Ok(body) => body,
            Err(_) => None,
        };

        if let Some(json_response) = json_response {
            self.trakt_access_token = Some(json_response.access_token.clone());
            self.trakt_refresh_token = Some(json_response.refresh_token.clone());
            self.trakt_refresh_token_expires_at =
                Some(json_response.created_at + 60 * 60 * 24 * 30 * 3); // secs * mins * hours * days * months => 3 months
            set_oauth_tokens(&json_response);
        } else {
            eprintln!("Failed to exchange code for access token");
        }
    }

    fn exchange_refresh_token_for_access_token(&mut self) {
        let agent = AgentBuilder::new()
            .timeout_read(Duration::from_secs(5))
            .timeout_write(Duration::from_secs(5))
            .build();
        let response = match agent
            .post("https://api.trakt.tv/oauth/token")
            .set("Content-Type", "application/json")
            .send_json(ureq::json!({
                "code": "Get the code from the webbrowser",
                "client_id": self.trakt_client_id,
                "client_secret": self.trakt_client_secret.as_ref().expect("client_secret not found"),
                "redirect_uri": "urn:ietf:wg:oauth:2.0:oob",
                "grant_type": "refresh_token",
            }))
        {
            Ok(response) => response,
            Err(_) => return,
        };

        let json_response: Option<TraktAccessToken> = match response.into_json() {
            Ok(body) => body,
            Err(_) => None,
        };

        if let Some(json_response) = json_response {
            self.trakt_access_token = Some(json_response.access_token.clone());
            self.trakt_refresh_token = Some(json_response.refresh_token.clone());
            self.trakt_refresh_token_expires_at =
                Some(json_response.created_at + 60 * 60 * 24 * 30 * 3); // secs * mins * hours * days * months => 3 months
            set_oauth_tokens(&json_response);
        } else {
            eprintln!("Failed to exchange refresh token for access token");
        }
    }
}

pub fn load_config() -> Env {
    let mut config = Ini::new();
    config.load("credentials.ini").unwrap();

    Env {
        discord_token: config
            .get("Discord", "discordClientID")
            .expect("discordClientID not found"),
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
    }
}

fn set_oauth_tokens(json_response: &TraktAccessToken) {
    let mut config = Ini::new_cs();
    config
        .load("credentials.ini")
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
    config.set(
        "Trakt API",
        "OAuthRefreshTokenExpiresAt",
        Some(json_response.created_at.to_string()),
    );
    config
        .write("credentials.ini")
        .expect("Failed to write credentials.ini");
}

pub fn log(message: &str) {
    println!(
        "{} : {}",
        Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
        message
    );
}

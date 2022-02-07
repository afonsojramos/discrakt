use configparser::ini::Ini;

pub struct Env {
    pub discord_token: String,
    pub trakt_username: String,
    pub trakt_client_id: String,
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
    }
}

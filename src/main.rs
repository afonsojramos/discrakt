use chrono::{SecondsFormat, Utc};
use discrakt::{config::load_config, discord::Discord, trakt::Trakt};
use std::{thread::sleep, time::Duration};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = load_config();
    let mut discord = Discord::new(cfg.discord_token);
    let mut trakt = Trakt::new(cfg.trakt_client_id, cfg.trakt_username);
    Discord::connect(&mut discord);

    loop {
        sleep(Duration::from_secs(15));

        let response = match Trakt::get_watching(&trakt) {
            Some(response) => response,
            None => {
                println!(
                    "{} : Nothing is being played",
                    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
                );
                // resets the connection to also reset the activity
                Discord::close(&mut discord);
                continue;
            }
        };

        Discord::set_activity(&mut discord, &response, &mut trakt);
    }
}

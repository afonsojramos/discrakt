use discrakt::{config::load_config, discord, trakt::Trakt};
use std::{thread::sleep, time::Duration};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = load_config();
    let mut discord_client = discord::new(cfg.discord_token)?;
    let mut trakt = Trakt::new(cfg.trakt_client_id, cfg.trakt_username);
    discord::connect(&mut discord_client);

    loop {
        sleep(Duration::from_secs(15));

        let response = match Trakt::get_watching(&trakt) {
            Some(response) => response,
            None => {
                println!("Nothing is being played");
                continue;
            }
        };

        discord::set_activity(&mut discord_client, &response, &mut trakt);
    }
}

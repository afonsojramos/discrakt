use discrakt::{config::load_config, discord, trakt};
use std::{thread::sleep, time::Duration};
use ureq::{Agent, AgentBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = load_config();
    let mut discord_client = discord::new(cfg.discord_token)?;
    let agent: Agent = AgentBuilder::new()
        .timeout_read(Duration::from_secs(5))
        .timeout_write(Duration::from_secs(5))
        .build();
    discord::connect(&mut discord_client);

    loop {
        sleep(Duration::from_secs(15));

        let response = match trakt::get_watching(&agent, &cfg.trakt_username, &cfg.trakt_client_id)
        {
            Some(response) => response,
            None => {
                println!("Nothing is being played");
                continue;
            }
        };

        discord::set_activity(&mut discord_client, &response);
    }
}

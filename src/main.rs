use discrakt::{
    discord::Discord,
    trakt::Trakt,
    utils::{load_config, log},
};
use std::{thread::sleep, time::Duration};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

fn init_logging() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let console_layer = fmt::layer()
        .with_target(true)
        .with_level(true)
        .with_thread_names(true);

    tracing_subscriber::registry()
        .with(filter)
        .with(console_layer)
        .init();
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_logging();
    let mut cfg = load_config();
    cfg.check_oauth();
    let mut discord = Discord::new(cfg.discord_client_id);
    let mut trakt = Trakt::new(
        cfg.trakt_client_id,
        cfg.trakt_username,
        cfg.trakt_access_token,
    );
    let tmdb_token = cfg.tmdb_token;
    Discord::connect(&mut discord);

    loop {
        sleep(Duration::from_secs(15));

        let response = match Trakt::get_watching(&trakt) {
            Some(response) => response,
            None => {
                log("Nothing is being played");
                // resets the connection to also reset the activity
                Discord::close(&mut discord);
                continue;
            }
        };

        Discord::set_activity(&mut discord, &response, &mut trakt, tmdb_token.clone());
    }
}

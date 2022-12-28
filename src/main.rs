use discrakt::{
    discord::Discord,
    trakt::Trakt,
    utils::{load_config, log},
};
use std::{thread::sleep, time::Duration};
use tmdb::themoviedb::TMDb;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = load_config();
    let mut discord = Discord::new(cfg.discord_token);
    let mut trakt = Trakt::new(cfg.trakt_client_id, cfg.trakt_username);
    let tmdb_token: String = match cfg.tmdb_token {
        // Burner account for out-of-the-box fetching
        None => "835a79f3fecf7c9dfb6a767699ceac90".to_string(),
        Some(token) => token
    };

    let tmdb_token_leaked: &'static str = Box::leak(tmdb_token.into_boxed_str());
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


        let tmdb = TMDb {
            api_key: tmdb_token_leaked,
            language: "en",
        };
        
        Discord::set_activity(&mut discord, &response, &mut trakt, &tmdb);
    }
}

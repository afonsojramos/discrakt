// Hide console window on Windows in release builds
#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

use discrakt::{
    discord::Discord,
    state::AppState,
    trakt::Trakt,
    tray::{Tray, TrayCommand},
    utils::{get_watch_stats, load_config, log},
};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, RwLock,
    },
    thread,
    time::Duration,
};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::WindowId;

#[cfg(target_os = "macos")]
fn hide_dock_icon() {
    use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
    use objc2_foundation::MainThreadMarker;

    if let Some(mtm) = MainThreadMarker::new() {
        let app = NSApplication::sharedApplication(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    }
}

#[cfg(not(target_os = "macos"))]
fn hide_dock_icon() {}

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

struct App {
    tray: Option<Tray>,
    app_state: Arc<RwLock<AppState>>,
    should_quit: Arc<AtomicBool>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {
        // Hide dock icon when app is fully resumed
        hide_dock_icon();
    }

    fn window_event(&mut self, _event_loop: &ActiveEventLoop, _id: WindowId, _event: WindowEvent) {}

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(ref mut tray) = self.tray {
            // Update tray status from shared state
            tray.update_status(&self.app_state);

            if let Some(command) = tray.poll_events(&self.app_state) {
                match command {
                    TrayCommand::Quit => {
                        self.should_quit.store(true, Ordering::Relaxed);
                        event_loop.exit();
                    }
                    TrayCommand::TogglePause => {
                        // State is already updated in poll_events
                    }
                }
            }
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_logging();
    let mut cfg = load_config();
    cfg.check_oauth();

    let app_state = AppState::new();
    let should_quit = Arc::new(AtomicBool::new(false));

    let app_state_clone = Arc::clone(&app_state);
    let should_quit_clone = Arc::clone(&should_quit);

    let discord_client_id = cfg.discord_client_id.clone();
    let trakt_client_id = cfg.trakt_client_id.clone();
    let trakt_username = cfg.trakt_username.clone();
    let trakt_access_token = cfg.trakt_access_token.clone();
    let tmdb_token = cfg.tmdb_token.clone();

    // Spawn background polling thread
    let polling_handle = thread::spawn(move || {
        let mut discord = match Discord::new(discord_client_id) {
            Ok(d) => d,
            Err(e) => {
                log(&format!("Failed to create Discord client: {e}"));
                return;
            }
        };
        let mut trakt = Trakt::new(trakt_client_id, trakt_username, trakt_access_token);

        Discord::connect(&mut discord);

        // Update state: Discord connected
        if let Ok(mut state) = app_state_clone.write() {
            state.set_discord_connected(true);
        }

        while !should_quit_clone.load(Ordering::Relaxed) {
            // Sleep in small increments to allow for responsive shutdown
            for _ in 0..15 {
                if should_quit_clone.load(Ordering::Relaxed) {
                    break;
                }
                thread::sleep(Duration::from_secs(1));
            }

            if should_quit_clone.load(Ordering::Relaxed) {
                break;
            }

            // Check if paused from shared state
            let is_paused = app_state_clone.read().map(|s| s.is_paused).unwrap_or(false);

            if is_paused {
                Discord::close(&mut discord);
                continue;
            }

            let response = match Trakt::get_watching(&trakt) {
                Some(response) => response,
                None => {
                    log("Nothing is being played");
                    // Update state: nothing playing
                    if let Ok(mut state) = app_state_clone.write() {
                        state.clear_watching();
                    }
                    Discord::close(&mut discord);
                    continue;
                }
            };

            // Update state with current watching info
            if let Ok(mut state) = app_state_clone.write() {
                let watch_stats = get_watch_stats(&response);
                let (title, details) = match response.r#type.as_str() {
                    "movie" => {
                        let movie = response.movie.as_ref().unwrap();
                        (
                            format!("{} ({})", movie.title, movie.year),
                            "Movie".to_string(),
                        )
                    }
                    "episode" => {
                        let show = response.show.as_ref().unwrap();
                        let episode = response.episode.as_ref().unwrap();
                        (
                            show.title.clone(),
                            format!(
                                "S{:02}E{:02} - {}",
                                episode.season, episode.number, episode.title
                            ),
                        )
                    }
                    _ => ("Unknown".to_string(), "".to_string()),
                };
                state.set_watching(title, details, watch_stats.watch_percentage);
            }

            Discord::set_activity(&mut discord, &response, &mut trakt, tmdb_token.clone());
        }

        Discord::close(&mut discord);
        log("Polling thread stopped");
    });

    // Create event loop - must be done on main thread
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);

    // Hide dock icon AFTER event loop creates NSApplication
    hide_dock_icon();

    // Initialize tray after event loop is created
    let tray = Tray::new()?;

    log("Discrakt is running in the system tray");

    let mut app = App {
        tray: Some(tray),
        app_state: Arc::clone(&app_state),
        should_quit: Arc::clone(&should_quit),
    };

    // Run the event loop
    event_loop.run_app(&mut app)?;

    // Wait for polling thread to finish
    should_quit.store(true, Ordering::Relaxed);
    polling_handle.join().expect("Polling thread panicked");

    log("Discrakt exited gracefully");
    Ok(())
}

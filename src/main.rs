// Hide console window on Windows in release builds
#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

use discrakt::{
    autostart,
    discord::Discord,
    state::AppState,
    trakt::Trakt,
    tray::{Tray, TrayCommand},
    utils::{get_watch_stats, load_config, log_dir_path, DEFAULT_DISCORD_APP_ID},
};
use std::{
    env, process,
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

/// On Windows with hidden console (windows_subsystem = "windows"), attach to parent console
/// for CLI output. This allows -V, -h, etc. to work when run from cmd.exe or PowerShell.
#[cfg(target_os = "windows")]
fn attach_console() {
    extern "system" {
        fn AttachConsole(dw_process_id: u32) -> i32;
    }
    const ATTACH_PARENT_PROCESS: u32 = 0xFFFFFFFF;

    // SAFETY: AttachConsole is a standard Windows API call that either succeeds
    // (attaches to parent's console) or fails gracefully (returns 0). It has no
    // memory safety implications - it only affects stdio handle routing.
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

#[cfg(not(target_os = "windows"))]
fn attach_console() {
    // No-op on non-Windows platforms
}

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

/// Platform-specific initialization.
/// On Linux, ksni handles its own D-Bus event loop, so no GTK initialization is needed.
fn platform_init() -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}

fn init_logging() -> Option<tracing_appender::non_blocking::WorkerGuard> {
    // Default to warn level for minimal logging in production
    // Users can set RUST_LOG=info or RUST_LOG=debug for verbose output
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn"));

    // Log to file only (no console output during normal operation)
    // Console output is only used for CLI flags like --help which print and exit before logging
    // This applies to all platforms since tray apps typically run without a terminal attached
    let log_dir = log_dir_path();

    // Ensure log directory exists
    if let Err(e) = std::fs::create_dir_all(&log_dir) {
        eprintln!(
            "Warning: Failed to create log directory {:?}: {}",
            log_dir, e
        );
    }

    // Use builder to get proper filename format: discrakt.YYYY-MM-DD.log
    // Keep only 7 days of logs to prevent unbounded disk usage
    let file_appender = tracing_appender::rolling::RollingFileAppender::builder()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix("discrakt")
        .filename_suffix("log")
        .max_log_files(7)
        .build(&log_dir)
        .expect("Failed to create log file appender");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let file_layer = fmt::layer()
        .with_target(true)
        .with_level(true)
        .with_thread_names(true)
        .with_ansi(false) // Disable ANSI colors for file output
        .with_writer(non_blocking);

    tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .init();

    Some(guard)
}

fn print_help() {
    let log_dir = log_dir_path();
    println!(
        "Discrakt - Trakt to Discord Rich Presence

Usage: discrakt [OPTIONS]

Options:
    --autostart <VALUE>  Control automatic startup at login
                         VALUES: 1, true, on  = enable
                                 0, false, off = disable
    --version, -V        Show version information
    --help, -h           Show this help message

When run without options, Discrakt starts normally and runs in
the system tray, updating your Discord status based on Trakt.

Logging:
    Logs are written to: {}
    Files are named discrakt.YYYY-MM-DD.log (daily rotation).
    Only warnings and errors are logged by default.
    Old logs are automatically deleted after 7 days.
    Set RUST_LOG=info or RUST_LOG=debug for verbose output.

Examples:
    discrakt                  Start Discrakt normally
    discrakt --autostart 1    Enable start at login and exit
    discrakt --autostart=off  Disable start at login and exit",
        log_dir.display()
    );
}

fn handle_autostart_arg(value: &str) -> ! {
    match value {
        "1" | "true" | "on" => match autostart::enable() {
            Ok(()) => {
                println!("Autostart enabled. Discrakt will start automatically at login.");
                process::exit(0);
            }
            Err(e) => {
                eprintln!("Failed to enable autostart: {}", e);
                process::exit(1);
            }
        },
        "0" | "false" | "off" => match autostart::disable() {
            Ok(()) => {
                println!("Autostart disabled.");
                process::exit(0);
            }
            Err(e) => {
                eprintln!("Failed to disable autostart: {}", e);
                process::exit(1);
            }
        },
        _ => {
            eprintln!("Invalid value for --autostart: '{}'", value);
            eprintln!("Valid values: 1, true, on (enable) or 0, false, off (disable)");
            process::exit(1);
        }
    }
}

fn parse_args() {
    let args: Vec<String> = env::args().collect();

    // Process first argument only - all current options exit immediately
    if let Some(arg) = args.get(1) {
        match arg.as_str() {
            "--help" | "-h" => {
                print_help();
                process::exit(0);
            }
            "--version" | "-V" => {
                // Use DISCRAKT_VERSION if set by build.rs (from release workflow),
                // otherwise fall back to Cargo.toml version
                let version = option_env!("DISCRAKT_VERSION").unwrap_or(env!("CARGO_PKG_VERSION"));
                println!("discrakt {}", version);
                process::exit(0);
            }
            "--autostart" => {
                let value = args.get(2).map(String::as_str).unwrap_or_else(|| {
                    eprintln!("Error: --autostart requires a value");
                    eprintln!("Use --help for usage information.");
                    process::exit(1);
                });
                handle_autostart_arg(value);
            }
            arg if arg.starts_with("--autostart=") => {
                let value = arg.strip_prefix("--autostart=").unwrap();
                if value.is_empty() {
                    eprintln!("Error: --autostart requires a value");
                    eprintln!("Use --help for usage information.");
                    process::exit(1);
                }
                handle_autostart_arg(value);
            }
            arg => {
                eprintln!("Unknown option: {}", arg);
                eprintln!("Use --help for usage information.");
                process::exit(1);
            }
        }
    }
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
        // Wake up every second to update tray status
        event_loop.set_control_flow(ControlFlow::wait_duration(Duration::from_secs(1)));

        if let Some(ref mut tray) = self.tray {
            // Update tray status from shared state
            tray.update_status(&self.app_state);

            if let Some(command) = tray.poll_events(&self.app_state) {
                match command {
                    TrayCommand::Quit => {
                        self.should_quit.store(true, Ordering::Relaxed);
                        event_loop.exit();
                    }
                    TrayCommand::TogglePause | TrayCommand::ToggleAutostart => {
                        // State is already updated in poll_events
                    }
                }
            }
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Always try to attach to parent console on Windows.
    // If launched from Explorer, this safely fails (no parent console).
    // If launched from cmd/PowerShell, enables console output for logs and CLI flags.
    attach_console();

    // Handle CLI arguments first (before logging, as --help/--autostart exit immediately)
    parse_args();

    // Keep the guard alive for the duration of the program (Windows file logging)
    let _log_guard = init_logging();

    // Platform-specific initialization
    platform_init()?;

    let mut cfg = load_config().map_err(|e| {
        tracing::error!("Failed to load configuration: {}", e);
        e
    })?;
    cfg.check_oauth();

    let app_state = AppState::new();
    let should_quit = Arc::new(AtomicBool::new(false));

    let app_state_clone = Arc::clone(&app_state);
    let should_quit_clone = Arc::clone(&should_quit);

    let trakt_client_id = cfg.trakt_client_id.clone();
    let trakt_username = cfg.trakt_username.clone();
    let trakt_access_token = cfg.trakt_access_token.clone();
    let tmdb_token = cfg.tmdb_token.clone();

    // Spawn background polling thread
    let polling_handle = thread::spawn(move || {
        let mut discord = Discord::new(DEFAULT_DISCORD_APP_ID.to_string());
        let mut trakt = Trakt::new(trakt_client_id, trakt_username, trakt_access_token);

        discord.connect();

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
                discord.close();
                continue;
            }

            let response = match Trakt::get_watching(&trakt) {
                Some(response) => response,
                None => {
                    tracing::debug!("Nothing is being played");
                    // Update state: nothing playing
                    if let Ok(mut state) = app_state_clone.write() {
                        state.clear_watching();
                    }
                    discord.close();
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

            discord.set_activity(&response, &mut trakt, tmdb_token.clone());
        }

        discord.close();
        tracing::info!("Polling thread stopped");
    });

    // Create event loop - must be done on main thread
    let event_loop = EventLoop::new()?;

    // Hide dock icon AFTER event loop creates NSApplication
    hide_dock_icon();

    // Initialize tray after event loop is created
    let tray = Tray::new()?;

    tracing::info!("Discrakt is running in the system tray");

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

    tracing::info!("Discrakt exited gracefully");
    Ok(())
}

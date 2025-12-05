pub mod autostart;
pub mod discord;
pub mod setup;
pub mod state;
pub mod trakt;
pub mod utils;

// Platform-specific tray implementations:
// - Linux: ksni (KDE StatusNotifierItem) for native KDE/freedesktop support
// - Windows/macOS: tray-icon crate
#[cfg(target_os = "linux")]
#[path = "tray_linux.rs"]
pub mod tray;

#[cfg(not(target_os = "linux"))]
pub mod tray;

use std::sync::{Arc, RwLock};

#[derive(Clone, Default)]
pub struct AppState {
    pub current_watching: Option<WatchingInfo>,
    pub discord_connected: bool,
    pub is_paused: bool,
    pub last_error: Option<String>,
}

#[derive(Clone)]
pub struct WatchingInfo {
    pub title: String,
    pub details: String,
    pub progress: String,
}

impl AppState {
    pub fn new() -> Arc<RwLock<Self>> {
        Arc::new(RwLock::new(Self::default()))
    }

    pub fn set_watching(&mut self, title: String, details: String, progress: String) {
        self.current_watching = Some(WatchingInfo {
            title,
            details,
            progress,
        });
    }

    pub fn clear_watching(&mut self) {
        self.current_watching = None;
    }

    pub fn set_discord_connected(&mut self, connected: bool) {
        self.discord_connected = connected;
    }

    pub fn set_paused(&mut self, paused: bool) {
        self.is_paused = paused;
    }

    pub fn set_error(&mut self, error: Option<String>) {
        self.last_error = error;
    }

    pub fn status_text(&self) -> String {
        if self.is_paused {
            return "Paused".to_string();
        }

        if !self.discord_connected {
            return "Connecting to Discord...".to_string();
        }

        match &self.current_watching {
            Some(info) => format!("{} - {}", info.title, info.details),
            None => "Nothing playing".to_string(),
        }
    }
}

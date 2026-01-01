//! Linux-specific tray implementation using ksni (KDE StatusNotifierItem).
//!
//! This module provides native integration with KDE Plasma and other desktop environments
//! that support the freedesktop StatusNotifierItem specification.

use crossbeam_channel::{Receiver, Sender};
use ksni::blocking::TrayMethods;
use ksni::menu::*;
use std::sync::{Arc, RwLock};

use crate::autostart;
use crate::state::AppState;
use crate::utils::{create_dark_icon, is_light_mode, LANGUAGES};

/// Commands that can be triggered from the tray menu.
pub enum TrayCommand {
    Quit,
    TogglePause,
    ToggleAutostart,
    SetLanguage(String),
}

/// Internal state shared between the tray icon and the main application.
struct TrayState {
    is_paused: bool,
    autostart_enabled: bool,
    status_text: String,
    command_sender: Sender<TrayCommand>,
}

/// The ksni tray implementation.
struct DiscraktTray {
    state: Arc<RwLock<TrayState>>,
}

impl ksni::Tray for DiscraktTray {
    // Make left-click open the menu (same as right-click)
    const MENU_ON_ACTIVATE: bool = true;

    fn id(&self) -> String {
        "discrakt".into()
    }

    fn icon_name(&self) -> String {
        // Use the installed icon from the system icon theme
        // Falls back to a generic icon if not found
        "discrakt".into()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        // Embed the icon directly as ARGB32 data
        let icon_bytes = include_bytes!("assets/icon.png");
        if let Ok(image) = image::load_from_memory(icon_bytes) {
            let rgba = image.to_rgba8();

            // Use dark (inverted) icon for light mode, original white icon for dark mode
            let final_image = if is_light_mode() {
                create_dark_icon(&rgba)
            } else {
                rgba
            };

            let (width, height) = final_image.dimensions();

            // Convert RGBA to ARGB (ksni expects ARGB format)
            let mut argb_data = Vec::with_capacity((width * height * 4) as usize);
            for pixel in final_image.pixels() {
                argb_data.push(pixel[3]); // A
                argb_data.push(pixel[0]); // R
                argb_data.push(pixel[1]); // G
                argb_data.push(pixel[2]); // B
            }

            vec![ksni::Icon {
                width: width as i32,
                height: height as i32,
                data: argb_data,
            }]
        } else {
            vec![]
        }
    }

    fn title(&self) -> String {
        "Discrakt".into()
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        let description = self
            .state
            .read()
            .map(|s| format!("Discrakt: {}", s.status_text))
            .unwrap_or_else(|_| "Discrakt - Trakt to Discord".into());

        ksni::ToolTip {
            icon_name: String::new(),
            icon_pixmap: vec![],
            title: "Discrakt".into(),
            description,
        }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let state = self.state.read().ok();
        let (is_paused, autostart_enabled, status_text) = state
            .map(|s| (s.is_paused, s.autostart_enabled, s.status_text.clone()))
            .unwrap_or((false, false, "Starting...".into()));

        let mut lang_items = Vec::new();
        for (name, code) in LANGUAGES {
            let code_clone = code.to_string();
            lang_items.push(
                StandardItem {
                    label: name.to_string(),
                    activate: Box::new(move |tray: &mut Self| {
                        if let Ok(state) = tray.state.read() {
                            let _ = state
                                .command_sender
                                .send(TrayCommand::SetLanguage(code_clone.clone()));
                        }
                    }),
                    ..Default::default()
                }
                .into(),
            );
        }

        let lang_submenu = Submenu {
            label: "Language".into(),
            children: lang_items,
            ..Default::default()
        };

        vec![
            // Status display (disabled item, just for showing info)
            StandardItem {
                label: status_text,
                enabled: false,
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            // Pause/Resume toggle
            StandardItem {
                label: if is_paused { "Resume" } else { "Pause" }.into(),
                enabled: true,
                activate: Box::new(|tray: &mut Self| {
                    if let Ok(state) = tray.state.read() {
                        let _ = state.command_sender.send(TrayCommand::TogglePause);
                    }
                }),
                ..Default::default()
            }
            .into(),
            // Autostart toggle (checkmark item)
            CheckmarkItem {
                label: "Start at Login".into(),
                enabled: true,
                checked: autostart_enabled,
                activate: Box::new(|tray: &mut Self| {
                    if let Ok(state) = tray.state.read() {
                        let _ = state.command_sender.send(TrayCommand::ToggleAutostart);
                    }
                }),
                ..Default::default()
            }
            .into(),
            lang_submenu.into(),
            MenuItem::Separator,
            // Quit item
            StandardItem {
                label: "Quit Discrakt".into(),
                enabled: true,
                activate: Box::new(|tray: &mut Self| {
                    if let Ok(state) = tray.state.read() {
                        let _ = state.command_sender.send(TrayCommand::Quit);
                    }
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

/// Handle to the tray icon, allowing updates from the main thread.
pub struct Tray {
    handle: ksni::blocking::Handle<DiscraktTray>,
    tray_state: Arc<RwLock<TrayState>>,
    command_receiver: Receiver<TrayCommand>,
    last_status: String,
}

impl Tray {
    /// Creates a new system tray icon.
    ///
    /// This spawns a background task to handle the D-Bus StatusNotifierItem protocol.
    /// The tray icon will appear in KDE Plasma and other compatible desktop environments.
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let (command_sender, command_receiver) = crossbeam_channel::unbounded();

        let tray_state = Arc::new(RwLock::new(TrayState {
            is_paused: false,
            autostart_enabled: autostart::is_enabled(),
            status_text: "Starting...".into(),
            command_sender,
        }));

        let tray = DiscraktTray {
            state: Arc::clone(&tray_state),
        };

        // Spawn the tray using the blocking API (ksni handles the event loop internally)
        let handle = tray.spawn().map_err(|e| {
            tracing::error!("Failed to spawn ksni tray: {}", e);
            Box::new(std::io::Error::other(format!("ksni spawn failed: {}", e)))
                as Box<dyn std::error::Error>
        })?;

        tracing::info!("System tray initialized (ksni/StatusNotifierItem)");

        Ok(Tray {
            handle,
            tray_state,
            command_receiver,
            last_status: String::new(),
        })
    }

    /// Updates the tray status display based on the current application state.
    pub fn update_status(&mut self, state: &Arc<RwLock<AppState>>) {
        if let Ok(app_state) = state.read() {
            let status = app_state.status_text();
            let is_paused = app_state.is_paused;

            if status != self.last_status {
                if let Ok(mut tray_state) = self.tray_state.write() {
                    tray_state.status_text = status.clone();
                    tray_state.is_paused = is_paused;
                }

                // Signal ksni to refresh the tray
                self.handle.update(|_| {});

                self.last_status = status;
            }
        }
    }

    /// Polls for menu events and returns any command that was triggered.
    pub fn poll_events(&mut self, state: &Arc<RwLock<AppState>>) -> Option<TrayCommand> {
        if let Ok(command) = self.command_receiver.try_recv() {
            match &command {
                TrayCommand::Quit => {
                    tracing::info!("Quit requested from tray menu");
                }
                TrayCommand::TogglePause => {
                    if let Ok(mut app_state) = state.write() {
                        let new_paused = !app_state.is_paused;
                        app_state.set_paused(new_paused);

                        if let Ok(mut tray_state) = self.tray_state.write() {
                            tray_state.is_paused = new_paused;
                        }

                        // Refresh the tray menu
                        self.handle.update(|_| {});

                        if new_paused {
                            tracing::info!("Paused from tray menu");
                        } else {
                            tracing::info!("Resumed from tray menu");
                        }
                    }
                }
                TrayCommand::ToggleAutostart => {
                    match autostart::toggle() {
                        Ok(enabled) => {
                            if let Ok(mut tray_state) = self.tray_state.write() {
                                tray_state.autostart_enabled = enabled;
                            }
                            // Refresh the tray menu
                            self.handle.update(|_| {});
                            tracing::info!(
                                "Autostart {}",
                                if enabled { "enabled" } else { "disabled" }
                            );
                        }
                        Err(e) => {
                            tracing::error!("Failed to toggle autostart: {}", e);
                            // Revert checkbox to actual state
                            if let Ok(mut tray_state) = self.tray_state.write() {
                                tray_state.autostart_enabled = autostart::is_enabled();
                            }
                            self.handle.update(|_| {});
                        }
                    }
                }
                TrayCommand::SetLanguage(code) => {
                    if let Ok(mut app_state) = state.write() {
                        app_state.pending_language = Some(code.clone());
                    }
                    tracing::info!("Language changed to: {}", code);
                }
            }
            return Some(command);
        }
        None
    }
}

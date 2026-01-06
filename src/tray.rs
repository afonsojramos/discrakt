//! System tray implementation using tray-icon (Windows/macOS).
//!
//! This module is only compiled on non-Linux platforms.
//! Linux uses the ksni-based implementation in tray_linux.rs.

use crossbeam_channel::Receiver;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tray_icon::{
    menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu},
    Icon, TrayIcon, TrayIconBuilder,
};

use crate::autostart;
use crate::state::AppState;
use crate::utils::{create_dark_icon, is_light_mode, LANGUAGES};

pub enum TrayCommand {
    Quit,
    TogglePause,
    ToggleAutostart,
    SetLanguage(String),
}

pub struct Tray {
    tray_icon: TrayIcon,
    menu_receiver: Receiver<MenuEvent>,
    quit_item_id: tray_icon::menu::MenuId,
    pause_item_id: tray_icon::menu::MenuId,
    autostart_item_id: tray_icon::menu::MenuId,
    pause_item: MenuItem,
    autostart_item: CheckMenuItem,
    status_item: MenuItem,
    last_status: String,
    language_items: HashMap<String, CheckMenuItem>,
}

impl Tray {
    pub fn new(current_language: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let icon = Self::load_icon()?;

        // Status display (disabled, just for showing info)
        let status_item = MenuItem::new("Starting...", false, None);
        let pause_item = MenuItem::new("Pause", true, None);
        let autostart_item =
            CheckMenuItem::new("Start at Login", true, autostart::is_enabled(), None);
        let quit_item = MenuItem::new("Quit Discrakt", true, None);

        let pause_item_id = pause_item.id().clone();
        let autostart_item_id = autostart_item.id().clone();
        let quit_item_id = quit_item.id().clone();

        let lang_submenu = Submenu::new("Language", true);
        let mut language_items = HashMap::new();

        for (name, code) in LANGUAGES {
            let is_checked = *code == current_language;
            let item = CheckMenuItem::new(*name, true, is_checked, None);

            language_items.insert(code.to_string(), item.clone());
            lang_submenu.append(&item)?;
        }

        let menu = Menu::new();
        menu.append(&status_item)?;
        menu.append(&PredefinedMenuItem::separator())?;
        menu.append(&pause_item)?;
        menu.append(&autostart_item)?;
        menu.append(&lang_submenu)?;
        menu.append(&PredefinedMenuItem::separator())?;
        menu.append(&quit_item)?;

        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("Discrakt - Trakt to Discord")
            .with_icon(icon)
            .build()?;

        let menu_receiver = MenuEvent::receiver().clone();

        tracing::info!("System tray initialized");

        Ok(Tray {
            tray_icon,
            menu_receiver,
            quit_item_id,
            pause_item_id,
            autostart_item_id,
            pause_item,
            autostart_item,
            status_item,
            last_status: String::new(),
            language_items,
        })
    }

    fn load_icon() -> Result<Icon, Box<dyn std::error::Error>> {
        let icon_bytes = include_bytes!("assets/icon.png");
        let image = image::load_from_memory(icon_bytes)?;
        let rgba = image.to_rgba8();

        // Use dark (inverted) icon for light mode, original white icon for dark mode
        let final_image = if is_light_mode() {
            tracing::debug!("Light mode detected, using dark tray icon");
            create_dark_icon(&rgba)
        } else {
            tracing::debug!("Dark mode detected, using light tray icon");
            rgba
        };

        let (width, height) = final_image.dimensions();
        Icon::from_rgba(final_image.into_raw(), width, height).map_err(|e| e.into())
    }

    fn update_language_checks(&self, selected_code: &str) {
        for (code, item) in &self.language_items {
            item.set_checked(code == selected_code);
        }
    }

    pub fn update_status(&mut self, state: &Arc<RwLock<AppState>>) {
        if let Ok(state) = state.read() {
            let status = state.status_text();
            if status != self.last_status {
                self.status_item.set_text(&status);
                let _ = self
                    .tray_icon
                    .set_tooltip(Some(&format!("Discrakt: {}", status)));
                self.last_status = status;
            }
        }
    }

    pub fn poll_events(&mut self, state: &Arc<RwLock<AppState>>) -> Option<TrayCommand> {
        if let Ok(event) = self.menu_receiver.try_recv() {
            for (code, item) in &self.language_items {
                if event.id == item.id() {
                    self.update_language_checks(code);

                    if let Ok(mut app_state) = state.write() {
                        app_state.pending_language = Some(code.clone());
                    }
                    tracing::info!("Language changed to: {}", code);
                    return Some(TrayCommand::SetLanguage(code.clone()));
                }
            }

            if event.id == self.quit_item_id {
                tracing::info!("Quit requested from tray menu");
                return Some(TrayCommand::Quit);
            } else if event.id == self.pause_item_id {
                if let Ok(mut app_state) = state.write() {
                    let new_paused = !app_state.is_paused;
                    app_state.set_paused(new_paused);
                    if new_paused {
                        self.pause_item.set_text("Resume");
                        tracing::info!("Paused from tray menu");
                    } else {
                        self.pause_item.set_text("Pause");
                        tracing::info!("Resumed from tray menu");
                    }
                }
                return Some(TrayCommand::TogglePause);
            } else if event.id == self.autostart_item_id {
                match autostart::toggle() {
                    Ok(enabled) => {
                        self.autostart_item.set_checked(enabled);
                        tracing::info!(
                            "Autostart {}",
                            if enabled { "enabled" } else { "disabled" }
                        );
                    }
                    Err(e) => {
                        tracing::error!("Failed to toggle autostart: {}", e);
                        // Revert checkbox to actual state
                        self.autostart_item.set_checked(autostart::is_enabled());
                    }
                }
                return Some(TrayCommand::ToggleAutostart);
            }
        }
        None
    }
}

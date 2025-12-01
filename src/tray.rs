use crossbeam_channel::Receiver;
use std::sync::{Arc, RwLock};
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    Icon, TrayIcon, TrayIconBuilder,
};

use crate::state::AppState;
use crate::utils::log;

pub enum TrayCommand {
    Quit,
    TogglePause,
}

pub struct Tray {
    tray_icon: TrayIcon,
    menu_receiver: Receiver<MenuEvent>,
    quit_item_id: tray_icon::menu::MenuId,
    pause_item_id: tray_icon::menu::MenuId,
    pause_item: MenuItem,
    status_item: MenuItem,
    is_paused: bool,
    last_status: String,
}

impl Tray {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let icon = Self::load_icon()?;

        // Status display (disabled, just for showing info)
        let status_item = MenuItem::new("Starting...", false, None);
        let pause_item = MenuItem::new("Pause", true, None);
        let quit_item = MenuItem::new("Quit Discrakt", true, None);

        let pause_item_id = pause_item.id().clone();
        let quit_item_id = quit_item.id().clone();

        let menu = Menu::new();
        menu.append(&status_item)?;
        menu.append(&PredefinedMenuItem::separator())?;
        menu.append(&pause_item)?;
        menu.append(&PredefinedMenuItem::separator())?;
        menu.append(&quit_item)?;

        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("Discrakt - Trakt to Discord")
            .with_icon(icon)
            .build()?;

        let menu_receiver = MenuEvent::receiver().clone();

        log("System tray initialized");

        Ok(Tray {
            tray_icon,
            menu_receiver,
            quit_item_id,
            pause_item_id,
            pause_item,
            status_item,
            is_paused: false,
            last_status: String::new(),
        })
    }

    fn load_icon() -> Result<Icon, Box<dyn std::error::Error>> {
        let icon_bytes = include_bytes!("assets/icon.png");
        let image = image::load_from_memory(icon_bytes)?;
        let rgba = image.to_rgba8();
        let (width, height) = rgba.dimensions();

        Icon::from_rgba(rgba.into_raw(), width, height).map_err(|e| e.into())
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

    pub fn poll_events(&mut self) -> Option<TrayCommand> {
        if let Ok(event) = self.menu_receiver.try_recv() {
            if event.id == self.quit_item_id {
                log("Quit requested from tray menu");
                return Some(TrayCommand::Quit);
            } else if event.id == self.pause_item_id {
                self.is_paused = !self.is_paused;
                if self.is_paused {
                    self.pause_item.set_text("Resume");
                    log("Paused from tray menu");
                } else {
                    self.pause_item.set_text("Pause");
                    log("Resumed from tray menu");
                }
                return Some(TrayCommand::TogglePause);
            }
        }
        None
    }

    pub fn is_paused(&self) -> bool {
        self.is_paused
    }
}

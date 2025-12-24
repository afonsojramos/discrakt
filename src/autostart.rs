#[cfg(target_os = "macos")]
const LAUNCHAGENT_LABEL: &str = "com.afonsojramos.discrakt";

#[cfg(target_os = "macos")]
mod macos {
    use super::LAUNCHAGENT_LABEL;
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;

    fn launch_agents_dir() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join("Library/LaunchAgents"))
    }

    fn plist_path() -> Option<PathBuf> {
        launch_agents_dir().map(|d| d.join(format!("{LAUNCHAGENT_LABEL}.plist")))
    }

    fn app_path() -> Option<PathBuf> {
        // Try to find the app bundle first
        if let Ok(exe) = std::env::current_exe() {
            // If running from an app bundle, exe is at Discrakt.app/Contents/MacOS/discrakt
            // We want to return the .app path
            if let Some(parent) = exe.parent() {
                if parent.ends_with("Contents/MacOS") {
                    if let Some(app_bundle) = parent.parent().and_then(|p| p.parent()) {
                        return Some(app_bundle.to_path_buf());
                    }
                }
            }
            // Not in app bundle, use the executable directly
            return Some(exe);
        }
        None
    }

    pub fn is_enabled() -> bool {
        plist_path().is_some_and(|p| p.exists())
    }

    pub fn enable() -> Result<(), String> {
        let plist_path = plist_path().ok_or("Could not determine LaunchAgents directory")?;
        let app_path = app_path().ok_or("Could not determine application path")?;

        // Ensure LaunchAgents directory exists
        if let Some(dir) = plist_path.parent() {
            fs::create_dir_all(dir)
                .map_err(|e| format!("Failed to create LaunchAgents dir: {e}"))?;
        }

        // Determine if we're launching an app bundle or binary
        let (program_path, program_args) = if app_path.extension().is_some_and(|e| e == "app") {
            // App bundle - use open command
            (
                "/usr/bin/open".to_string(),
                format!(
                    "<string>-a</string>\n      <string>{}</string>",
                    app_path.display()
                ),
            )
        } else {
            // Direct binary
            (app_path.display().to_string(), String::new())
        };

        let plist_content = if program_args.is_empty() {
            format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{LAUNCHAGENT_LABEL}</string>
    <key>ProgramArguments</key>
    <array>
      <string>{program_path}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <false/>
    <key>ProcessType</key>
    <string>Interactive</string>
</dict>
</plist>
"#
            )
        } else {
            format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{LAUNCHAGENT_LABEL}</string>
    <key>ProgramArguments</key>
    <array>
      <string>{program_path}</string>
      {program_args}
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <false/>
    <key>ProcessType</key>
    <string>Interactive</string>
</dict>
</plist>
"#
            )
        };

        fs::write(&plist_path, plist_content).map_err(|e| format!("Failed to write plist: {e}"))?;

        // Load the launch agent
        let plist_path_str = plist_path
            .to_str()
            .ok_or("Plist path contains invalid UTF-8")?;
        let _ = Command::new("launchctl")
            .args(["load", "-w", plist_path_str])
            .output();

        tracing::info!("Autostart enabled via LaunchAgent");
        Ok(())
    }

    pub fn disable() -> Result<(), String> {
        let plist_path = plist_path().ok_or("Could not determine LaunchAgents directory")?;

        if plist_path.exists() {
            // Unload the launch agent first
            if let Some(plist_path_str) = plist_path.to_str() {
                let _ = Command::new("launchctl")
                    .args(["unload", "-w", plist_path_str])
                    .output();
            }

            fs::remove_file(&plist_path).map_err(|e| format!("Failed to remove plist: {e}"))?;

            tracing::info!("Autostart disabled");
        }
        Ok(())
    }
}

#[cfg(target_os = "windows")]
mod windows {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
    use winreg::RegKey;

    const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const VALUE_NAME: &str = "Discrakt";

    fn exe_path() -> Option<String> {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.to_str().map(String::from))
    }

    pub fn is_enabled() -> bool {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let Ok(run_key) = hkcu.open_subkey_with_flags(RUN_KEY, KEY_READ) else {
            return false;
        };
        run_key.get_value::<String, _>(VALUE_NAME).is_ok()
    }

    pub fn enable() -> Result<(), String> {
        let exe = exe_path().ok_or("Could not determine executable path")?;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        // Use create_subkey to handle edge case where Run key doesn't exist
        let (run_key, _) = hkcu
            .create_subkey(RUN_KEY)
            .map_err(|e| format!("Failed to open registry key: {}", e))?;

        run_key
            .set_value(VALUE_NAME, &exe)
            .map_err(|e| format!("Failed to set registry value: {}", e))?;

        tracing::info!("Autostart enabled via Registry");
        Ok(())
    }

    pub fn disable() -> Result<(), String> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let Ok(run_key) = hkcu.open_subkey_with_flags(RUN_KEY, KEY_WRITE) else {
            // Key doesn't exist, so autostart is already disabled
            return Ok(());
        };

        // Ignore error if value doesn't exist (already disabled)
        let _ = run_key.delete_value(VALUE_NAME);

        tracing::info!("Autostart disabled");
        Ok(())
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use std::fs;
    use std::path::PathBuf;

    fn autostart_dir() -> Option<PathBuf> {
        dirs::config_dir().map(|c| c.join("autostart"))
    }

    fn desktop_file_path() -> Option<PathBuf> {
        autostart_dir().map(|d| d.join("discrakt.desktop"))
    }

    fn exe_path() -> Option<String> {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.to_str().map(String::from))
    }

    pub fn is_enabled() -> bool {
        desktop_file_path().map(|p| p.exists()).unwrap_or(false)
    }

    pub fn enable() -> Result<(), String> {
        let desktop_path = desktop_file_path().ok_or("Could not determine autostart directory")?;
        let exe = exe_path().ok_or("Could not determine executable path")?;

        // Ensure autostart directory exists
        if let Some(dir) = desktop_path.parent() {
            fs::create_dir_all(dir)
                .map_err(|e| format!("Failed to create autostart dir: {}", e))?;
        }

        let desktop_content = format!(
            r#"[Desktop Entry]
Type=Application
Name=Discrakt
Comment=Trakt to Discord Rich Presence
Exec={}
Icon=discrakt
Terminal=false
Categories=Utility;
X-GNOME-Autostart-enabled=true
"#,
            exe
        );

        fs::write(&desktop_path, desktop_content)
            .map_err(|e| format!("Failed to write desktop file: {}", e))?;

        tracing::info!("Autostart enabled via XDG autostart");
        Ok(())
    }

    pub fn disable() -> Result<(), String> {
        let desktop_path = desktop_file_path().ok_or("Could not determine autostart directory")?;

        if desktop_path.exists() {
            fs::remove_file(&desktop_path)
                .map_err(|e| format!("Failed to remove desktop file: {}", e))?;

            tracing::info!("Autostart disabled");
        }
        Ok(())
    }
}

// Re-export platform-specific functions
#[cfg(target_os = "macos")]
pub use macos::{disable, enable, is_enabled};

#[cfg(target_os = "windows")]
pub use windows::{disable, enable, is_enabled};

#[cfg(target_os = "linux")]
pub use linux::{disable, enable, is_enabled};

pub fn toggle() -> Result<bool, String> {
    if is_enabled() {
        disable()?;
        Ok(false)
    } else {
        enable()?;
        Ok(true)
    }
}

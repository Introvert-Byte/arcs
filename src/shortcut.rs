//! First-run desktop shortcut / launcher creation.
//! Creates a convenient way to relaunch Arcs from the desktop / app menu.

use crate::error::{ArcsError, Result};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// Returns true if this looks like the very first successful launch
/// (no marker file yet in the project data directory).
pub fn is_first_run() -> bool {
    marker_path().map(|p| !p.exists()).unwrap_or(false)
}

/// Mark that the first-run experience has been completed.
pub fn mark_first_run_done() -> Result<()> {
    if let Some(path) = marker_path() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, b"1")?;
    }
    Ok(())
}

fn marker_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("dev", "Arcs", "arcs")
        .map(|d| d.data_local_dir().join(".first_run_done"))
}

/// Create a desktop / application-menu shortcut for the current executable.
/// Returns the path of the created shortcut (or a human-readable description).
pub fn create_desktop_shortcut() -> Result<String> {
    let exe = env::current_exe().map_err(|e| ArcsError::Io(e))?;
    let exe = fs::canonicalize(&exe).unwrap_or(exe);

    cfg_if::cfg_if! {
        if #[cfg(target_os = "linux")] {
            create_linux_desktop_entry(&exe)
        } else if #[cfg(target_os = "windows")] {
            create_windows_shortcut(&exe)
        } else if #[cfg(target_os = "macos")] {
            create_macos_alias(&exe)
        } else {
            Err(ArcsError::Unsupported(
                "Automatic shortcut creation is not supported on this platform".into(),
            ))
        }
    }
}

#[cfg(target_os = "linux")]
fn create_linux_desktop_entry(exe: &Path) -> Result<String> {
    let home = dirs::home_dir().ok_or_else(|| ArcsError::Other("No home directory".into()))?;
    let apps_dir = home.join(".local/share/applications");
    fs::create_dir_all(&apps_dir)?;

    // Also try Desktop
    let desktop_dir = home.join("Desktop");
    let _ = fs::create_dir_all(&desktop_dir);

    let icon_path = install_icon_linux()?;

    let content = format!(
        r#"[Desktop Entry]
Type=Application
Name=Arcs
GenericName=Backup & Migration
Comment=Secure cross-platform backup, migration and application packaging
Exec="{}"
Icon={}
Terminal=false
Categories=Utility;Archiving;System;
Keywords=backup;archive;migration;restore;encrypt;
StartupNotify=true
"#,
        exe.display(),
        icon_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "application-x-executable".into())
    );

    let desktop_file = apps_dir.join("arcs.desktop");
    fs::write(&desktop_file, &content)?;

    // Make executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&desktop_file)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&desktop_file, perms)?;
    }

    // Copy to Desktop if it exists and is writable
    if desktop_dir.exists() {
        let desk_copy = desktop_dir.join("Arcs.desktop");
        let _ = fs::write(&desk_copy, &content);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = fs::metadata(&desk_copy) {
                let mut perms = meta.permissions();
                perms.set_mode(0o755);
                let _ = fs::set_permissions(&desk_copy, perms);
            }
        }
    }

    // Update desktop database (best-effort)
    let _ = std::process::Command::new("update-desktop-database")
        .arg(&apps_dir)
        .output();

    Ok(format!(
        "Desktop entry created: {}\nYou can also find “Arcs” in your application menu.",
        desktop_file.display()
    ))
}

#[cfg(target_os = "linux")]
fn install_icon_linux() -> Result<Option<PathBuf>> {
    let home = dirs::home_dir().ok_or_else(|| ArcsError::Other("No home".into()))?;
    let icon_dir = home.join(".local/share/icons/hicolor/256x256/apps");
    fs::create_dir_all(&icon_dir)?;
    let dest = icon_dir.join("arcs.png");

    // Prefer embedded / assets logo
    let candidates = [
        PathBuf::from("assets/logo.png"),
        PathBuf::from("assets/logo-128.png"),
        env::current_exe()
            .ok()
            .and_then(|e| e.parent().map(|p| p.join("assets/logo.png")))
            .unwrap_or_default(),
    ];

    for c in candidates {
        if c.exists() {
            fs::copy(&c, &dest)?;
            // Best-effort icon cache update
            let _ = std::process::Command::new("gtk-update-icon-cache")
                .args(["-f", "-t", &home.join(".local/share/icons/hicolor").display().to_string()])
                .output();
            return Ok(Some(dest));
        }
    }

    // Fallback: try to write the embedded bytes if we have them later
    Ok(None)
}

#[cfg(target_os = "windows")]
fn create_windows_shortcut(exe: &Path) -> Result<String> {
    // Create a simple .bat launcher on Desktop + Start Menu as a reliable fallback
    // (full .lnk creation needs COM / extra crates; bat is zero-dependency and works)
    let home = dirs::home_dir().ok_or_else(|| ArcsError::Other("No home".into()))?;
    let desktop = home.join("Desktop");
    let start_menu = home
        .join("AppData")
        .join("Roaming")
        .join("Microsoft")
        .join("Windows")
        .join("Start Menu")
        .join("Programs");

    fs::create_dir_all(&start_menu)?;
    let _ = fs::create_dir_all(&desktop);

    let bat_content = format!(
        "@echo off\r\nstart \"\" \"{}\"\r\n",
        exe.display()
    );

    let desktop_bat = desktop.join("Arcs.bat");
    fs::write(&desktop_bat, &bat_content)?;

    let start_bat = start_menu.join("Arcs.bat");
    fs::write(&start_bat, &bat_content)?;

    Ok(format!(
        "Launchers created:\n  • Desktop: {}\n  • Start Menu: {}\nDouble-click “Arcs” to run the app anytime.",
        desktop_bat.display(),
        start_bat.display()
    ))
}

#[cfg(target_os = "macos")]
fn create_macos_alias(exe: &Path) -> Result<String> {
    let home = dirs::home_dir().ok_or_else(|| ArcsError::Other("No home".into()))?;
    let apps = home.join("Applications");
    fs::create_dir_all(&apps)?;

    // Create a small wrapper .command script that users can put in Dock
    let script = apps.join("Arcs.command");
    let content = format!(
        "#!/bin/bash\ncd \"$(dirname \"$0\")\"\nexec \"{}\"\n",
        exe.display()
    );
    fs::write(&script, content)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&script)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script, perms)?;
    }

    // Also put on Desktop
    let desktop = home.join("Desktop").join("Arcs.command");
    let _ = fs::copy(&script, &desktop);

    Ok(format!(
        "Launcher created at {}\nYou can drag it to the Dock for quick access.",
        script.display()
    ))
}

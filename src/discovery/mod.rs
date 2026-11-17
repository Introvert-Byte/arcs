//! Cross-platform discovery of configs, packages and user data.

use crate::error::Result;
use crate::archive::format::PackageInfo;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredItem {
    pub id: String,
    pub name: String,
    pub kind: ItemKind,
    pub path: PathBuf,
    pub size_hint: Option<u64>,
    pub package: Option<PackageInfo>,
    pub selected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ItemKind {
    ConfigDir,
    AppData,
    FlatpakApp,
    SnapApp,
    AptPackage,
    HomebrewFormula,
    Custom,
}

pub trait Discoverer: Send + Sync {
    fn name(&self) -> &str;
    fn discover(&self) -> Result<Vec<DiscoveredItem>>;
}

pub fn platform_discoverers() -> Vec<Box<dyn Discoverer>> {
    let mut v: Vec<Box<dyn Discoverer>> = Vec::new();

    // Always try common config locations
    v.push(Box::new(CommonConfigDiscoverer));

    cfg_if::cfg_if! {
        if #[cfg(target_os = "linux")] {
            v.push(Box::new(linux::FlatpakDiscoverer));
            v.push(Box::new(linux::SnapDiscoverer));
            v.push(Box::new(linux::AptDiscoverer));
        } else if #[cfg(target_os = "macos")] {
            v.push(Box::new(macos::HomebrewDiscoverer));
            v.push(Box::new(macos::AppSupportDiscoverer));
        } else if #[cfg(target_os = "windows")] {
            v.push(Box::new(windows::AppDataDiscoverer));
        }
    }

    v
}

/// Common ~/.config and similar
pub struct CommonConfigDiscoverer;

impl Discoverer for CommonConfigDiscoverer {
    fn name(&self) -> &str {
        "user-config"
    }

    fn discover(&self) -> Result<Vec<DiscoveredItem>> {
        let mut items = Vec::new();
        if let Some(home) = dirs::home_dir() {
            let candidates = [
                home.join(".config"),
                home.join(".local/share"),
                home.join(".local/state"),
            ];
            for c in candidates {
                if c.is_dir() {
                    // list immediate children as selectable items
                    if let Ok(rd) = std::fs::read_dir(&c) {
                        for entry in rd.flatten() {
                            let p = entry.path();
                            let name = p
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("unknown")
                                .to_string();
                            items.push(DiscoveredItem {
                                id: format!("config:{}", p.display()),
                                name: name.clone(),
                                kind: ItemKind::ConfigDir,
                                path: p,
                                size_hint: None,
                                package: None,
                                selected: false,
                            });
                        }
                    }
                }
            }
        }
        Ok(items)
    }
}

#[cfg(target_os = "linux")]
pub mod linux {
    use super::*;
    use std::process::Command;

    pub struct FlatpakDiscoverer;
    impl Discoverer for FlatpakDiscoverer {
        fn name(&self) -> &str {
            "flatpak"
        }
        fn discover(&self) -> Result<Vec<DiscoveredItem>> {
            let mut items = Vec::new();
            let output = Command::new("flatpak")
                .args(["list", "--app", "--columns=application,name,version"])
                .output();
            if let Ok(out) = output {
                if out.status.success() {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    for line in stdout.lines().skip(0) {
                        let parts: Vec<&str> = line.split('\t').collect();
                        if parts.len() >= 2 {
                            let app_id = parts[0].trim().to_string();
                            let name = parts.get(1).unwrap_or(&app_id.as_str()).to_string();
                            let version = parts.get(2).map(|s| s.to_string());
                            // Typical user data location
                            let path = dirs::home_dir()
                                .map(|h| h.join(".var/app").join(&app_id))
                                .unwrap_or_default();
                            items.push(DiscoveredItem {
                                id: format!("flatpak:{}", app_id),
                                name,
                                kind: ItemKind::FlatpakApp,
                                path,
                                size_hint: None,
                                package: Some(PackageInfo {
                                    name: app_id.clone(),
                                    version,
                                    manager: "flatpak".into(),
                                    source_os: "linux".into(),
                                    install_hint: Some(format!("flatpak install {}", app_id)),
                                    files: vec![],
                                }),
                                selected: false,
                            });
                        }
                    }
                }
            }
            Ok(items)
        }
    }

    pub struct SnapDiscoverer;
    impl Discoverer for SnapDiscoverer {
        fn name(&self) -> &str {
            "snap"
        }
        fn discover(&self) -> Result<Vec<DiscoveredItem>> {
            let mut items = Vec::new();
            let output = Command::new("snap").args(["list"]).output();
            if let Ok(out) = output {
                if out.status.success() {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    for line in stdout.lines().skip(1) {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() >= 2 {
                            let name = parts[0].to_string();
                            let version = Some(parts[1].to_string());
                            let path = PathBuf::from("/var/lib/snapd/snaps"); // simplified
                            items.push(DiscoveredItem {
                                id: format!("snap:{}", name),
                                name: name.clone(),
                                kind: ItemKind::SnapApp,
                                path,
                                size_hint: None,
                                package: Some(PackageInfo {
                                    name: name.clone(),
                                    version,
                                    manager: "snap".into(),
                                    source_os: "linux".into(),
                                    install_hint: Some(format!("snap install {}", name)),
                                    files: vec![],
                                }),
                                selected: false,
                            });
                        }
                    }
                }
            }
            Ok(items)
        }
    }

    pub struct AptDiscoverer;
    impl Discoverer for AptDiscoverer {
        fn name(&self) -> &str {
            "apt"
        }
        fn discover(&self) -> Result<Vec<DiscoveredItem>> {
            // Only list manually installed packages (heuristic)
            let mut items = Vec::new();
            let output = Command::new("apt-mark")
                .args(["showmanual"])
                .output();
            if let Ok(out) = output {
                if out.status.success() {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    for line in stdout.lines().take(200) {
                        // limit for TUI
                        let name = line.trim().to_string();
                        if name.is_empty() {
                            continue;
                        }
                        items.push(DiscoveredItem {
                            id: format!("apt:{}", name),
                            name: name.clone(),
                            kind: ItemKind::AptPackage,
                            path: PathBuf::new(),
                            size_hint: None,
                            package: Some(PackageInfo {
                                name: name.clone(),
                                version: None,
                                manager: "apt".into(),
                                source_os: "linux".into(),
                                install_hint: Some(format!("sudo apt install {}", name)),
                                files: vec![],
                            }),
                            selected: false,
                        });
                    }
                }
            }
            Ok(items)
        }
    }
}

#[cfg(target_os = "macos")]
pub mod macos {
    use super::*;
    use std::process::Command;

    pub struct HomebrewDiscoverer;
    impl Discoverer for HomebrewDiscoverer {
        fn name(&self) -> &str {
            "homebrew"
        }
        fn discover(&self) -> Result<Vec<DiscoveredItem>> {
            let mut items = Vec::new();
            let output = Command::new("brew").args(["list", "--formula"]).output();
            if let Ok(out) = output {
                if out.status.success() {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    for line in stdout.lines() {
                        let name = line.trim().to_string();
                        if name.is_empty() {
                            continue;
                        }
                        items.push(DiscoveredItem {
                            id: format!("brew:{}", name),
                            name: name.clone(),
                            kind: ItemKind::HomebrewFormula,
                            path: PathBuf::new(),
                            size_hint: None,
                            package: Some(PackageInfo {
                                name: name.clone(),
                                version: None,
                                manager: "homebrew".into(),
                                source_os: "macos".into(),
                                install_hint: Some(format!("brew install {}", name)),
                                files: vec![],
                            }),
                            selected: false,
                        });
                    }
                }
            }
            Ok(items)
        }
    }

    pub struct AppSupportDiscoverer;
    impl Discoverer for AppSupportDiscoverer {
        fn name(&self) -> &str {
            "application-support"
        }
        fn discover(&self) -> Result<Vec<DiscoveredItem>> {
            let mut items = Vec::new();
            if let Some(home) = dirs::home_dir() {
                let support = home.join("Library/Application Support");
                if support.is_dir() {
                    if let Ok(rd) = std::fs::read_dir(&support) {
                        for entry in rd.flatten() {
                            let p = entry.path();
                            if p.is_dir() {
                                let name = p
                                    .file_name()
                                    .and_then(|n| n.to_str())
                                    .unwrap_or("unknown")
                                    .to_string();
                                items.push(DiscoveredItem {
                                    id: format!("appsupport:{}", name),
                                    name,
                                    kind: ItemKind::AppData,
                                    path: p,
                                    size_hint: None,
                                    package: None,
                                    selected: false,
                                });
                            }
                        }
                    }
                }
            }
            Ok(items)
        }
    }
}

#[cfg(target_os = "windows")]
pub mod windows {
    use super::*;

    pub struct AppDataDiscoverer;
    impl Discoverer for AppDataDiscoverer {
        fn name(&self) -> &str {
            "appdata"
        }
        fn discover(&self) -> Result<Vec<DiscoveredItem>> {
            let mut items = Vec::new();
            if let Some(roaming) = dirs::config_dir() {
                if let Ok(rd) = std::fs::read_dir(&roaming) {
                    for entry in rd.flatten() {
                        let p = entry.path();
                        if p.is_dir() {
                            let name = p
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("unknown")
                                .to_string();
                            items.push(DiscoveredItem {
                                id: format!("appdata:{}", name),
                                name,
                                kind: ItemKind::AppData,
                                path: p,
                                size_hint: None,
                                package: None,
                                selected: false,
                            });
                        }
                    }
                }
            }
            Ok(items)
        }
    }
}

/// Run all discoverers and merge results.
pub fn discover_all() -> Result<Vec<DiscoveredItem>> {
    let mut all = Vec::new();
    for d in platform_discoverers() {
        match d.discover() {
            Ok(items) => all.extend(items),
            Err(e) => {
                tracing::warn!("discoverer {} failed: {}", d.name(), e);
            }
        }
    }
    Ok(all)
}
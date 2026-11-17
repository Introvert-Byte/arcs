//! User style preferences + style addons (YAML/JSON packs).

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Tunable style values controlled from Settings or by addons.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StylePrefs {
    /// Base theme id: "light" | "dark" | "glass"
    #[serde(default = "default_theme")]
    pub theme: String,
    /// Corner radius 4–28
    #[serde(default = "default_radius")]
    pub radius: f32,
    /// Card transparency 0.0–1.0 (used mainly with glass)
    #[serde(default = "default_alpha")]
    pub glass_alpha: f32,
    /// Accent RGB 0–255
    #[serde(default = "default_accent")]
    pub accent: [u8; 3],
    /// Soft shadow under cards
    #[serde(default = "default_true")]
    pub shadows: bool,
    /// Larger spacing / padding
    #[serde(default = "default_true")]
    pub comfortable: bool,
    /// Active style addon id (filename stem), if any
    #[serde(default)]
    pub active_addon: Option<String>,
}

fn default_theme() -> String {
    "glass".into()
}
fn default_radius() -> f32 {
    18.0
}
fn default_alpha() -> f32 {
    0.12
}
fn default_accent() -> [u8; 3] {
    [56, 189, 248]
}
fn default_true() -> bool {
    true
}

impl Default for StylePrefs {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            radius: default_radius(),
            glass_alpha: default_alpha(),
            accent: default_accent(),
            shadows: true,
            comfortable: true,
            active_addon: None,
        }
    }
}

impl StylePrefs {
    pub fn config_path() -> PathBuf {
        directories::ProjectDirs::from("dev", "Arcs", "arcs")
            .map(|d| d.config_dir().join("style.json"))
            .unwrap_or_else(|| PathBuf::from("style.json"))
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if let Ok(s) = fs::read_to_string(&path) {
            if let Ok(p) = serde_json::from_str(&s) {
                return p;
            }
        }
        Self::default()
    }

    pub fn save(&self) {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let _ = fs::write(path, s);
        }
    }
}

/// A style addon pack (drop YAML/JSON into addons/).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StyleAddon {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub theme: Option<String>,
    #[serde(default)]
    pub radius: Option<f32>,
    #[serde(default)]
    pub glass_alpha: Option<f32>,
    #[serde(default)]
    pub accent: Option<[u8; 3]>,
    #[serde(default)]
    pub shadows: Option<bool>,
    #[serde(default)]
    pub comfortable: Option<bool>,
}

impl StyleAddon {
    pub fn apply_to(&self, prefs: &mut StylePrefs) {
        if let Some(ref t) = self.theme {
            prefs.theme = t.clone();
        }
        if let Some(r) = self.radius {
            prefs.radius = r.clamp(4.0, 28.0);
        }
        if let Some(a) = self.glass_alpha {
            prefs.glass_alpha = a.clamp(0.05, 0.6);
        }
        if let Some(c) = self.accent {
            prefs.accent = c;
        }
        if let Some(s) = self.shadows {
            prefs.shadows = s;
        }
        if let Some(c) = self.comfortable {
            prefs.comfortable = c;
        }
        prefs.active_addon = Some(self.id.clone());
    }
}

/// Folders searched for style addons.
pub fn addon_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    dirs.push(PathBuf::from("addons"));
    if let Some(pd) = directories::ProjectDirs::from("dev", "Arcs", "arcs") {
        dirs.push(pd.config_dir().join("addons"));
        dirs.push(pd.data_dir().join("addons"));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            dirs.push(parent.join("addons"));
        }
    }
    dirs
}

pub fn discover_addons() -> Vec<StyleAddon> {
    let mut out = Vec::new();
    for dir in addon_dirs() {
        let Ok(rd) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in rd.flatten() {
            let path = entry.path();
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            if ext != "yaml" && ext != "yml" && ext != "json" {
                continue;
            }
            if let Some(addon) = load_addon(&path) {
                out.push(addon);
            }
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn load_addon(path: &Path) -> Option<StyleAddon> {
    let content = fs::read_to_string(path).ok()?;
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("addon")
        .to_string();
    let mut addon: StyleAddon = if path.extension().and_then(|e| e.to_str()) == Some("json") {
        serde_json::from_str(&content).ok()?
    } else {
        serde_yaml::from_str(&content).ok()?
    };
    if addon.id.is_empty() {
        addon.id = stem;
    }
    if addon.name.is_empty() {
        addon.name = addon.id.clone();
    }
    Some(addon)
}

/// Built-in example addons written on first run if missing.
pub fn ensure_example_addons() {
    let dir = PathBuf::from("addons");
    let _ = fs::create_dir_all(&dir);
    let samples = [
        (
            "liquid-cyan.yaml",
            r#"id: liquid-cyan
name: Liquid Cyan
author: Arcs
description: Glass panels with cyan liquid accent
theme: glass
radius: 20
glass_alpha: 0.14
accent: [56, 189, 248]
shadows: true
comfortable: true
"#,
        ),
        (
            "neon-purple.yaml",
            r#"id: neon-purple
name: Neon Purple
author: Arcs
description: Dark glass with purple glow
theme: glass
radius: 16
glass_alpha: 0.18
accent: [168, 85, 247]
shadows: true
comfortable: true
"#,
        ),
        (
            "soft-studio.yaml",
            r#"id: soft-studio
name: Soft Studio
author: Arcs
description: Clean light studio look
theme: light
radius: 12
glass_alpha: 0.1
accent: [79, 70, 229]
shadows: false
comfortable: true
"#,
        ),
    ];
    for (name, body) in samples {
        let path = dir.join(name);
        if !path.exists() {
            let _ = fs::write(path, body);
        }
    }
}

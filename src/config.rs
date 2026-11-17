use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    /// Paths or globs to include
    #[serde(default)]
    pub include: Vec<String>,
    /// Paths or globs to exclude
    #[serde(default)]
    pub exclude: Vec<String>,
    /// Package managers / sources to discover
    #[serde(default)]
    pub sources: Vec<SourceKind>,
    /// Output archive path (can contain {timestamp}, {name})
    pub output: String,
    /// Optional password (prefer env / prompt in real use)
    #[serde(default)]
    pub password: Option<String>,
    /// Encrypt the archive
    #[serde(default = "default_true")]
    pub encrypt: bool,
    /// Dry-run only
    #[serde(default)]
    pub dry_run: bool,
    /// Cloud destination (optional)
    #[serde(default)]
    pub cloud: Option<CloudTarget>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SourceKind {
    Config,
    AppData,
    Flatpak,
    Snap,
    Apt,
    Homebrew,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudTarget {
    pub provider: CloudProvider,
    pub endpoint: Option<String>,
    pub bucket_or_path: String,
    pub access_key: Option<String>,
    pub secret_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CloudProvider {
    S3,
    WebDav,
    Local, // for testing
}

impl Profile {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let content = std::fs::read_to_string(path.as_ref())?;
        let profile: Profile = if path.as_ref().extension().and_then(|e| e.to_str()) == Some("json")
        {
            serde_json::from_str(&content)?
        } else {
            serde_yaml::from_str(&content)?
        };
        Ok(profile)
    }

    pub fn resolve_output(&self) -> PathBuf {
        let ts = chrono::Local::now().format("%Y%m%d-%H%M%S");
        let s = self
            .output
            .replace("{timestamp}", &ts.to_string())
            .replace("{name}", &self.name);
        PathBuf::from(s)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArcsConfig {
    pub default_profile: Option<String>,
    pub profiles_dir: PathBuf,
    pub cache_dir: PathBuf,
}

impl Default for ArcsConfig {
    fn default() -> Self {
        let base = directories::ProjectDirs::from("dev", "Arcs", "arcs")
            .map(|d| d.config_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        Self {
            default_profile: None,
            profiles_dir: base.join("profiles"),
            cache_dir: base.join("cache"),
        }
    }
}
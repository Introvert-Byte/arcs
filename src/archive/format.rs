//! Manifest and high-level description of an .arcs archive.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

pub const ARCS_MAGIC: &[u8] = b"ARCS\x01";
pub const MANIFEST_NAME: &str = "manifest.json";
pub const FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub format_version: u32,
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub source_os: String,
    pub source_arch: String,
    pub arcs_version: String,
    pub comment: Option<String>,
    pub encrypted: bool,
    pub entries: Vec<Entry>,
    pub packages: Vec<PackageInfo>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    /// Path inside the archive (posix-style)
    pub archive_path: String,
    /// Original absolute or home-relative path on source system
    pub original_path: String,
    pub size: u64,
    pub mode: Option<u32>,
    pub modified: Option<DateTime<Utc>>,
    pub sha256: Option<String>,
    pub kind: EntryKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EntryKind {
    File,
    Directory,
    Symlink,
    Config,
    PackageData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageInfo {
    pub name: String,
    pub version: Option<String>,
    pub manager: String, // apt, flatpak, snap, brew, scoop, winget, ...
    pub source_os: String,
    pub install_hint: Option<String>,
    /// Relative paths inside the archive that belong to this package
    pub files: Vec<String>,
}

impl Manifest {
    pub fn new(source_os: &str, source_arch: &str) -> Self {
        Self {
            format_version: FORMAT_VERSION,
            id: Uuid::new_v4(),
            created_at: Utc::now(),
            source_os: source_os.to_string(),
            source_arch: source_arch.to_string(),
            arcs_version: env!("CARGO_PKG_VERSION").to_string(),
            comment: None,
            encrypted: false,
            entries: Vec::new(),
            packages: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    pub fn total_size(&self) -> u64 {
        self.entries.iter().map(|e| e.size).sum()
    }

    pub fn file_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| e.kind == EntryKind::File || e.kind == EntryKind::Config)
            .count()
    }
}
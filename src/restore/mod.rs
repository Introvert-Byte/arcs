//! Restore logic with dry-run, compatibility checks and selective extraction.

use crate::archive::{ArchiveReader, EntryKind, Manifest};
use crate::error::Result;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct RestoreOptions {
    pub target_root: PathBuf,
    pub dry_run: bool,
    pub password: Option<String>,
    pub selected_paths: Option<Vec<String>>, // if None → restore everything
    pub overwrite: bool,
}

impl Default for RestoreOptions {
    fn default() -> Self {
        Self {
            target_root: dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")),
            dry_run: false,
            password: None,
            selected_paths: None,
            overwrite: false,
        }
    }
}

#[derive(Debug)]
pub struct RestorePlan {
    pub actions: Vec<RestoreAction>,
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub enum RestoreAction {
    CreateDir { path: PathBuf },
    WriteFile { archive_path: String, dest: PathBuf, size: u64 },
    Skip { reason: String },
}

pub fn plan_restore(reader: &ArchiveReader, opts: &RestoreOptions) -> Result<RestorePlan> {
    let manifest = reader.manifest(opts.password.as_deref())?;
    let mut actions = Vec::new();
    let mut warnings = Vec::new();

    // Compatibility check
    let current_os = std::env::consts::OS;
    if manifest.source_os != current_os {
        warnings.push(format!(
            "Archive was created on '{}' but current OS is '{}'. Some packages may not be installable.",
            manifest.source_os, current_os
        ));
    }

    for pkg in &manifest.packages {
        if pkg.source_os != current_os {
            warnings.push(format!(
                "Package '{}' ({}) originates from {} – install hint: {:?}",
                pkg.name, pkg.manager, pkg.source_os, pkg.install_hint
            ));
        }
    }

    for entry in &manifest.entries {
        if let Some(ref sel) = opts.selected_paths {
            if !sel.iter().any(|s| entry.archive_path.starts_with(s) || s == &entry.archive_path)
            {
                continue;
            }
        }

        // Map original path → target. Simple strategy: keep relative structure under target_root
        // or try to re-home common prefixes.
        let dest = map_destination(&entry.original_path, &opts.target_root, &manifest);

        match entry.kind {
            EntryKind::Directory => {
                actions.push(RestoreAction::CreateDir { path: dest });
            }
            EntryKind::File | EntryKind::Config | EntryKind::PackageData => {
                if dest.exists() && !opts.overwrite {
                    actions.push(RestoreAction::Skip {
                        reason: format!("{} already exists (use --overwrite)", dest.display()),
                    });
                } else {
                    actions.push(RestoreAction::WriteFile {
                        archive_path: entry.archive_path.clone(),
                        dest,
                        size: entry.size,
                    });
                }
            }
            EntryKind::Symlink => {
                // simplified – skip for now
                actions.push(RestoreAction::Skip {
                    reason: "symlink restore not yet implemented".into(),
                });
            }
        }
    }

    Ok(RestorePlan { actions, warnings })
}

fn map_destination(original: &str, target_root: &Path, _manifest: &Manifest) -> PathBuf {
    // Very simple mapping: if original starts with a known home, strip it.
    // Production code would use more sophisticated remapping tables.
    let orig = Path::new(original);
    if let Some(home) = dirs::home_dir() {
        if let Ok(rel) = orig.strip_prefix(&home) {
            return target_root.join(rel);
        }
    }
    // Fallback: put under target_root / "restored" / basename
    target_root
        .join("restored")
        .join(orig.file_name().unwrap_or_default())
}

pub fn execute_restore(
    reader: &ArchiveReader,
    plan: &RestorePlan,
    opts: &RestoreOptions,
) -> Result<()> {
    if opts.dry_run {
        tracing::info!("Dry-run – no changes will be made");
        for a in &plan.actions {
            match a {
                RestoreAction::CreateDir { path } => {
                    println!("  [dry-run] mkdir {}", path.display());
                }
                RestoreAction::WriteFile { dest, size, .. } => {
                    println!("  [dry-run] write {} ({} bytes)", dest.display(), size);
                }
                RestoreAction::Skip { reason } => {
                    println!("  [dry-run] skip: {}", reason);
                }
            }
        }
        return Ok(());
    }

    for a in &plan.actions {
        match a {
            RestoreAction::CreateDir { path } => {
                fs::create_dir_all(path)?;
            }
            RestoreAction::WriteFile {
                archive_path,
                dest,
                ..
            } => {
                if let Some(parent) = dest.parent() {
                    fs::create_dir_all(parent)?;
                }
                let data = reader.extract_file(archive_path, opts.password.as_deref())?;
                fs::write(dest, data)?;
            }
            RestoreAction::Skip { .. } => {}
        }
    }
    Ok(())
}
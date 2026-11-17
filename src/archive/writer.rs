use crate::archive::format::{Entry, EntryKind, Manifest, MANIFEST_NAME};
use crate::crypto;
use crate::error::{ArcsError, Result};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use zip::write::FileOptions;
use zip::CompressionMethod;
use zip::ZipWriter;

pub struct ArchiveWriter {
    manifest: Manifest,
    files: Vec<(PathBuf, String)>, // (local_path, archive_path)
    password: Option<String>,
}

impl ArchiveWriter {
    pub fn new(source_os: &str, source_arch: &str) -> Self {
        Self {
            manifest: Manifest::new(source_os, source_arch),
            files: Vec::new(),
            password: None,
        }
    }

    pub fn set_password(&mut self, password: impl Into<String>) {
        self.password = Some(password.into());
        self.manifest.encrypted = true;
    }

    pub fn set_comment(&mut self, comment: impl Into<String>) {
        self.manifest.comment = Some(comment.into());
    }

    /// Add a single file or directory (recursively).
    pub fn add_path(&mut self, local: impl AsRef<Path>, archive_prefix: &str) -> Result<()> {
        let local = local.as_ref();
        if !local.exists() {
            return Err(ArcsError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("path not found: {}", local.display()),
            )));
        }

        if local.is_file() {
            let name = local
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("file");
            let ap = if archive_prefix.is_empty() {
                name.to_string()
            } else {
                format!("{}/{}", archive_prefix.trim_end_matches('/'), name)
            };
            self.files.push((local.to_path_buf(), ap.clone()));
            self.manifest.entries.push(Entry {
                archive_path: ap,
                original_path: local.display().to_string(),
                size: local.metadata().map(|m| m.len()).unwrap_or(0),
                mode: None,
                modified: None,
                sha256: None,
                kind: EntryKind::File,
            });
        } else {
            for entry in WalkDir::new(local).into_iter().filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.is_dir() {
                    continue;
                }
                let rel = path
                    .strip_prefix(local)
                    .unwrap_or(path)
                    .to_string_lossy()
                    .replace('\\', "/");
                let ap = if archive_prefix.is_empty() {
                    rel
                } else {
                    format!("{}/{}", archive_prefix.trim_end_matches('/'), rel)
                };
                let size = path.metadata().map(|m| m.len()).unwrap_or(0);
                self.files.push((path.to_path_buf(), ap.clone()));
                self.manifest.entries.push(Entry {
                    archive_path: ap,
                    original_path: path.display().to_string(),
                    size,
                    mode: None,
                    modified: None,
                    sha256: None,
                    kind: EntryKind::File,
                });
            }
        }
        Ok(())
    }

    pub fn add_package(&mut self, info: crate::archive::format::PackageInfo) {
        self.manifest.packages.push(info);
    }

    /// Write the final .arcs file.
    /// Internally we build a ZIP, optionally encrypt the whole ZIP blob.
    pub fn finish(mut self, output: impl AsRef<Path>) -> Result<()> {
        let output = output.as_ref();
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent)?;
        }

        // Build ZIP in memory (or temp file for large archives – simplified here)
        let mut zip_buf: Vec<u8> = Vec::new();
        {
            let mut zip = ZipWriter::new(std::io::Cursor::new(&mut zip_buf));
            let options = FileOptions::default()
                .compression_method(CompressionMethod::Deflated)
                .unix_permissions(0o644);

            // Write files and compute hashes
            for (local, archive_path) in &self.files {
                let mut file = File::open(local)?;
                let mut content = Vec::new();
                file.read_to_end(&mut content)?;

                let mut hasher = Sha256::new();
                hasher.update(&content);
                let hash = hex::encode(hasher.finalize());

                if let Some(entry) = self
                    .manifest
                    .entries
                    .iter_mut()
                    .find(|e| e.archive_path == *archive_path)
                {
                    entry.sha256 = Some(hash);
                    entry.size = content.len() as u64;
                }

                zip.start_file(archive_path, options)?;
                zip.write_all(&content)?;
            }

            // Write manifest last
            let manifest_json = serde_json::to_vec_pretty(&self.manifest)?;
            zip.start_file(MANIFEST_NAME, options)?;
            zip.write_all(&manifest_json)?;
            zip.finish()?;
        }

        // Optionally encrypt the whole ZIP
        let final_bytes = if let Some(ref pw) = self.password {
            crypto::encrypt(&zip_buf, pw)?
        } else {
            // Prepend a simple magic for unencrypted
            let mut out = crate::archive::format::ARCS_MAGIC.to_vec();
            out.extend_from_slice(&zip_buf);
            out
        };

        let mut f = File::create(output)?;
        f.write_all(&final_bytes)?;
        Ok(())
    }
}
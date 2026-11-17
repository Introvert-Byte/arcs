use crate::archive::format::{Manifest, MANIFEST_NAME, ARCS_MAGIC};
use crate::crypto;
use crate::error::{ArcsError, Result};
use std::fs::File;
use std::io::{Cursor, Read};
use std::path::Path;
use zip::ZipArchive;

pub struct ArchiveReader {
    data: Vec<u8>,
    encrypted: bool,
}

impl ArchiveReader {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let mut f = File::open(path)?;
        let mut data = Vec::new();
        f.read_to_end(&mut data)?;
        let encrypted = !data.starts_with(ARCS_MAGIC);
        Ok(Self { data, encrypted })
    }

    pub fn is_encrypted(&self) -> bool {
        self.encrypted
    }

    /// Decrypt if needed and return the inner ZIP bytes.
    fn inner_zip(&self, password: Option<&str>) -> Result<Vec<u8>> {
        if self.encrypted {
            let pw = password.ok_or_else(|| {
                ArcsError::Crypto("archive is encrypted – password required".into())
            })?;
            crypto::decrypt(&self.data, pw)
        } else {
            // skip magic
            if self.data.len() < ARCS_MAGIC.len() {
                return Err(ArcsError::Archive("invalid magic".into()));
            }
            Ok(self.data[ARCS_MAGIC.len()..].to_vec())
        }
    }

    pub fn manifest(&self, password: Option<&str>) -> Result<Manifest> {
        let zip_bytes = self.inner_zip(password)?;
        let cursor = Cursor::new(zip_bytes);
        let mut archive = ZipArchive::new(cursor)?;
        let mut manifest_file = archive
            .by_name(MANIFEST_NAME)
            .map_err(|_| ArcsError::Archive("manifest.json not found".into()))?;
        let mut s = String::new();
        manifest_file.read_to_string(&mut s)?;
        let manifest: Manifest = serde_json::from_str(&s)?;
        Ok(manifest)
    }

    /// Extract a single file by archive path into memory.
    pub fn extract_file(&self, archive_path: &str, password: Option<&str>) -> Result<Vec<u8>> {
        let zip_bytes = self.inner_zip(password)?;
        let cursor = Cursor::new(zip_bytes);
        let mut archive = ZipArchive::new(cursor)?;
        let mut file = archive
            .by_name(archive_path)
            .map_err(|_| ArcsError::Archive(format!("file not found: {archive_path}")))?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)?;
        Ok(buf)
    }

    /// List all entries (from manifest).
    pub fn list(&self, password: Option<&str>) -> Result<Manifest> {
        self.manifest(password)
    }
}
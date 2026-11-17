//! Minimal cloud push/pull for S3-compatible and WebDAV.
//! Real credentials should come from env or secure store.

use crate::error::{ArcsError, Result};
use crate::config::{CloudProvider, CloudTarget};
use reqwest::Client;
use std::path::Path;
use tokio::fs;

pub struct CloudClient {
    http: Client,
}

impl CloudClient {
    pub fn new() -> Self {
        Self {
            http: Client::builder()
                .user_agent(concat!("arcs/", env!("CARGO_PKG_VERSION")))
                .build()
                .expect("http client"),
        }
    }

    pub async fn push(&self, local: impl AsRef<Path>, target: &CloudTarget) -> Result<()> {
        let local = local.as_ref();
        let data = fs::read(local)
            .await
            .map_err(|e| ArcsError::Cloud(e.to_string()))?;
        let remote_name = local
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("backup.arcs");

        match target.provider {
            CloudProvider::S3 => self.push_s3(&data, remote_name, target).await,
            CloudProvider::WebDav => self.push_webdav(&data, remote_name, target).await,
            CloudProvider::Local => {
                let dest = Path::new(&target.bucket_or_path).join(remote_name);
                if let Some(p) = dest.parent() {
                    fs::create_dir_all(p).await.ok();
                }
                fs::write(&dest, &data)
                    .await
                    .map_err(|e| ArcsError::Cloud(e.to_string()))?;
                tracing::info!("Pushed to local path {}", dest.display());
                Ok(())
            }
        }
    }

    pub async fn pull(&self, remote_name: &str, local: impl AsRef<Path>, target: &CloudTarget) -> Result<()> {
        let local = local.as_ref();
        let data = match target.provider {
            CloudProvider::S3 => self.pull_s3(remote_name, target).await?,
            CloudProvider::WebDav => self.pull_webdav(remote_name, target).await?,
            CloudProvider::Local => {
                let src = Path::new(&target.bucket_or_path).join(remote_name);
                fs::read(&src)
                    .await
                    .map_err(|e| ArcsError::Cloud(e.to_string()))?
            }
        };
        if let Some(p) = local.parent() {
            fs::create_dir_all(p).await.ok();
        }
        fs::write(local, data)
            .await
            .map_err(|e| ArcsError::Cloud(e.to_string()))?;
        Ok(())
    }

    async fn push_s3(&self, data: &[u8], name: &str, target: &CloudTarget) -> Result<()> {
        // Minimal S3 PutObject using path-style or virtual-hosted.
        // For production use aws-sdk-s3 or a proper signer.
        let endpoint = target
            .endpoint
            .as_deref()
            .unwrap_or("https://s3.amazonaws.com");
        let url = format!(
            "{}/{}/{}",
            endpoint.trim_end_matches('/'),
            target.bucket_or_path,
            name
        );
        let mut req = self.http.put(&url).body(data.to_vec());
        if let (Some(ak), Some(sk)) = (&target.access_key, &target.secret_key) {
            // Placeholder – real implementation needs AWS Signature V4
            req = req.header("x-amz-meta-arcs", "true");
            let _ = (ak, sk); // silence
            tracing::warn!("S3 signing not fully implemented – using unsigned request (local/dev only)");
        }
        let resp = req.send().await.map_err(|e| ArcsError::Cloud(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(ArcsError::Cloud(format!(
                "S3 upload failed: {}",
                resp.status()
            )));
        }
        Ok(())
    }

    async fn pull_s3(&self, name: &str, target: &CloudTarget) -> Result<Vec<u8>> {
        let endpoint = target
            .endpoint
            .as_deref()
            .unwrap_or("https://s3.amazonaws.com");
        let url = format!(
            "{}/{}/{}",
            endpoint.trim_end_matches('/'),
            target.bucket_or_path,
            name
        );
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| ArcsError::Cloud(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(ArcsError::Cloud(format!(
                "S3 download failed: {}",
                resp.status()
            )));
        }
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| ArcsError::Cloud(e.to_string()))?;
        Ok(bytes.to_vec())
    }

    async fn push_webdav(&self, data: &[u8], name: &str, target: &CloudTarget) -> Result<()> {
        let base = target
            .endpoint
            .as_ref()
            .ok_or_else(|| ArcsError::Cloud("WebDAV endpoint required".into()))?;
        let url = format!(
            "{}/{}",
            base.trim_end_matches('/'),
            name
        );
        let mut req = self.http.put(&url).body(data.to_vec());
        if let (Some(user), Some(pass)) = (&target.access_key, &target.secret_key) {
            req = req.basic_auth(user, Some(pass));
        }
        let resp = req.send().await.map_err(|e| ArcsError::Cloud(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(ArcsError::Cloud(format!(
                "WebDAV upload failed: {}",
                resp.status()
            )));
        }
        Ok(())
    }

    async fn pull_webdav(&self, name: &str, target: &CloudTarget) -> Result<Vec<u8>> {
        let base = target
            .endpoint
            .as_ref()
            .ok_or_else(|| ArcsError::Cloud("WebDAV endpoint required".into()))?;
        let url = format!("{}/{}", base.trim_end_matches('/'), name);
        let mut req = self.http.get(&url);
        if let (Some(user), Some(pass)) = (&target.access_key, &target.secret_key) {
            req = req.basic_auth(user, Some(pass));
        }
        let resp = req.send().await.map_err(|e| ArcsError::Cloud(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(ArcsError::Cloud(format!(
                "WebDAV download failed: {}",
                resp.status()
            )));
        }
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| ArcsError::Cloud(e.to_string()))?;
        Ok(bytes.to_vec())
    }
}

impl Default for CloudClient {
    fn default() -> Self {
        Self::new()
    }
}
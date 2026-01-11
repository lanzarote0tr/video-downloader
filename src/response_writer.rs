use crate::recorder::{BodyData, RequestData};
use anyhow::Result;
use base64::Engine;
use std::{
    io::ErrorKind,
    path::{Path, PathBuf},
};
use tokio::{fs::OpenOptions, io::AsyncWriteExt};

pub struct ResponseWriter {
    base_dir: PathBuf,
}

impl ResponseWriter {
    pub async fn new(base_dir: &Path) -> Result<Self> {
        tokio::fs::create_dir_all(base_dir).await?;
        Ok(Self {
            base_dir: base_dir.to_path_buf(),
        })
    }

    pub async fn write(&mut self, request: Option<&RequestData>, body: &BodyData) -> Result<()> {
        let Some(filename) = Self::pick_filename(request) else {
            return Ok(());
        };
        let mut file = self.create_unique_file(&filename).await?;
        let bytes = if body.base64_encoded {
            base64::engine::general_purpose::STANDARD
                .decode(&body.text)
                .unwrap_or_else(|_| body.text.as_bytes().to_vec())
        } else {
            body.text.as_bytes().to_vec()
        };
        file.write_all(&bytes).await?;
        file.flush().await?;
        Ok(())
    }

    fn pick_filename(request: Option<&RequestData>) -> Option<String> {
        let request = request?;
        let trimmed = request.url.split('#').next().unwrap_or(&request.url);
        let path = trimmed.split('?').next().unwrap_or(trimmed);
        let raw_name = path.rsplit('/').next().unwrap_or("");
        if raw_name.is_empty() {
            return None;
        }
        let cleaned = sanitize(raw_name);
        let lower = cleaned.to_ascii_lowercase();
        if lower.ends_with(".mp4") || lower.ends_with(".m4s") {
            Some(cleaned)
        } else {
            None
        }
    }

    async fn create_unique_file(&self, filename: &str) -> Result<tokio::fs::File> {
        let (stem, ext) = split_name(filename);
        let mut attempt = 0;
        loop {
            let candidate = if attempt == 0 {
                self.base_dir.join(filename)
            } else {
                self.base_dir.join(format!("{}-{}{}", stem, attempt, ext))
            };
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&candidate)
                .await
            {
                Ok(file) => return Ok(file),
                Err(e) if e.kind() == ErrorKind::AlreadyExists => {
                    attempt += 1;
                }
                Err(e) => return Err(e.into()),
            }
        }
    }
}

fn split_name(name: &str) -> (String, String) {
    match name.rfind('.') {
        Some(idx) => (name[..idx].to_string(), name[idx..].to_string()),
        None => (name.to_string(), String::new()),
    }
}

fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect()
}

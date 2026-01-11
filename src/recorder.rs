use crate::response_writer::ResponseWriter;
use anyhow::Result;
use serde::Serialize;
use serde_json::Value;
use std::{collections::HashMap, path::Path, sync::Arc};
use tokio::{fs::OpenOptions, io::AsyncWriteExt, sync::Mutex};

#[derive(Clone, Serialize, Default)]
struct Entry {
    request_id: String,
    request: Option<RequestData>,
    response: Option<ResponseData>,
    body: Option<BodyData>,
    error: Option<String>,
}

#[derive(Clone, Serialize)]
pub struct RequestData {
    pub url: String,
    pub method: String,
    pub headers: Value,
    pub body: Option<String>,
    pub timestamp: Option<f64>,
}

#[derive(Clone, Serialize)]
pub struct ResponseData {
    pub status: i64,
    pub status_text: String,
    pub headers: Value,
    pub mime_type: Option<String>,
}

#[derive(Clone, Serialize)]
pub struct BodyData {
    pub text: String,
    pub base64_encoded: bool,
}

struct State {
    file: tokio::fs::File,
    entries: HashMap<String, Entry>,
    writer: ResponseWriter,
}

#[derive(Clone)]
pub struct Recorder {
    inner: Arc<Mutex<State>>,
}

impl Recorder {
    pub async fn new(path: &Path) -> Result<Self> {
        let writer = ResponseWriter::new(path.parent().unwrap_or(Path::new(""))).await?;
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .await?;
        let state = State {
            file,
            entries: HashMap::new(),
            writer,
        };
        Ok(Self {
            inner: Arc::new(Mutex::new(state)),
        })
    }

    pub async fn start_request(&self, request_id: &str, data: RequestData) -> Result<()> {
        let mut inner = self.inner.lock().await;
        let entry = inner
            .entries
            .entry(request_id.to_string())
            .or_insert_with(|| Entry {
                request_id: request_id.to_string(),
                ..Entry::default()
            });
        entry.request = Some(data);
        Ok(())
    }

    pub async fn set_response(&self, request_id: &str, data: ResponseData) -> Result<()> {
        let mut inner = self.inner.lock().await;
        let entry = inner
            .entries
            .entry(request_id.to_string())
            .or_insert_with(|| Entry {
                request_id: request_id.to_string(),
                ..Entry::default()
            });
        entry.response = Some(data);
        Ok(())
    }

    pub async fn set_body(&self, request_id: &str, data: BodyData) -> Result<()> {
        {
            let mut inner = self.inner.lock().await;
            if let Some(entry) = inner.entries.get_mut(request_id) {
                entry.body = Some(data);
            } else {
                inner.entries.insert(
                    request_id.to_string(),
                    Entry {
                        request_id: request_id.to_string(),
                        body: Some(data),
                        ..Entry::default()
                    },
                );
            }
        }
        self.flush_entry(request_id).await
    }

    pub async fn fail(&self, request_id: &str, error: &str) -> Result<()> {
        {
            let mut inner = self.inner.lock().await;
            let entry = inner
                .entries
                .entry(request_id.to_string())
                .or_insert_with(|| Entry {
                    request_id: request_id.to_string(),
                    ..Entry::default()
                });
            entry.error = Some(error.to_string());
        }
        self.flush_entry(request_id).await
    }

    async fn flush_entry(&self, request_id: &str) -> Result<()> {
        let mut inner = self.inner.lock().await;
        if let Some(entry) = inner.entries.remove(request_id) {
            if let Some(body) = &entry.body {
                inner.writer.write(entry.request.as_ref(), body).await?;
            }
            let line = serde_json::to_vec(&entry)?;
            inner.file.write_all(&line).await?;
            inner.file.write_all(b"\n").await?;
            inner.file.flush().await?;
        }
        Ok(())
    }
}

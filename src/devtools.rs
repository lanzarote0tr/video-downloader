use crate::{
    devtools_handler::handle_event,
    recorder::{BodyData, Recorder},
};
use anyhow::{Context, Result};
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{collections::HashMap, time::Duration};
use tokio::time::sleep;
use tokio_tungstenite::{connect_async, tungstenite::Message};

#[derive(Deserialize)]
struct TargetInfo {
    #[serde(rename = "webSocketDebuggerUrl")]
    ws_url: String,
    #[serde(rename = "type")]
    kind: String,
}

pub async fn capture(port: u16, recorder: Recorder) -> Result<()> {
    let ws_url = wait_for_page_ws(port).await?;
    let (mut ws, _) = connect_async(&ws_url)
        .await
        .with_context(|| format!("connect to devtools ws at {}", ws_url))?;
    let enable = json!({"id":1,"method":"Network.enable","params":{"maxResourceBufferSize":0,"maxTotalBufferSize":0}});
    ws.send(Message::Text(enable.to_string()))
        .await
        .context("enable network domain")?;
    let mut next_id: u64 = 2;
    let mut pending_bodies: HashMap<u64, String> = HashMap::new();
    while let Some(msg) = ws.next().await {
        let msg = match msg {
            Ok(Message::Text(t)) => t,
            Ok(Message::Close(_)) => break,
            Ok(_) => continue,
            Err(e) => return Err(e.into()),
        };
        let payload: Value = serde_json::from_str(&msg)?;
        if let Some(method) = payload.get("method").and_then(|m| m.as_str()) {
            handle_event(
                method,
                payload.get("params").unwrap_or(&Value::Null),
                &recorder,
                &mut ws,
                &mut next_id,
                &mut pending_bodies,
            )
            .await?;
        } else if let Some(id) = payload.get("id").and_then(|v| v.as_u64()) {
            if let (Some(req_id), Some(result)) =
                (pending_bodies.remove(&id), payload.get("result"))
            {
                let text = result
                    .get("body")
                    .and_then(|b| b.as_str())
                    .unwrap_or("")
                    .to_string();
                let base64 = result
                    .get("base64Encoded")
                    .and_then(|b| b.as_bool())
                    .unwrap_or(false);
                recorder
                    .set_body(
                        &req_id,
                        BodyData {
                            text,
                            base64_encoded: base64,
                        },
                    )
                    .await?;
            }
        }
    }
    Ok(())
}

async fn wait_for_page_ws(port: u16) -> Result<String> {
    let client = reqwest::Client::builder().build()?;
    let endpoint = format!("http://127.0.0.1:{}/json/list", port);
    for _ in 0..120 {
        if let Ok(resp) = client.get(&endpoint).send().await {
            if let Ok(body) = resp.error_for_status() {
                if let Ok(targets) = body.json::<Vec<TargetInfo>>().await {
                    if let Some(page) = targets
                        .into_iter()
                        .find(|t| t.kind == "page" && !t.ws_url.is_empty())
                    {
                        return Ok(page.ws_url);
                    }
                }
            }
        }
        sleep(Duration::from_millis(250)).await;
    }
    Err(anyhow::anyhow!(
        "page devtools endpoint not available at {}",
        endpoint
    ))
}

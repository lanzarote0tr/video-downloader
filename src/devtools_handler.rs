use crate::recorder::{Recorder, RequestData, ResponseData};
use anyhow::{Context, Result};
use futures::SinkExt;
use serde_json::{json, Value};
use std::collections::HashMap;
use tokio::net::TcpStream;
use tokio_tungstenite::{tungstenite::Message, MaybeTlsStream, WebSocketStream};

pub type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;

fn field(v: &Value, key: &str) -> String {
    v.get(key).and_then(|v| v.as_str()).unwrap_or("").to_string()
}

fn opt_field(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(|v| v.as_str()).map(|s| s.to_string())
}

pub async fn handle_event(
    method: &str,
    params: &Value,
    recorder: &Recorder,
    ws: &mut WsStream,
    next_id: &mut u64,
    pending_bodies: &mut HashMap<u64, String>,
) -> Result<()> {
    match method {
        "Network.requestWillBeSent" => {
            if let Some(request_id) = params.get("requestId").and_then(|v| v.as_str()) {
                let req = params.get("request").unwrap_or(&Value::Null);
                let data = RequestData {
                    url: field(req, "url"),
                    method: field(req, "method"),
                    headers: req.get("headers").cloned().unwrap_or(Value::Null),
                    body: opt_field(req, "postData"),
                    timestamp: params.get("wallTime").and_then(|v| v.as_f64()),
                };
                recorder.start_request(request_id, data).await?;
            }
        }
        "Network.responseReceived" => {
            if let Some(request_id) = params.get("requestId").and_then(|v| v.as_str()) {
                let resp = params.get("response").unwrap_or(&Value::Null);
                let data = ResponseData {
                    status: resp.get("status").and_then(|v| v.as_i64()).unwrap_or(0),
                    status_text: field(resp, "statusText"),
                    headers: resp.get("headers").cloned().unwrap_or(Value::Null),
                    mime_type: opt_field(resp, "mimeType"),
                };
                recorder.set_response(request_id, data).await?;
            }
        }
        "Network.loadingFinished" => {
            if let Some(request_id) = params.get("requestId").and_then(|v| v.as_str()) {
                let call_id = *next_id;
                *next_id += 1;
                pending_bodies.insert(call_id, request_id.to_string());
                let message = json!({
                    "id": call_id,
                    "method": "Network.getResponseBody",
                    "params": { "requestId": request_id }
                });
                ws.send(Message::Text(message.to_string()))
                    .await
                    .context("request response body")?;
            }
        }
        "Network.loadingFailed" => {
            if let Some(request_id) = params.get("requestId").and_then(|v| v.as_str()) {
                if let Some(error) = params.get("errorText").and_then(|v| v.as_str()) {
                    recorder.fail(request_id, error).await?;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

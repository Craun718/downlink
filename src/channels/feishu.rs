use async_trait::async_trait;
use base64::Engine;
use hmac::{Hmac, Mac};
use serde_json::{json, Value};
use sha2::Sha256;

use crate::capability::Capabilities;
use crate::channel::{Channel, NotifyError, SendReceipt};
use crate::http::{HttpClient, HttpRequest, HttpMethod};
use crate::message::{Message, MessageBody};

pub struct FeishuChannel {
    webhook_url: String,
    secret: Option<String>,
    http: Box<dyn HttpClient>,
}

impl FeishuChannel {
    pub fn new(webhook_url: String, secret: Option<String>, http: Box<dyn HttpClient>) -> Self {
        Self { webhook_url, secret, http }
    }

    fn build_payload(&self, message: &Message, timestamp: &str) -> Value {
        let mut payload = match &message.body {
            MessageBody::Text { text } => json!({
                "msg_type": "text",
                "content": { "text": text }
            }),
            MessageBody::Markdown { text, .. } => json!({
                "msg_type": "text",
                "content": { "text": text }
            }),
            MessageBody::Card { title, markdown } => json!({
                "msg_type": "interactive",
                "card": {
                    "header": { "title": { "tag": "plain_text", "content": title } },
                    "elements": [{ "tag": "markdown", "content": markdown }]
                }
            }),
        };
        if let Some(secret) = &self.secret {
            let string_to_sign = format!("{timestamp}\n{secret}");
            let mut mac = Hmac::<Sha256>::new_from_slice(string_to_sign.as_bytes())
                .expect("HMAC can take key of any size");
            mac.update(b"");
            let sign = base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes());
            payload["timestamp"] = json!(timestamp);
            payload["sign"] = json!(sign);
        }
        payload
    }
}

#[async_trait]
impl Channel for FeishuChannel {
    fn name(&self) -> &'static str {
        "feishu"
    }

    fn capabilities(&self) -> &Capabilities {
        &Self::CAPABILITIES
    }

    async fn send(&self, message: &Message) -> Result<SendReceipt, NotifyError> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            .to_string();
        let payload = self.build_payload(message, &timestamp);
        let request = HttpRequest {
            method: HttpMethod::Post,
            url: self.webhook_url.clone(),
            headers: vec![("Content-Type".into(), "application/json".into())],
            body: Some(payload),
        };
        let response = self.http.execute(request).await?;
        let body = response.json()?;
        let code = body
            .get("code")
            .or_else(|| body.get("StatusCode"))
            .and_then(Value::as_i64);
        if code.is_some_and(|c| c != 0) {
            let msg = body
                .get("msg")
                .or_else(|| body.get("StatusMessage"))
                .and_then(Value::as_str)
                .unwrap_or("unknown error");
            return Err(NotifyError::Channel(msg.to_string()));
        }
        Ok(SendReceipt {
            channel: Self::CHANNEL_NAME.into(),
            message_id: None,
            raw_response: Some(response.body),
        })
    }
}

impl FeishuChannel {
    const CHANNEL_NAME: &'static str = "feishu";
    const CAPABILITIES: Capabilities = Capabilities::new().with_card();
}

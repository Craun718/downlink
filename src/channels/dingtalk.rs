use async_trait::async_trait;
use base64::Engine;
use hmac::{Hmac, Mac};
use serde_json::{Value, json};
use sha2::Sha256;

use crate::capability::Capabilities;
use crate::channel::{Channel, NotifyError, SendReceipt};
use crate::http::{HttpClient, HttpMethod, HttpRequest};
use crate::message::{Message, MessageBody};

pub struct DingTalkChannel {
    webhook_url: String,
    secret: Option<String>,
    http: Box<dyn HttpClient>,
}

impl DingTalkChannel {
    pub fn new(webhook_url: String, secret: Option<String>, http: Box<dyn HttpClient>) -> Self {
        Self {
            webhook_url,
            secret,
            http,
        }
    }

    fn signed_url(&self) -> String {
        let Some(secret) = &self.secret else {
            return self.webhook_url.clone();
        };
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let string_to_sign = format!("{timestamp}\n{secret}");
        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
            .expect("HMAC can take key of any size");
        mac.update(string_to_sign.as_bytes());
        let sign = base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes());
        format!(
            "{}&timestamp={timestamp}&sign={}",
            self.webhook_url,
            urlencoding(&sign)
        )
    }

    fn build_payload(&self, message: &Message) -> Value {
        let mut payload = match &message.body {
            MessageBody::Text { text } => json!({ "msgtype": "text", "text": { "content": text } }),
            MessageBody::Markdown { text, title } => json!({
                "msgtype": "markdown",
                "markdown": { "title": title.clone().unwrap_or_default(), "text": text }
            }),
            MessageBody::Card { title, markdown } => json!({
                "msgtype": "markdown",
                "markdown": { "title": title, "text": markdown }
            }),
        };
        if !message.mentions.is_empty() {
            let mobiles: Vec<_> = message
                .mentions
                .iter()
                .filter(|m| m.is_mobile)
                .map(|m| m.id.clone())
                .collect();
            let at_user_ids: Vec<_> = message
                .mentions
                .iter()
                .filter(|m| !m.is_mobile)
                .map(|m| m.id.clone())
                .collect();
            if !mobiles.is_empty() || !at_user_ids.is_empty() {
                payload["at"] = json!({ "atMobiles": mobiles, "atUserIds": at_user_ids });
            }
        }
        payload
    }
}

fn urlencoding(input: &str) -> String {
    let mut out = String::new();
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[async_trait]
impl Channel for DingTalkChannel {
    fn name(&self) -> &'static str {
        "dingtalk"
    }

    fn capabilities(&self) -> &Capabilities {
        &Self::CAPABILITIES
    }

    async fn send(&self, message: &Message) -> Result<SendReceipt, NotifyError> {
        let url = self.signed_url();
        let payload = self.build_payload(message);
        let request = HttpRequest {
            method: HttpMethod::Post,
            url,
            headers: vec![("Content-Type".into(), "application/json".into())],
            body: Some(payload),
        };
        let response = self
            .http
            .execute(request)
            .await?
            .ensure_success(Self::CHANNEL_NAME)?;
        let body = response.json_for(Self::CHANNEL_NAME)?;
        let success = body.get("errcode").and_then(Value::as_i64) == Some(0);
        if !success {
            let msg = body
                .get("errmsg")
                .and_then(Value::as_str)
                .unwrap_or("unknown error");
            return Err(NotifyError::Provider {
                channel: Self::CHANNEL_NAME.into(),
                http_status: response.status,
                code: body
                    .get("errcode")
                    .and_then(Value::as_i64)
                    .map(|code| code.to_string()),
                message: msg.to_string(),
                retry_after: None,
            });
        }
        Ok(SendReceipt {
            channel: Self::CHANNEL_NAME.into(),
            message_id: None,
            raw_response: Some(response.body),
        })
    }
}

impl DingTalkChannel {
    const CHANNEL_NAME: &'static str = "dingtalk";
    const CAPABILITIES: Capabilities = Capabilities::new()
        .with_markdown()
        .with_mentions(true, true, true);
}

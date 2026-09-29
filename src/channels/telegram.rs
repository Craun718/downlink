use async_trait::async_trait;
use serde_json::{json, Value};

use crate::capability::Capabilities;
use crate::channel::{Channel, NotifyError, SendReceipt};
use crate::http::{HttpClient, HttpRequest, HttpMethod};
use crate::message::{Message, MessageBody};

pub struct TelegramChannel {
    bot_token: String,
    chat_id: String,
    http: Box<dyn HttpClient>,
}

impl TelegramChannel {
    pub fn new(bot_token: String, chat_id: String, http: Box<dyn HttpClient>) -> Self {
        Self { bot_token, chat_id, http }
    }

    fn api_url(&self) -> String {
        format!("https://api.telegram.org/bot{}/sendMessage", self.bot_token)
    }

    fn build_payload(&self, message: &Message) -> Value {
        let text = message.as_text();
        let mut payload = json!({
            "chat_id": self.chat_id,
            "text": text,
        });
        if let MessageBody::Markdown { .. } | MessageBody::Card { .. } = &message.body {
            payload["parse_mode"] = json!("MarkdownV2");
        }
        if !message.mentions.is_empty() {
            let mention_text: String = message
                .mentions
                .iter()
                .map(|m| format!("@{}", m.id))
                .collect::<Vec<_>>()
                .join(" ");
            payload["text"] = json!(format!("{}\n{}", mention_text, text));
        }
        payload
    }
}

#[async_trait]
impl Channel for TelegramChannel {
    fn name(&self) -> &'static str {
        "telegram"
    }

    fn capabilities(&self) -> &Capabilities {
        &Self::CAPABILITIES
    }

    async fn send(&self, message: &Message) -> Result<SendReceipt, NotifyError> {
        let request = HttpRequest {
            method: HttpMethod::Post,
            url: self.api_url(),
            headers: vec![("Content-Type".into(), "application/json".into())],
            body: Some(self.build_payload(message)),
        };
        let response = self.http.execute(request).await?;
        let body = response.json()?;
        let ok = body.get("ok").and_then(Value::as_bool).unwrap_or(false);
        if !ok {
            let description = body
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or("unknown error");
            return Err(NotifyError::ChannelAuth(description.to_string()));
        }
        let message_id = body
            .pointer("/result/message_id")
            .and_then(Value::as_i64)
            .map(|id| id.to_string());
        Ok(SendReceipt {
            channel: Self::CHANNEL_NAME.into(),
            message_id,
            raw_response: Some(response.body),
        })
    }
}

impl TelegramChannel {
    const CHANNEL_NAME: &'static str = "telegram";
    const CAPABILITIES: Capabilities = Capabilities::new()
        .with_markdown()
        .with_mentions(false, true, false);
}

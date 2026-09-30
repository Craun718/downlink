use async_trait::async_trait;
use serde_json::{Value, json};

use crate::capability::Capabilities;
use crate::channel::{Channel, NotifyError, SendReceipt};
use crate::http::{HttpClient, HttpMethod, HttpRequest};
use crate::message::{Message, MessageBody};

pub struct DiscordChannel {
    webhook_url: String,
    http: Box<dyn HttpClient>,
}

impl DiscordChannel {
    pub fn new(webhook_url: String, http: Box<dyn HttpClient>) -> Self {
        Self { webhook_url, http }
    }

    fn build_payload(&self, message: &Message) -> Value {
        match &message.body {
            MessageBody::Card { title, markdown } => {
                json!({
                    "embeds": [{
                        "title": title,
                        "description": markdown,
                    }]
                })
            }
            _ => {
                let text = message.as_text();
                let mentions: String = message
                    .mentions
                    .iter()
                    .map(|m| format!("@{}", m.id))
                    .collect::<Vec<_>>()
                    .join(" ");
                let content = if mentions.is_empty() {
                    text
                } else {
                    format!("{mentions}\n{text}")
                };
                json!({ "content": content })
            }
        }
    }
}

#[async_trait]
impl Channel for DiscordChannel {
    fn name(&self) -> &'static str {
        "discord"
    }

    fn capabilities(&self) -> &Capabilities {
        &Self::CAPABILITIES
    }

    async fn send(&self, message: &Message) -> Result<SendReceipt, NotifyError> {
        let request = HttpRequest {
            method: HttpMethod::Post,
            url: self.webhook_url.clone(),
            headers: vec![("Content-Type".into(), "application/json".into())],
            body: Some(self.build_payload(message)),
        };
        let response = self
            .http
            .execute(request)
            .await?
            .ensure_success(Self::CHANNEL_NAME)?;
        let message_id = response
            .json_for(Self::CHANNEL_NAME)
            .ok()
            .and_then(|v| v.get("id").and_then(Value::as_str).map(String::from));
        Ok(SendReceipt {
            channel: Self::CHANNEL_NAME.into(),
            message_id,
            raw_response: Some(response.body),
        })
    }
}

impl DiscordChannel {
    const CHANNEL_NAME: &'static str = "discord";
    const CAPABILITIES: Capabilities = Capabilities::new()
        .with_markdown()
        .with_card()
        .with_mentions(false, true, false);
}

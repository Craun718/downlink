use async_trait::async_trait;
use serde_json::{Value, json};

use crate::capability::Capabilities;
use crate::channel::{Channel, NotifyError, SendReceipt};
use crate::http::{HttpClient, HttpMethod, HttpRequest};
use crate::message::{Message, MessageBody};

const API_BASE: &str = "https://discord.com/api/v10";

pub struct DiscordChannel {
    webhook_url: String,
    http: Box<dyn HttpClient>,
}

impl DiscordChannel {
    pub fn new(webhook_url: String, http: Box<dyn HttpClient>) -> Self {
        Self { webhook_url, http }
    }
}

pub struct DiscordBotChannel {
    bot_token: String,
    user_id: String,
    http: Box<dyn HttpClient>,
}

impl DiscordBotChannel {
    pub fn new(bot_token: String, user_id: String, http: Box<dyn HttpClient>) -> Self {
        Self {
            bot_token,
            user_id,
            http,
        }
    }

    fn headers(&self) -> Vec<(String, String)> {
        vec![
            ("Authorization".into(), format!("Bot {}", self.bot_token)),
            ("Content-Type".into(), "application/json".into()),
            ("User-Agent".into(), "DiscordBot".into()),
        ]
    }

    async fn open_direct_message_channel(&self) -> Result<String, NotifyError> {
        let request = HttpRequest {
            method: HttpMethod::Post,
            url: format!("{API_BASE}/users/@me/channels"),
            headers: self.headers(),
            body: Some(json!({ "recipient_id": self.user_id })),
        };
        let response = self
            .http
            .execute(request)
            .await?
            .ensure_success(Self::CHANNEL_NAME)?;
        let body = response.json_for(Self::CHANNEL_NAME)?;
        body.get("id")
            .and_then(Value::as_str)
            .map(String::from)
            .ok_or_else(|| NotifyError::Provider {
                channel: Self::CHANNEL_NAME.into(),
                http_status: response.status,
                code: body
                    .get("code")
                    .and_then(Value::as_i64)
                    .map(|code| code.to_string()),
                message: body
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("Discord did not return a direct message channel id")
                    .to_owned(),
                retry_after: None,
            })
    }
}

fn build_message_payload(message: &Message) -> Value {
    if let MessageBody::Card { title, markdown } = &message.body {
        return json!({
            "embeds": [{
                "title": title,
                "description": markdown,
            }]
        });
    }

    let text = message.as_text();
    let mentions: String = message
        .mentions
        .iter()
        .map(|m| format!("<@{}>", m.id))
        .collect::<Vec<_>>()
        .join(" ");
    let content = if mentions.is_empty() {
        text
    } else {
        format!("{mentions}\n{text}")
    };
    json!({ "content": content })
}

#[async_trait]
impl Channel for DiscordBotChannel {
    fn name(&self) -> &'static str {
        "discord-bot"
    }

    fn capabilities(&self) -> &Capabilities {
        &Self::CAPABILITIES
    }

    async fn send(&self, message: &Message) -> Result<SendReceipt, NotifyError> {
        let channel_id = self.open_direct_message_channel().await?;
        let request = HttpRequest {
            method: HttpMethod::Post,
            url: format!("{API_BASE}/channels/{channel_id}/messages"),
            headers: self.headers(),
            body: Some(build_message_payload(message)),
        };
        let response = self
            .http
            .execute(request)
            .await?
            .ensure_success(Self::CHANNEL_NAME)?;
        let body = response.json_for(Self::CHANNEL_NAME)?;
        let message_id = body.get("id").and_then(Value::as_str).map(String::from);
        Ok(SendReceipt {
            channel: Self::CHANNEL_NAME.into(),
            message_id,
            raw_response: Some(response.body),
        })
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
            body: Some(build_message_payload(message)),
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

impl DiscordBotChannel {
    const CHANNEL_NAME: &'static str = "discord-bot";
    const CAPABILITIES: Capabilities = Capabilities::new()
        .with_markdown()
        .with_card()
        .with_mentions(false, true, false);
}

use async_trait::async_trait;
use serde_json::{Value, json};

use crate::capability::Capabilities;
use crate::channel::{Channel, NotifyError, SendReceipt};
use crate::http::{HttpClient, HttpMethod, HttpRequest};
use crate::message::{Message, MessageBody};

pub struct WxPusherChannel {
    app_token: Option<String>,
    uid: String,
    http: Box<dyn HttpClient>,
}

impl WxPusherChannel {
    pub fn new(app_token: Option<String>, uid: String, http: Box<dyn HttpClient>) -> Self {
        Self {
            app_token,
            uid,
            http,
        }
    }

    fn content_for(&self, message: &Message) -> String {
        match &message.body {
            MessageBody::Card { title, markdown } => format!("{title}\n\n{markdown}"),
            MessageBody::Markdown {
                text,
                title: Some(title),
            } => format!("{title}\n\n{text}"),
            MessageBody::Markdown { text, title: None } | MessageBody::Text { text } => {
                text.clone()
            }
        }
    }
}

#[async_trait]
impl Channel for WxPusherChannel {
    fn name(&self) -> &'static str {
        "wxpusher"
    }

    fn capabilities(&self) -> &Capabilities {
        &Self::CAPABILITIES
    }

    async fn send(&self, message: &Message) -> Result<SendReceipt, NotifyError> {
        let content = self.content_for(message);
        let (url, payload) = match self.app_token.as_deref().filter(|t| !t.is_empty()) {
            Some(app_token) => (
                "https://wxpusher.zjiecode.com/api/send/message",
                json!({
                    "appToken": app_token,
                    "content": content,
                    "summary": content.chars().take(20).collect::<String>(),
                    "contentType": 1,
                    "uids": [self.uid],
                }),
            ),
            None => (
                "https://wxpusher.zjiecode.com/api/send/message/simple-push",
                json!({
                    "content": content,
                    "summary": content.chars().take(20).collect::<String>(),
                    "contentType": 1,
                    "spt": self.uid,
                }),
            ),
        };
        let request = HttpRequest {
            method: HttpMethod::Post,
            url: url.into(),
            headers: vec![("Content-Type".into(), "application/json".into())],
            body: Some(payload),
        };
        let response = self
            .http
            .execute(request)
            .await?
            .ensure_success(Self::CHANNEL_NAME)?;
        let body = response.json_for(Self::CHANNEL_NAME)?;
        if body.get("success").and_then(Value::as_bool) != Some(true) {
            return Err(NotifyError::Provider {
                channel: Self::CHANNEL_NAME.into(),
                http_status: response.status,
                code: body
                    .get("code")
                    .and_then(Value::as_i64)
                    .map(|code| code.to_string()),
                message: body
                    .get("msg")
                    .and_then(Value::as_str)
                    .unwrap_or("WxPusher rejected the request")
                    .to_owned(),
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

impl WxPusherChannel {
    const CHANNEL_NAME: &'static str = "wxpusher";
    const CAPABILITIES: Capabilities = Capabilities::new();
}

use async_trait::async_trait;
use serde_json::{Value, json};

use crate::capability::Capabilities;
use crate::channel::{Channel, NotifyError, SendReceipt};
use crate::http::{HttpClient, HttpMethod, HttpRequest};
use crate::message::{Message, MessageBody};

const MAX_MESSAGE_CHARS: usize = 1800;

pub struct QmsgChannel {
    server_url: String,
    key: String,
    user_id: String,
    bot_id: Option<String>,
    http: Box<dyn HttpClient>,
}

impl QmsgChannel {
    pub fn new(
        server_url: String,
        key: String,
        user_id: String,
        bot_id: Option<String>,
        http: Box<dyn HttpClient>,
    ) -> Self {
        Self {
            server_url,
            key,
            user_id,
            bot_id,
            http,
        }
    }

    fn endpoint(&self) -> Result<String, NotifyError> {
        let base = url::Url::parse(&self.server_url)
            .map_err(|e| NotifyError::ConfigParse(format!("qmsg: invalid server URL: {e}")))?;
        if base.scheme() != "http" && base.scheme() != "https" {
            return Err(NotifyError::ConfigParse(
                "qmsg: server URL must use http or https".into(),
            ));
        }
        let base_path = base.path().trim_end_matches('/').to_owned();
        let mut endpoint = base;
        endpoint.set_path(&format!("{base_path}/jsend/{}", self.key));
        endpoint.set_query(None);
        endpoint.set_fragment(None);
        Ok(endpoint.into())
    }

    fn content_for(&self, message: &Message) -> String {
        let title = match &message.body {
            MessageBody::Card { title, .. } => Some(title.as_str()),
            MessageBody::Markdown { title, .. } => title.as_deref(),
            MessageBody::Text { .. } => None,
        };
        let text = message.as_text();
        let content = match title {
            Some(title) => format!("{title}\n{text}"),
            None => text,
        };
        if content.chars().count() <= MAX_MESSAGE_CHARS {
            return content;
        }
        content
            .chars()
            .skip(content.chars().count() - MAX_MESSAGE_CHARS)
            .collect()
    }
}

#[async_trait]
impl Channel for QmsgChannel {
    fn name(&self) -> &'static str {
        "qmsg"
    }

    fn capabilities(&self) -> &Capabilities {
        &Self::CAPABILITIES
    }

    async fn send(&self, message: &Message) -> Result<SendReceipt, NotifyError> {
        let mut payload = json!({
            "msg": self.content_for(message),
            "qq": self.user_id,
        });
        if let Some(bot_id) = &self.bot_id {
            payload["bot"] = json!(bot_id);
        }
        let request = HttpRequest {
            method: HttpMethod::Post,
            url: self.endpoint()?,
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
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("Qmsg rejected the request")
                    .to_owned(),
                retry_after: None,
            });
        }
        Ok(SendReceipt {
            channel: Self::CHANNEL_NAME.into(),
            message_id: body.get("result").map(|id| id.to_string()),
            raw_response: Some(response.body),
        })
    }
}

impl QmsgChannel {
    const CHANNEL_NAME: &'static str = "qmsg";
    const CAPABILITIES: Capabilities = Capabilities::new();
}

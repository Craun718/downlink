use async_trait::async_trait;
use serde_json::{Value, json};

use crate::capability::Capabilities;
use crate::channel::{Channel, NotifyError, SendReceipt};
use crate::http::{HttpClient, HttpMethod, HttpRequest};
use crate::message::{Message, MessageBody, Priority};

pub struct GotifyChannel {
    server_url: String,
    token: String,
    http: Box<dyn HttpClient>,
}

impl GotifyChannel {
    pub fn new(server_url: String, token: String, http: Box<dyn HttpClient>) -> Self {
        Self {
            server_url,
            token,
            http,
        }
    }

    fn endpoint(&self) -> Result<String, NotifyError> {
        let base = url::Url::parse(&self.server_url)
            .map_err(|e| NotifyError::ConfigParse(format!("gotify: invalid server URL: {e}")))?;
        if base.scheme() != "http" && base.scheme() != "https" {
            return Err(NotifyError::ConfigParse(
                "gotify: server URL must use http or https".into(),
            ));
        }
        let base_path = base.path().trim_end_matches('/').to_owned();
        let mut endpoint = base;
        endpoint.set_path(&format!("{base_path}/message"));
        endpoint.set_query(None);
        endpoint.set_fragment(None);
        Ok(endpoint.into())
    }

    fn title_for(&self, message: &Message) -> String {
        match &message.body {
            MessageBody::Card { title, .. } => title.clone(),
            MessageBody::Markdown { title, .. } => {
                title.clone().unwrap_or_else(|| "Notification".into())
            }
            MessageBody::Text { .. } => match message.priority {
                Priority::Critical => "[CRITICAL] Notification".into(),
                Priority::High => "[HIGH] Notification".into(),
                _ => "Notification".into(),
            },
        }
    }
}

#[async_trait]
impl Channel for GotifyChannel {
    fn name(&self) -> &'static str {
        "gotify"
    }

    fn capabilities(&self) -> &Capabilities {
        &Self::CAPABILITIES
    }

    async fn send(&self, message: &Message) -> Result<SendReceipt, NotifyError> {
        let payload = json!({
            "title": self.title_for(message),
            "message": message.as_text(),
            "priority": match message.priority {
                Priority::Low => 1,
                Priority::Normal => 5,
                Priority::High => 8,
                Priority::Critical => 10,
            },
        });
        let request = HttpRequest {
            method: HttpMethod::Post,
            url: self.endpoint()?,
            headers: vec![
                ("Content-Type".into(), "application/json".into()),
                ("X-Gotify-Key".into(), self.token.clone()),
            ],
            body: Some(payload),
        };
        let response = self
            .http
            .execute(request)
            .await?
            .ensure_success(Self::CHANNEL_NAME)?;
        let body = response.json_for(Self::CHANNEL_NAME)?;
        let message_id = body
            .get("id")
            .map(|id| id.to_string())
            .filter(|id| id != "null");
        let Some(message_id) = message_id else {
            return Err(NotifyError::Provider {
                channel: Self::CHANNEL_NAME.into(),
                http_status: response.status,
                code: body
                    .get("error")
                    .and_then(Value::as_i64)
                    .map(|code| code.to_string()),
                message: body
                    .get("errorDescription")
                    .and_then(Value::as_str)
                    .unwrap_or("Gotify did not return a message id")
                    .to_owned(),
                retry_after: None,
            });
        };
        Ok(SendReceipt {
            channel: Self::CHANNEL_NAME.into(),
            message_id: Some(message_id),
            raw_response: Some(response.body),
        })
    }
}

impl GotifyChannel {
    const CHANNEL_NAME: &'static str = "gotify";
    const CAPABILITIES: Capabilities = Capabilities::new().with_markdown();
}

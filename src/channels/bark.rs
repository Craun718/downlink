use async_trait::async_trait;
use serde_json::{Value, json};

use crate::capability::Capabilities;
use crate::channel::{Channel, NotifyError, SendReceipt};
use crate::http::{HttpClient, HttpMethod, HttpRequest};
use crate::message::{Message, MessageBody};

pub struct BarkChannel {
    server_url: String,
    device_key: String,
    http: Box<dyn HttpClient>,
}

impl BarkChannel {
    pub fn new(server_url: String, device_key: String, http: Box<dyn HttpClient>) -> Self {
        Self {
            server_url,
            device_key,
            http,
        }
    }

    fn endpoint(&self) -> Result<String, NotifyError> {
        let base = url::Url::parse(&self.server_url)
            .map_err(|e| NotifyError::ConfigParse(format!("bark: invalid server URL: {e}")))?;
        if base.scheme() != "http" && base.scheme() != "https" {
            return Err(NotifyError::ConfigParse(
                "bark: server URL must use http or https".into(),
            ));
        }
        let base_path = base.path().trim_end_matches('/').to_owned();
        let mut endpoint = base;
        endpoint.set_path(&format!("{base_path}/push"));
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
            MessageBody::Text { .. } => "Notification".into(),
        }
    }

    fn build_payload(&self, message: &Message) -> Value {
        json!({
            "title": self.title_for(message),
            "body": message.as_text(),
            "device_key": self.device_key,
            "group": "downlink",
        })
    }
}

#[async_trait]
impl Channel for BarkChannel {
    fn name(&self) -> &'static str {
        "bark"
    }

    fn capabilities(&self) -> &Capabilities {
        &Self::CAPABILITIES
    }

    async fn send(&self, message: &Message) -> Result<SendReceipt, NotifyError> {
        let request = HttpRequest {
            method: HttpMethod::Post,
            url: self.endpoint()?,
            headers: vec![("Content-Type".into(), "application/json".into())],
            body: Some(self.build_payload(message)),
        };
        let response = self
            .http
            .execute(request)
            .await?
            .ensure_success(Self::CHANNEL_NAME)?;
        let body = response.json_for(Self::CHANNEL_NAME)?;
        let code = body.get("code").and_then(Value::as_i64);
        if code != Some(200) {
            return Err(NotifyError::Provider {
                channel: Self::CHANNEL_NAME.into(),
                http_status: response.status,
                code: code.map(|code| code.to_string()),
                message: body
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("Bark rejected the request")
                    .to_owned(),
                retry_after: None,
            });
        }
        Ok(SendReceipt {
            channel: Self::CHANNEL_NAME.into(),
            message_id: body
                .pointer("/data/task_id")
                .and_then(Value::as_str)
                .map(String::from),
            raw_response: Some(response.body),
        })
    }
}

impl BarkChannel {
    const CHANNEL_NAME: &'static str = "bark";
    const CAPABILITIES: Capabilities = Capabilities::new();
}

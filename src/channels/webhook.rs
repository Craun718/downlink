use async_trait::async_trait;
use serde_json::{Value, json};

use crate::capability::Capabilities;
use crate::channel::{Channel, NotifyError, SendReceipt};
use crate::config::WebhookHeader;
use crate::http::{HttpClient, HttpMethod, HttpRequest};
use crate::message::{Message, MessageBody, Priority};

pub struct CustomWebhookChannel {
    url: String,
    headers: Vec<WebhookHeader>,
    body_template: Option<String>,
    content_type: Option<String>,
    http: Box<dyn HttpClient>,
}

impl CustomWebhookChannel {
    pub fn new(
        url: String,
        headers: Vec<WebhookHeader>,
        body_template: Option<String>,
        content_type: Option<String>,
        http: Box<dyn HttpClient>,
    ) -> Self {
        Self {
            url,
            headers,
            body_template,
            content_type,
            http,
        }
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

    fn default_body(&self, message: &Message) -> Value {
        json!({
            "title": self.title_for(message),
            "message": message.as_text(),
            "priority": message.priority,
        })
    }

    fn render_body(&self, message: &Message) -> Result<Value, NotifyError> {
        let Some(template) = &self.body_template else {
            return Ok(self.default_body(message));
        };
        let parsed: Value = serde_json::from_str(template).map_err(|e| {
            NotifyError::MessageConversion(format!("webhook body template must be valid JSON: {e}"))
        })?;
        Ok(render_template(
            &parsed,
            &self.title_for(message),
            &message.as_text(),
            message.priority,
        ))
    }

    fn headers(&self) -> Result<Vec<(String, String)>, NotifyError> {
        let mut headers = Vec::new();
        for header in &self.headers {
            if header.name.is_empty()
                || header.name.bytes().any(|byte| {
                    byte <= b' ' || byte == b'\x7f' || b"()<>@,;:\\\"/[]?={}".contains(&byte)
                })
            {
                return Err(NotifyError::ConfigParse(format!(
                    "webhook: invalid header name '{}'",
                    header.name
                )));
            }
            headers.push((header.name.clone(), header.value.clone()));
        }
        headers.push((
            "Content-Type".into(),
            self.content_type
                .clone()
                .unwrap_or_else(|| "application/json".into()),
        ));
        Ok(headers)
    }
}

fn render_template(value: &Value, title: &str, message: &str, priority: Priority) -> Value {
    match value {
        Value::String(text) => Value::String(
            text.replace("{title}", title)
                .replace("{content}", message)
                .replace("{message}", message)
                .replace("{priority}", priority_label(priority)),
        ),
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| render_template(item, title, message, priority))
                .collect(),
        ),
        Value::Object(fields) => Value::Object(
            fields
                .iter()
                .map(|(key, item)| (key.clone(), render_template(item, title, message, priority)))
                .collect(),
        ),
        other => other.clone(),
    }
}

fn priority_label(priority: Priority) -> &'static str {
    match priority {
        Priority::Low => "low",
        Priority::Normal => "normal",
        Priority::High => "high",
        Priority::Critical => "critical",
    }
}

#[async_trait]
impl Channel for CustomWebhookChannel {
    fn name(&self) -> &'static str {
        "webhook"
    }

    fn capabilities(&self) -> &Capabilities {
        &Self::CAPABILITIES
    }

    async fn send(&self, message: &Message) -> Result<SendReceipt, NotifyError> {
        let request = HttpRequest {
            method: HttpMethod::Post,
            url: self.url.clone(),
            headers: self.headers()?,
            body: Some(self.render_body(message)?),
        };
        let response = self
            .http
            .execute(request)
            .await?
            .ensure_success(Self::CHANNEL_NAME)?;
        Ok(SendReceipt {
            channel: Self::CHANNEL_NAME.into(),
            message_id: None,
            raw_response: Some(response.body),
        })
    }
}

impl CustomWebhookChannel {
    const CHANNEL_NAME: &'static str = "webhook";
    const CAPABILITIES: Capabilities = Capabilities::new().with_markdown().with_card();
}

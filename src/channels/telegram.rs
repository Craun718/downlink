use async_trait::async_trait;
use serde_json::{Value, json};

use crate::capability::Capabilities;
use crate::channel::{Channel, NotifyError, SendReceipt};
use crate::http::{HttpClient, HttpMethod, HttpRequest, HttpResponse};
use crate::message::{Message, MessageBody};

pub struct TelegramChannel {
    bot_token: String,
    chat_id: String,
    http: Box<dyn HttpClient>,
}

impl TelegramChannel {
    pub fn new(bot_token: String, chat_id: String, http: Box<dyn HttpClient>) -> Self {
        Self {
            bot_token,
            chat_id,
            http,
        }
    }

    fn api_url(&self) -> String {
        format!("https://api.telegram.org/bot{}/sendMessage", self.bot_token)
    }

    fn error_from_response(response: &HttpResponse) -> Option<NotifyError> {
        let body: Value = serde_json::from_str(&response.body).ok()?;
        if body.get("ok").and_then(Value::as_bool) != Some(false) {
            return None;
        }
        Some(Self::provider_error(response, &body))
    }

    fn provider_error(response: &HttpResponse, body: &Value) -> NotifyError {
        let message = body
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("Telegram rejected the request")
            .to_owned();
        NotifyError::Provider {
            channel: Self::CHANNEL_NAME.into(),
            http_status: response.status,
            code: body
                .get("error_code")
                .and_then(Value::as_i64)
                .map(|code| code.to_string()),
            message,
            retry_after: body
                .pointer("/parameters/retry_after")
                .and_then(Value::as_u64)
                .map(std::time::Duration::from_secs)
                .or_else(|| response.retry_after()),
        }
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
        if !response.is_success() {
            return Err(Self::error_from_response(&response)
                .unwrap_or_else(|| response.into_http_error(Self::CHANNEL_NAME)));
        }

        let body = response.json_for(Self::CHANNEL_NAME)?;
        let ok = body.get("ok").and_then(Value::as_bool).unwrap_or(false);
        if !ok {
            return Err(Self::provider_error(&response, &body));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channel::ErrorKind;

    #[test]
    fn non_success_rate_limit_response_preserves_provider_retry_after() {
        let response = HttpResponse {
            status: 429,
            headers: vec![],
            body: r#"{
                "ok": false,
                "error_code": 429,
                "description": "Too Many Requests",
                "parameters": { "retry_after": 13 }
            }"#
            .into(),
        };

        let error = TelegramChannel::error_from_response(&response)
            .expect("Telegram's documented error payload must be preserved");

        assert_eq!(error.kind(), ErrorKind::RateLimited);
        assert_eq!(error.channel(), Some("telegram"));
        assert_eq!(error.http_status(), Some(429));
        assert_eq!(
            error.retry_after(),
            Some(std::time::Duration::from_secs(13))
        );
        assert!(error.is_retryable());
        assert!(matches!(
            error,
            NotifyError::Provider {
                code: Some(ref code),
                ..
            } if code == "429"
        ));
    }

    #[test]
    fn unknown_non_success_response_falls_back_to_shared_http_error() {
        let response = HttpResponse {
            status: 502,
            headers: vec![],
            body: "upstream failure".into(),
        };
        let error = TelegramChannel::error_from_response(&response)
            .unwrap_or_else(|| response.into_http_error(TelegramChannel::CHANNEL_NAME));

        assert_eq!(error.kind(), ErrorKind::RemoteServer);
        assert!(error.is_retryable());
        assert!(matches!(error, NotifyError::HttpStatus { .. }));
    }
}

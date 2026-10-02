use async_trait::async_trait;
use serde_json::Value;

use crate::capability::Capabilities;
use crate::channel::{Channel, NotifyError, SendReceipt};
use crate::http::{HttpClient, HttpMethod, HttpRequest};
use crate::message::{Message, MessageBody};

pub struct ServerChanChannel {
    send_key: String,
    http: Box<dyn HttpClient>,
}

impl ServerChanChannel {
    pub fn new(send_key: String, http: Box<dyn HttpClient>) -> Self {
        Self { send_key, http }
    }

    fn endpoint(&self) -> Result<String, NotifyError> {
        if self.send_key.starts_with("sctp") {
            let digits = self.send_key[4..]
                .chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>();
            if digits.is_empty() || !self.send_key[4 + digits.len()..].starts_with('t') {
                return Err(NotifyError::ConfigParse(
                    "serverchan: sctp send_key must match sctp<number>t...".into(),
                ));
            }
            return Ok(format!(
                "https://{}.push.ft07.com/send/{}.send",
                digits, self.send_key
            ));
        }
        Ok(format!("https://sctapi.ftqq.com/{}.send", self.send_key))
    }

    fn title_for(&self, message: &Message) -> String {
        let title = match &message.body {
            MessageBody::Card { title, .. } => Some(title.as_str()),
            MessageBody::Markdown { title, .. } => title.as_deref(),
            MessageBody::Text { .. } => None,
        };
        let title = title.unwrap_or("Notification");
        title.chars().filter(|c| *c != '\n').take(32).collect()
    }

    fn url(&self, message: &Message) -> Result<String, NotifyError> {
        let endpoint = self.endpoint()?;
        let title = percent_encode(&self.title_for(message));
        let desp = percent_encode(&message.as_text());
        Ok(format!("{endpoint}?title={title}&desp={desp}"))
    }
}

#[async_trait]
impl Channel for ServerChanChannel {
    fn name(&self) -> &'static str {
        "serverchan"
    }

    fn capabilities(&self) -> &Capabilities {
        &Self::CAPABILITIES
    }

    async fn send(&self, message: &Message) -> Result<SendReceipt, NotifyError> {
        let request = HttpRequest {
            method: HttpMethod::Get,
            url: self.url(message)?,
            headers: Vec::new(),
            body: None,
        };
        let response = self
            .http
            .execute(request)
            .await?
            .ensure_success(Self::CHANNEL_NAME)?;
        let body = response.json_for(Self::CHANNEL_NAME)?;
        let code = body.get("code").and_then(Value::as_i64);
        if code != Some(0) {
            return Err(NotifyError::Provider {
                channel: Self::CHANNEL_NAME.into(),
                http_status: response.status,
                code: code.map(|code| code.to_string()),
                message: body
                    .get("message")
                    .or_else(|| body.get("errmsg"))
                    .and_then(Value::as_str)
                    .unwrap_or("ServerChan rejected the request")
                    .to_owned(),
                retry_after: None,
            });
        }
        Ok(SendReceipt {
            channel: Self::CHANNEL_NAME.into(),
            message_id: body
                .pointer("/data/pushid")
                .and_then(Value::as_str)
                .map(String::from),
            raw_response: Some(response.body),
        })
    }
}

fn percent_encode(input: &str) -> String {
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

impl ServerChanChannel {
    const CHANNEL_NAME: &'static str = "serverchan";
    const CAPABILITIES: Capabilities = Capabilities::new().with_markdown();
}

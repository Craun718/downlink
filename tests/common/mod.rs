#![allow(dead_code)]

use downlink::{ChannelConfig, HttpClient, HttpRequest, HttpResponse, NotifyError};
use serde::Deserialize;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct MockState {
    requests: Mutex<Vec<HttpRequest>>,
    responses: Mutex<VecDeque<HttpResponse>>,
}

/// Cloneable in-memory double for `HttpClient`: records every request and
/// replays queued responses without touching the network. Clone it before
/// handing one copy to a channel so the test can inspect recorded requests.
#[derive(Clone, Default)]
pub struct MockHttpClient {
    state: Arc<MockState>,
}

impl MockHttpClient {
    pub fn new(response: HttpResponse) -> Self {
        let client = Self::default();
        client.push_response(response);
        client
    }

    pub fn push_response(&self, response: HttpResponse) {
        self.state.responses.lock().unwrap().push_back(response);
    }

    pub fn requests(&self) -> Vec<HttpRequest> {
        self.state.requests.lock().unwrap().clone()
    }
}

#[async_trait::async_trait]
impl HttpClient for MockHttpClient {
    async fn execute(&self, request: HttpRequest) -> Result<HttpResponse, NotifyError> {
        self.state.requests.lock().unwrap().push(request);
        let mut queue = self.state.responses.lock().unwrap();
        Ok(queue.pop_front().expect("no queued mock response"))
    }
}

pub fn ok_response(body: &str) -> HttpResponse {
    status_response(200, body)
}

pub fn status_response(status: u16, body: &str) -> HttpResponse {
    HttpResponse {
        status,
        headers: vec![("Content-Type".into(), "application/json".into())],
        body: body.to_owned(),
    }
}

/// Credentials are stored as separate id/secret keys; full URLs are only
/// assembled at runtime via `to_config`, never persisted.
#[derive(Debug, Deserialize)]
pub struct DingTalkSecrets {
    pub access_token: String,
    pub secret: Option<String>,
}

impl DingTalkSecrets {
    pub fn to_config(&self) -> ChannelConfig {
        ChannelConfig::DingTalk {
            webhook_url: format!(
                "https://oapi.dingtalk.com/robot/send?access_token={}",
                self.access_token
            ),
            secret: self.secret.clone(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct FeishuSecrets {
    pub hook_id: String,
    pub secret: Option<String>,
}

impl FeishuSecrets {
    pub fn to_config(&self) -> ChannelConfig {
        ChannelConfig::Feishu {
            webhook_url: format!(
                "https://open.feishu.cn/open-apis/bot/v2/hook/{}",
                self.hook_id
            ),
            secret: self.secret.clone(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct TelegramSecrets {
    pub bot_token: String,
    pub chat_id: String,
}

impl TelegramSecrets {
    pub fn to_config(&self) -> ChannelConfig {
        ChannelConfig::Telegram {
            bot_token: self.bot_token.clone(),
            chat_id: self.chat_id.clone(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct DiscordSecrets {
    pub webhook_id: String,
    pub webhook_token: String,
}

impl DiscordSecrets {
    pub fn to_config(&self) -> ChannelConfig {
        ChannelConfig::Discord {
            webhook_url: format!(
                "https://discord.com/api/webhooks/{}/{}",
                self.webhook_id, self.webhook_token
            ),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct SmtpSecrets {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub from: String,
    pub to: Vec<String>,
}

impl SmtpSecrets {
    pub fn to_config(&self) -> ChannelConfig {
        ChannelConfig::Smtp {
            host: self.host.clone(),
            port: self.port,
            username: self.username.clone(),
            password: self.password.clone(),
            from: self.from.clone(),
            to: self.to.clone(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct TestSecrets {
    pub dingtalk: Option<DingTalkSecrets>,
    pub feishu: Option<FeishuSecrets>,
    pub telegram: Option<TelegramSecrets>,
    pub discord: Option<DiscordSecrets>,
    pub smtp: Option<SmtpSecrets>,
}

impl TestSecrets {
    /// Loads real credentials from `NOTIFY_TEST_SECRETS_JSON` (raw JSON,
    /// highest priority) or `tests/fixtures/secrets.json`. Returns `None`
    /// when neither source exists so callers can skip instead of fail.
    pub fn load() -> Option<Self> {
        if let Ok(raw) = std::env::var("NOTIFY_TEST_SECRETS_JSON") {
            return Some(
                serde_json::from_str(&raw)
                    .expect("NOTIFY_TEST_SECRETS_JSON must be valid test secrets JSON"),
            );
        }
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/secrets.json");
        let raw = std::fs::read_to_string(path).ok()?;
        Some(
            serde_json::from_str(&raw)
                .expect("tests/fixtures/secrets.json must be valid test secrets JSON"),
        )
    }
}

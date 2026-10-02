use serde::{Deserialize, Serialize};

use crate::channel::NotifyError;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "channel", rename_all = "snake_case")]
pub enum ChannelConfig {
    DingTalk {
        webhook_url: String,
        secret: Option<String>,
    },
    Feishu {
        webhook_url: String,
        secret: Option<String>,
    },
    Telegram {
        bot_token: String,
        chat_id: String,
    },
    Discord {
        webhook_url: String,
    },
    DiscordBot {
        bot_token: String,
        user_id: String,
    },
    ServerChan {
        send_key: String,
    },
    Bark {
        server_url: String,
        device_key: String,
    },
    Gotify {
        server_url: String,
        token: String,
    },
    Qmsg {
        server_url: String,
        key: String,
        user_id: String,
        bot_id: Option<String>,
    },
    WxPusher {
        app_token: Option<String>,
        uid: String,
    },
    CustomWebhook {
        url: String,
        #[serde(default)]
        headers: Vec<WebhookHeader>,
        #[serde(default)]
        body_template: Option<String>,
        #[serde(default)]
        content_type: Option<String>,
    },
    Smtp {
        host: String,
        port: u16,
        username: String,
        password: String,
        from: String,
        to: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebhookHeader {
    pub name: String,
    pub value: String,
}

impl ChannelConfig {
    pub fn from_json(input: &str) -> Result<Self, NotifyError> {
        serde_json::from_str(input)
            .map_err(|e| NotifyError::ConfigParse(format!("invalid JSON config: {e}")))
    }

    pub fn to_json(&self) -> Result<String, NotifyError> {
        serde_json::to_string(self)
            .map_err(|e| NotifyError::ConfigParse(format!("serialization failed: {e}")))
    }
}

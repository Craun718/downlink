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
    Smtp {
        host: String,
        port: u16,
        username: String,
        password: String,
        from: String,
        to: Vec<String>,
    },
}

impl ChannelConfig {
    pub fn from_url(input: &str) -> Result<Self, NotifyError> {
        let parsed = url::Url::parse(input)
            .map_err(|e| NotifyError::ConfigParse(format!("invalid URL: {e}")))?;
        let scheme = parsed.scheme();
        let host = parsed.host_str().unwrap_or_default();
        let path = parsed.path().trim_start_matches('/');
        let params: std::collections::HashMap<_, _> = parsed
            .query_pairs()
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();

        match scheme {
            "dingtalk" => {
                let access_token = host;
                let webhook_url = format!("https://oapi.dingtalk.com/robot/send?access_token={access_token}");
                Ok(Self::DingTalk {
                    webhook_url,
                    secret: params.get("secret").cloned(),
                })
            }
            "feishu" => {
                let webhook_url = format!("https://open.feishu.cn/open-apis/bot/v2/hook/{host}{path}");
                Ok(Self::Feishu {
                    webhook_url,
                    secret: params.get("secret").cloned(),
                })
            }
            "telegram" => {
                let bot_token = host;
                let chat_id = path
                    .split('/')
                    .find(|s| !s.is_empty())
                    .ok_or_else(|| NotifyError::ConfigParse("telegram: missing chat_id in path".into()))?
                    .to_string();
                Ok(Self::Telegram { bot_token: bot_token.to_string(), chat_id })
            }
            "discord" => {
                let webhook_url = format!("https://discord.com/api/webhooks/{host}{path}");
                Ok(Self::Discord { webhook_url })
            }
            "smtp" => {
                let port = parsed
                    .port()
                    .unwrap_or(587);
                let username = parsed
                    .username()
                    .to_string();
                let password = parsed
                    .password()
                    .unwrap_or_default()
                    .to_string();
                let to: Vec<String> = params
                    .get("to")
                    .map(|v| v.split(',').map(String::from).collect())
                    .unwrap_or_default();
                if username.is_empty() || to.is_empty() {
                    return Err(NotifyError::ConfigParse(
                        "smtp: URL requires user:pass@host and ?to=addr1,addr2".into(),
                    ));
                }
                let from = username.clone();
                Ok(Self::Smtp {
                    host: host.to_string(),
                    port,
                    username,
                    password,
                    from,
                    to,
                })
            }
            _ => Err(NotifyError::ConfigParse(format!("unknown channel scheme: {scheme}"))),
        }
    }

    pub fn from_json(input: &str) -> Result<Self, NotifyError> {
        serde_json::from_str(input)
            .map_err(|e| NotifyError::ConfigParse(format!("invalid JSON config: {e}")))
    }

    pub fn to_json(&self) -> Result<String, NotifyError> {
        serde_json::to_string(self)
            .map_err(|e| NotifyError::ConfigParse(format!("serialization failed: {e}")))
    }
}

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
                let webhook_url =
                    format!("https://oapi.dingtalk.com/robot/send?access_token={access_token}");
                Ok(Self::DingTalk {
                    webhook_url,
                    secret: params.get("secret").cloned(),
                })
            }
            "feishu" => {
                let webhook_url =
                    format!("https://open.feishu.cn/open-apis/bot/v2/hook/{host}{path}");
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
                    .ok_or_else(|| {
                        NotifyError::ConfigParse("telegram: missing chat_id in path".into())
                    })?
                    .to_string();
                Ok(Self::Telegram {
                    bot_token: bot_token.to_string(),
                    chat_id,
                })
            }
            "discord" => {
                let webhook_url = format!("https://discord.com/api/webhooks/{host}{path}");
                Ok(Self::Discord { webhook_url })
            }
            "discord-bot" => {
                let bot_token =
                    required_userinfo(parsed.username(), "discord-bot: missing bot_token")?;
                let user_id = required_param(&params, "user_id", "discord-bot")?;
                Ok(Self::DiscordBot { bot_token, user_id })
            }
            "serverchan" => {
                let send_key = required_userinfo(
                    parsed.username(),
                    "serverchan: missing send_key before '@'",
                )?;
                Ok(Self::ServerChan { send_key })
            }
            "bark" => {
                let device_key = required_userinfo(parsed.username(), "bark: missing device_key")?;
                let server_url =
                    server_url_from_parts(&parsed, &params, "bark", "https://api.day.app")?;
                Ok(Self::Bark {
                    server_url,
                    device_key,
                })
            }
            "gotify" => {
                let token = required_userinfo(parsed.username(), "gotify: missing token")?;
                let server_url = server_url_from_parts(
                    &parsed,
                    &params,
                    "gotify",
                    "https://gotify.example.com",
                )?;
                Ok(Self::Gotify { server_url, token })
            }
            "qmsg" => {
                let key = required_userinfo(parsed.username(), "qmsg: missing key")?;
                let server_url =
                    server_url_from_parts(&parsed, &params, "qmsg", "https://qmsg.zendee.cn")?;
                let user_id = required_param(&params, "user_id", "qmsg")
                    .or_else(|_| required_param(&params, "qq", "qmsg"))?;
                Ok(Self::Qmsg {
                    server_url,
                    key,
                    user_id,
                    bot_id: params.get("bot").cloned(),
                })
            }
            "wxpusher" => {
                let uid = params.get("uid").cloned().unwrap_or_else(|| {
                    if host != "send" {
                        host.to_string()
                    } else {
                        String::new()
                    }
                });
                if uid.is_empty() {
                    return Err(NotifyError::ConfigParse(
                        "wxpusher: missing uid in path/query".into(),
                    ));
                }
                Ok(Self::WxPusher {
                    app_token: params.get("app_token").cloned(),
                    uid,
                })
            }
            "webhook" => Self::custom_webhook_from_url(parsed, &params),
            "smtp" => {
                let port = parsed.port().unwrap_or(587);
                let username = parsed.username().to_string();
                let password = parsed.password().unwrap_or_default().to_string();
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
            _ => Err(NotifyError::ConfigParse(format!(
                "unknown channel scheme: {scheme}"
            ))),
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

fn required_userinfo(value: &str, message: &str) -> Result<String, NotifyError> {
    if value.is_empty() {
        Err(NotifyError::ConfigParse(message.into()))
    } else {
        Ok(value.to_string())
    }
}

fn required_param(
    params: &std::collections::HashMap<String, String>,
    name: &str,
    channel: &str,
) -> Result<String, NotifyError> {
    params
        .get(name)
        .cloned()
        .filter(|v| !v.is_empty())
        .ok_or_else(|| {
            NotifyError::ConfigParse(format!(
                "{channel}: missing required query parameter '{name}'"
            ))
        })
}

fn server_url_from_parts(
    parsed: &url::Url,
    params: &std::collections::HashMap<String, String>,
    channel: &str,
    default: &str,
) -> Result<String, NotifyError> {
    if let Some(server) = params.get("server").filter(|v| !v.is_empty()) {
        return validate_http_url(server, channel);
    }

    let host = parsed.host_str().unwrap_or_default();
    if host.is_empty() || host == "send" {
        return validate_http_url(default, channel);
    }
    let port = parsed
        .port()
        .map(|port| format!(":{port}"))
        .unwrap_or_default();
    let scheme = params.get("scheme").map(String::as_str).unwrap_or("https");
    if scheme != "http" && scheme != "https" {
        return Err(NotifyError::ConfigParse(format!(
            "{channel}: scheme must be http or https"
        )));
    }
    validate_http_url(
        &format!("{scheme}://{host}{port}{}", parsed.path()),
        channel,
    )
}

fn validate_http_url(value: &str, channel: &str) -> Result<String, NotifyError> {
    let parsed = url::Url::parse(value)
        .map_err(|e| NotifyError::ConfigParse(format!("{channel}: invalid server URL: {e}")))?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err(NotifyError::ConfigParse(format!(
            "{channel}: server URL must use http or https"
        )));
    }
    Ok(parsed.to_string())
}

impl ChannelConfig {
    fn custom_webhook_from_url(
        parsed: url::Url,
        params: &std::collections::HashMap<String, String>,
    ) -> Result<Self, NotifyError> {
        let scheme = params.get("scheme").map(String::as_str).unwrap_or("https");
        if scheme != "http" && scheme != "https" {
            return Err(NotifyError::ConfigParse(
                "webhook: scheme must be http or https".into(),
            ));
        }
        let host = parsed
            .host_str()
            .filter(|host| !host.is_empty() && *host != "send")
            .ok_or_else(|| NotifyError::ConfigParse("webhook: missing host".into()))?;
        let port = parsed
            .port()
            .map(|port| format!(":{port}"))
            .unwrap_or_default();
        let mut target = url::Url::parse(&format!("{scheme}://{host}{port}{}", parsed.path()))
            .map_err(|e| NotifyError::ConfigParse(format!("webhook: invalid target URL: {e}")))?;

        let controls = ["scheme", "template", "body", "content_type"];
        let mut headers = Vec::new();
        {
            let mut query = target.query_pairs_mut();
            query.clear();
            for (key, value) in parsed.query_pairs() {
                if let Some(name) = key.strip_prefix("header.") {
                    headers.push(WebhookHeader {
                        name: name.to_string(),
                        value: value.to_string(),
                    });
                } else if !controls.contains(&key.as_ref()) {
                    query.append_pair(&key, &value);
                }
            }
        }

        Ok(Self::CustomWebhook {
            url: target.to_string(),
            headers,
            body_template: params
                .get("template")
                .or_else(|| params.get("body"))
                .cloned(),
            content_type: params.get("content_type").cloned(),
        })
    }
}

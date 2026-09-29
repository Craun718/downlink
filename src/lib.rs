//! 通知聚合协议的 Rust core SDK。
//!
//! 只发不收，跨平台。统一消息模型 + 渠道适配器 trait，
//! 首发支持钉钉、飞书、Telegram、Discord、SMTP 邮箱。

pub mod capability;
pub mod channel;
pub mod channels;
pub mod config;
pub mod http;
pub mod message;

pub use capability::Capabilities;
pub use channel::{Channel, NotifyError, SendReceipt};
pub use config::ChannelConfig;
pub use http::{HttpClient, HttpRequest, HttpResponse};
pub use message::{Message, MessageBody, Mention, Priority};

#[cfg(feature = "default-client")]
pub use http::default::ReqwestClient;

pub fn channel_from_config(
    config: &ChannelConfig,
    http: Box<dyn HttpClient>,
) -> Result<Box<dyn Channel>, NotifyError> {
    match config {
        ChannelConfig::DingTalk { webhook_url, secret } => Ok(Box::new(
            channels::dingtalk::DingTalkChannel::new(webhook_url.clone(), secret.clone(), http),
        )),
        ChannelConfig::Feishu { webhook_url, secret } => Ok(Box::new(
            channels::feishu::FeishuChannel::new(webhook_url.clone(), secret.clone(), http),
        )),
        ChannelConfig::Telegram { bot_token, chat_id } => Ok(Box::new(
            channels::telegram::TelegramChannel::new(bot_token.clone(), chat_id.clone(), http),
        )),
        ChannelConfig::Discord { webhook_url } => Ok(Box::new(
            channels::discord::DiscordChannel::new(webhook_url.clone(), http),
        )),
        ChannelConfig::Smtp { host, port, username, password, from, to } => Ok(Box::new(
            channels::smtp::SmtpChannel::new(host.clone(), *port, username.clone(), password.clone(), from.clone(), to.clone()),
        )),
    }
}

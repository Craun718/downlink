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
pub use channel::{Channel, ErrorKind, NotifyError, ResponseBody, SendReceipt};
pub use config::ChannelConfig;
pub use http::{HttpClient, HttpRequest, HttpResponse};
pub use message::{Mention, Message, MessageBody, Priority};

/// Send a message through a channel using the shared capability negotiation.
pub async fn send(channel: &dyn Channel, message: &Message) -> Result<SendReceipt, NotifyError> {
    channel.send_with_fallback(message).await
}

#[cfg(feature = "default-client")]
pub use http::default::ReqwestClient;

pub fn channel_from_config(
    config: &ChannelConfig,
    http: Box<dyn HttpClient>,
) -> Result<Box<dyn Channel>, NotifyError> {
    #[cfg(not(any(
        feature = "channel-dingtalk",
        feature = "channel-feishu",
        feature = "channel-telegram",
        feature = "channel-discord",
    )))]
    let _ = &http;

    match config {
        #[cfg(feature = "channel-dingtalk")]
        ChannelConfig::DingTalk {
            webhook_url,
            secret,
        } => Ok(Box::new(channels::dingtalk::DingTalkChannel::new(
            webhook_url.clone(),
            secret.clone(),
            http,
        ))),
        #[cfg(feature = "channel-feishu")]
        ChannelConfig::Feishu {
            webhook_url,
            secret,
        } => Ok(Box::new(channels::feishu::FeishuChannel::new(
            webhook_url.clone(),
            secret.clone(),
            http,
        ))),
        #[cfg(feature = "channel-telegram")]
        ChannelConfig::Telegram { bot_token, chat_id } => Ok(Box::new(
            channels::telegram::TelegramChannel::new(bot_token.clone(), chat_id.clone(), http),
        )),
        #[cfg(feature = "channel-discord")]
        ChannelConfig::Discord { webhook_url } => Ok(Box::new(
            channels::discord::DiscordChannel::new(webhook_url.clone(), http),
        )),
        #[cfg(feature = "channel-smtp")]
        ChannelConfig::Smtp {
            host,
            port,
            username,
            password,
            from,
            to,
        } => Ok(Box::new(channels::smtp::SmtpChannel::new(
            host.clone(),
            *port,
            username.clone(),
            password.clone(),
            from.clone(),
            to.clone(),
        ))),
        #[allow(unreachable_patterns)]
        _ => Err(NotifyError::ConfigParse(format!(
            "channel '{}' is not enabled; enable the corresponding feature flag",
            match config {
                ChannelConfig::DingTalk { .. } => "dingtalk",
                ChannelConfig::Feishu { .. } => "feishu",
                ChannelConfig::Telegram { .. } => "telegram",
                ChannelConfig::Discord { .. } => "discord",
                ChannelConfig::Smtp { .. } => "smtp",
            }
        ))),
    }
}

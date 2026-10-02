//! 通知聚合协议的 Rust core SDK。
//!
//! 只发不收，跨平台。统一消息模型 + 渠道适配器 trait，
//! 首发支持钉钉、飞书、Telegram、Discord、SMTP、Server酱、Bark、Gotify、
//! Qmsg、WxPusher 与自定义 Webhook。

pub mod capability;
pub mod channel;
pub mod channels;
pub mod config;
#[cfg(feature = "default-client")]
pub mod engine;
pub mod http;
pub mod message;

pub use capability::Capabilities;
pub use channel::{Channel, ErrorKind, NotifyError, ResponseBody, SendReceipt};
pub use config::{ChannelConfig, WebhookHeader};
#[cfg(feature = "default-client")]
pub use engine::Engine;
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
        feature = "channel-serverchan",
        feature = "channel-bark",
        feature = "channel-gotify",
        feature = "channel-qmsg",
        feature = "channel-wxpusher",
        feature = "channel-webhook",
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
        #[cfg(feature = "channel-discord")]
        ChannelConfig::DiscordBot { bot_token, user_id } => Ok(Box::new(
            channels::discord::DiscordBotChannel::new(bot_token.clone(), user_id.clone(), http),
        )),
        #[cfg(feature = "channel-serverchan")]
        ChannelConfig::ServerChan { send_key } => Ok(Box::new(
            channels::serverchan::ServerChanChannel::new(send_key.clone(), http),
        )),
        #[cfg(feature = "channel-bark")]
        ChannelConfig::Bark {
            server_url,
            device_key,
        } => Ok(Box::new(channels::bark::BarkChannel::new(
            server_url.clone(),
            device_key.clone(),
            http,
        ))),
        #[cfg(feature = "channel-gotify")]
        ChannelConfig::Gotify { server_url, token } => Ok(Box::new(
            channels::gotify::GotifyChannel::new(server_url.clone(), token.clone(), http),
        )),
        #[cfg(feature = "channel-qmsg")]
        ChannelConfig::Qmsg {
            server_url,
            key,
            user_id,
            bot_id,
        } => Ok(Box::new(channels::qmsg::QmsgChannel::new(
            server_url.clone(),
            key.clone(),
            user_id.clone(),
            bot_id.clone(),
            http,
        ))),
        #[cfg(feature = "channel-wxpusher")]
        ChannelConfig::WxPusher { app_token, uid } => Ok(Box::new(
            channels::wxpusher::WxPusherChannel::new(app_token.clone(), uid.clone(), http),
        )),
        #[cfg(feature = "channel-webhook")]
        ChannelConfig::CustomWebhook {
            url,
            headers,
            body_template,
            content_type,
        } => Ok(Box::new(channels::webhook::CustomWebhookChannel::new(
            url.clone(),
            headers.clone(),
            body_template.clone(),
            content_type.clone(),
            http,
        ))),
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
                ChannelConfig::DiscordBot { .. } => "discord-bot",
                ChannelConfig::ServerChan { .. } => "serverchan",
                ChannelConfig::Bark { .. } => "bark",
                ChannelConfig::Gotify { .. } => "gotify",
                ChannelConfig::Qmsg { .. } => "qmsg",
                ChannelConfig::WxPusher { .. } => "wxpusher",
                ChannelConfig::CustomWebhook { .. } => "webhook",
                ChannelConfig::Smtp { .. } => "smtp",
            }
        ))),
    }
}

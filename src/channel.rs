use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::capability::Capabilities;
use crate::message::Message;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SendReceipt {
    pub channel: String,
    pub message_id: Option<String>,
    pub raw_response: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotifyError {
    ConfigParse(String),
    ChannelAuth(String),
    Network(String),
    MessageConversion(String),
    Channel(String),
}

impl std::fmt::Display for NotifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConfigParse(msg) => write!(f, "config parse error: {msg}"),
            Self::ChannelAuth(msg) => write!(f, "channel auth error: {msg}"),
            Self::Network(msg) => write!(f, "network error: {msg}"),
            Self::MessageConversion(msg) => write!(f, "message conversion error: {msg}"),
            Self::Channel(msg) => write!(f, "channel error: {msg}"),
        }
    }
}

impl std::error::Error for NotifyError {}

#[async_trait]
pub trait Channel: Send + Sync {
    fn name(&self) -> &'static str;
    fn capabilities(&self) -> &Capabilities;
    async fn send(&self, message: &Message) -> Result<SendReceipt, NotifyError>;
}

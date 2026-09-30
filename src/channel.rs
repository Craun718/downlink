use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::capability::Capabilities;
use crate::message::Message;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SendReceipt {
    pub channel: String,
    pub message_id: Option<String>,
    pub raw_response: Option<String>,
}

/// A bounded copy of a response body suitable for diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResponseBody {
    pub preview: String,
    pub truncated: bool,
}

impl ResponseBody {
    pub const MAX_PREVIEW_BYTES: usize = 4096;

    pub fn new(body: &str) -> Self {
        let truncated = body.len() > Self::MAX_PREVIEW_BYTES;
        let mut end = body.len().min(Self::MAX_PREVIEW_BYTES);
        while end > 0 && !body.is_char_boundary(end) {
            end -= 1;
        }
        Self {
            preview: body[..end].to_owned(),
            truncated,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    Config,
    Transport,
    Http,
    Authentication,
    Authorization,
    InvalidRequest,
    RateLimited,
    RemoteServer,
    ResponseDecode,
    Provider,
    UnsupportedCapability,
    MessageConversion,
    Channel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotifyError {
    ConfigParse(String),
    ChannelAuth(String),
    Network(String),
    MessageConversion(String),
    Channel(String),
    UnsupportedCapability(String),
    HttpStatus {
        channel: String,
        status: u16,
        body: ResponseBody,
        retry_after: Option<Duration>,
    },
    ResponseDecode {
        channel: String,
        status: u16,
        message: String,
        body: ResponseBody,
    },
    Provider {
        channel: String,
        http_status: u16,
        code: Option<String>,
        message: String,
        retry_after: Option<Duration>,
    },
}

impl NotifyError {
    pub fn kind(&self) -> ErrorKind {
        match self {
            Self::ConfigParse(_) => ErrorKind::Config,
            Self::Network(_) => ErrorKind::Transport,
            Self::ChannelAuth(_) => ErrorKind::Authentication,
            Self::MessageConversion(_) => ErrorKind::MessageConversion,
            Self::UnsupportedCapability(_) => ErrorKind::UnsupportedCapability,
            Self::Channel(_) => ErrorKind::Channel,
            Self::ResponseDecode { .. } => ErrorKind::ResponseDecode,
            Self::HttpStatus { status, .. } => http_error_kind(*status),
            Self::Provider {
                http_status,
                retry_after,
                ..
            } if retry_after.is_some() => ErrorKind::RateLimited,
            Self::Provider { http_status, .. } if (200..300).contains(http_status) => {
                ErrorKind::Provider
            }
            Self::Provider { http_status, .. } => http_error_kind(*http_status),
        }
    }

    pub fn channel(&self) -> Option<&str> {
        match self {
            Self::HttpStatus { channel, .. }
            | Self::ResponseDecode { channel, .. }
            | Self::Provider { channel, .. } => Some(channel),
            _ => None,
        }
    }

    pub fn http_status(&self) -> Option<u16> {
        match self {
            Self::HttpStatus { status, .. } | Self::ResponseDecode { status, .. } => Some(*status),
            Self::Provider { http_status, .. } => Some(*http_status),
            _ => None,
        }
    }

    pub fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::HttpStatus { retry_after, .. } | Self::Provider { retry_after, .. } => {
                *retry_after
            }
            _ => None,
        }
    }

    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Network(_) => true,
            Self::HttpStatus { status, .. } => is_retryable_http_status(*status),
            Self::Provider {
                http_status,
                retry_after,
                ..
            } => retry_after.is_some() || is_retryable_http_status(*http_status),
            _ => false,
        }
    }
}

fn http_error_kind(status: u16) -> ErrorKind {
    match status {
        401 => ErrorKind::Authentication,
        403 => ErrorKind::Authorization,
        408 | 425 | 429 => ErrorKind::RateLimited,
        400..=499 => ErrorKind::InvalidRequest,
        500..=599 => ErrorKind::RemoteServer,
        _ => ErrorKind::Http,
    }
}

fn is_retryable_http_status(status: u16) -> bool {
    matches!(status, 408 | 425 | 429 | 500 | 502 | 503 | 504)
}

impl std::fmt::Display for NotifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConfigParse(msg) => write!(f, "config parse error: {msg}"),
            Self::ChannelAuth(msg) => write!(f, "channel auth error: {msg}"),
            Self::Network(msg) => write!(f, "network error: {msg}"),
            Self::MessageConversion(msg) => write!(f, "message conversion error: {msg}"),
            Self::Channel(msg) => write!(f, "channel error: {msg}"),
            Self::UnsupportedCapability(msg) => write!(f, "unsupported capability: {msg}"),
            Self::HttpStatus {
                channel,
                status,
                body,
                ..
            } => write!(f, "{channel} returned HTTP {status}: {}", body.preview),
            Self::ResponseDecode {
                channel,
                status,
                message,
                ..
            } => write!(
                f,
                "{channel} returned invalid response (HTTP {status}): {message}"
            ),
            Self::Provider {
                channel,
                code,
                message,
                ..
            } => match code {
                Some(code) => write!(f, "{channel} error {code}: {message}"),
                None => write!(f, "{channel} error: {message}"),
            },
        }
    }
}

impl std::error::Error for NotifyError {}

#[async_trait]
pub trait Channel: Send + Sync {
    fn name(&self) -> &'static str;
    fn capabilities(&self) -> &Capabilities;
    async fn send(&self, message: &Message) -> Result<SendReceipt, NotifyError>;

    /// Negotiate the message against this channel's capabilities, then send.
    async fn send_with_fallback(&self, message: &Message) -> Result<SendReceipt, NotifyError> {
        let message = self.capabilities().try_degrade(message)?;
        self.send(&message).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_body_truncates_on_utf8_boundary() {
        let body = "中".repeat(ResponseBody::MAX_PREVIEW_BYTES);
        let response = ResponseBody::new(&body);
        assert!(response.truncated);
        assert!(response.preview.len() <= ResponseBody::MAX_PREVIEW_BYTES);
        assert!(response.preview.is_char_boundary(response.preview.len()));
    }

    #[test]
    fn retry_policy_distinguishes_client_and_server_errors() {
        let client_error = NotifyError::HttpStatus {
            channel: "discord".into(),
            status: 400,
            body: ResponseBody::new("bad request"),
            retry_after: None,
        };
        let server_error = NotifyError::HttpStatus {
            channel: "discord".into(),
            status: 503,
            body: ResponseBody::new("unavailable"),
            retry_after: None,
        };

        assert_eq!(client_error.kind(), ErrorKind::InvalidRequest);
        assert!(!client_error.is_retryable());
        assert_eq!(server_error.kind(), ErrorKind::RemoteServer);
        assert!(server_error.is_retryable());
    }
}

use crate::channel::{Channel, NotifyError, SendReceipt};
use crate::config::ChannelConfig;
use crate::http::default::ReqwestClient;
use crate::message::Message;

/// A synchronous facade over the async channel runtime.
///
/// `Engine` is intended for hosts that do not manage a Tokio runtime, such as
/// desktop and mobile language bindings. It owns both the runtime used to
/// execute channel operations and the default HTTP client used by the channel.
#[cfg(feature = "default-client")]
pub struct Engine {
    runtime: tokio::runtime::Runtime,
    channel: Box<dyn Channel>,
}

#[cfg(feature = "default-client")]
impl Engine {
    /// Creates an engine for a channel using the default HTTP client.
    pub fn new(config: &ChannelConfig) -> Result<Self, NotifyError> {
        let channel = crate::channel_from_config(config, Box::new(ReqwestClient::new()))?;
        Self::from_channel(channel)
    }

    /// Creates an engine from the JSON form of a channel configuration.
    ///
    /// This is the constructor intended for language bindings. It keeps the
    /// binding boundary to JSON input, JSON output, and [`NotifyError`].
    pub fn from_config_json(config_json: &str) -> Result<Self, NotifyError> {
        let config = ChannelConfig::from_json(config_json)?;
        Self::new(&config)
    }

    fn from_channel(channel: Box<dyn Channel>) -> Result<Self, NotifyError> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|error| {
                NotifyError::Channel(format!("failed to start the Tokio runtime: {error}"))
            })?;

        Ok(Self { runtime, channel })
    }

    /// Sends a message using the engine's channel.
    ///
    /// This is a blocking API and must not be called from inside an async
    /// runtime context. Hosts already using Tokio should call
    /// [`Channel::send_with_fallback`] directly instead.
    pub fn send(&self, message: &Message) -> Result<SendReceipt, NotifyError> {
        self.runtime
            .block_on(self.channel.send_with_fallback(message))
    }

    /// Sends a JSON-encoded message and returns its receipt as JSON.
    ///
    /// Language bindings should use this method instead of exposing Rust
    /// channel trait objects, futures, or Tokio runtime types.
    pub fn send_json(&self, message_json: &str) -> Result<String, NotifyError> {
        let message = serde_json::from_str(message_json)
            .map_err(|error| NotifyError::ConfigParse(format!("invalid JSON message: {error}")))?;
        let receipt = self.send(&message)?;

        serde_json::to_string(&receipt).map_err(|error| {
            NotifyError::MessageConversion(format!("serialize send receipt: {error}"))
        })
    }
}

#[cfg(all(test, feature = "default-client"))]
mod tests {
    use super::*;
    use crate::capability::Capabilities;
    use async_trait::async_trait;

    struct TextChannel;

    #[async_trait]
    impl Channel for TextChannel {
        fn name(&self) -> &'static str {
            "text"
        }

        fn capabilities(&self) -> &Capabilities {
            static CAPABILITIES: Capabilities = Capabilities::new();
            &CAPABILITIES
        }

        async fn send(&self, message: &Message) -> Result<SendReceipt, NotifyError> {
            Ok(SendReceipt {
                channel: self.name().to_owned(),
                message_id: None,
                raw_response: Some(message.as_text()),
            })
        }
    }

    #[test]
    fn engine_executes_channel_and_applies_fallback() {
        let engine = Engine::from_channel(Box::new(TextChannel))
            .expect("test channel must construct an engine");

        let receipt = engine
            .send(&Message::card("Alert", "plain **body**"))
            .expect("engine send must succeed");

        assert_eq!(receipt.channel, "text");
        assert_eq!(receipt.raw_response.as_deref(), Some("plain body"));
    }

    #[test]
    fn json_boundary_degrades_and_returns_json_receipt() {
        let engine = Engine::from_channel(Box::new(TextChannel))
            .expect("test channel must construct an engine");

        let receipt = engine
            .send_json(
                r#"{
                    "body": {
                        "type": "card",
                        "title": "Alert",
                        "markdown": "plain **body**"
                    }
                }"#,
            )
            .expect("JSON send must succeed");

        let receipt: serde_json::Value = serde_json::from_str(&receipt).unwrap();
        assert_eq!(receipt["channel"], "text");
        assert_eq!(receipt["raw_response"], "plain body");
    }

    #[test]
    fn invalid_message_json_is_a_configuration_error() {
        let engine = Engine::from_channel(Box::new(TextChannel))
            .expect("test channel must construct an engine");

        let error = engine
            .send_json("{")
            .expect_err("malformed JSON must be rejected");

        assert!(matches!(error, NotifyError::ConfigParse(_)));
    }
}

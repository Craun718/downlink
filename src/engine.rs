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
}

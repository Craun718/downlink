use async_trait::async_trait;

use crate::capability::Capabilities;
use crate::channel::{Channel, NotifyError, SendReceipt};
use crate::message::Message;

pub struct SmtpChannel {
    #[allow(dead_code)]
    host: String,
    #[allow(dead_code)]
    port: u16,
    #[allow(dead_code)]
    username: String,
    #[allow(dead_code)]
    password: String,
    #[allow(dead_code)]
    from: String,
    #[allow(dead_code)]
    to: Vec<String>,
}

impl SmtpChannel {
    pub fn new(
        host: String,
        port: u16,
        username: String,
        password: String,
        from: String,
        to: Vec<String>,
    ) -> Self {
        Self { host, port, username, password, from, to }
    }
}

#[async_trait]
impl Channel for SmtpChannel {
    fn name(&self) -> &'static str {
        "smtp"
    }

    fn capabilities(&self) -> &Capabilities {
        &Self::CAPABILITIES
    }

    async fn send(&self, _message: &Message) -> Result<SendReceipt, NotifyError> {
        Err(NotifyError::Channel(
            "smtp adapter is not yet implemented; enable a future `channel-smtp` release".into(),
        ))
    }
}

impl SmtpChannel {
    const CAPABILITIES: Capabilities = Capabilities::new()
        .with_markdown()
        .with_card();
}

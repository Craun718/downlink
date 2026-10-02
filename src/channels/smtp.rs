use async_trait::async_trait;
use lettre::{
    AsyncSmtpTransport, AsyncTransport, Tokio1Executor,
    message::{Mailbox, Message as Email, SinglePart, header::ContentType},
    transport::smtp::authentication::Credentials,
};

use crate::capability::Capabilities;
use crate::channel::{Channel, NotifyError, SendReceipt};
use crate::message::{Message, MessageBody};
use markdown_plain_text::markdown_to_plain_text;

pub struct SmtpChannel {
    host: String,
    port: u16,
    username: String,
    password: String,
    from: String,
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
        Self {
            host,
            port,
            username,
            password,
            from,
            to,
        }
    }

    fn subject_for(&self, message: &Message) -> String {
        match &message.body {
            MessageBody::Card { title, .. } => title.clone(),
            MessageBody::Markdown {
                title: Some(title), ..
            } => title.clone(),
            _ => match message.priority {
                crate::message::Priority::Critical => "[CRITICAL] Notification".into(),
                crate::message::Priority::High => "[HIGH] Notification".into(),
                _ => "Notification".into(),
            },
        }
    }

    fn body_for(&self, message: &Message) -> String {
        let text = match &message.body {
            MessageBody::Text { text } => text.clone(),
            MessageBody::Markdown { text, .. } | MessageBody::Card { markdown: text, .. } => {
                markdown_to_plain_text(text)
            }
        };
        if message.mentions.is_empty() {
            return text;
        }
        let mentions: String = message
            .mentions
            .iter()
            .map(|m| {
                if m.is_mobile {
                    format!("☎ {}\n", m.id)
                } else {
                    format!("👤 @{}\n", m.id)
                }
            })
            .collect();
        format!("{mentions}\n{text}")
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

    async fn send(&self, message: &Message) -> Result<SendReceipt, NotifyError> {
        let from_mailbox: Mailbox = self
            .from
            .parse()
            .map_err(|e| NotifyError::ChannelAuth(format!("invalid from address: {e}")))?;

        let mut builder = Email::builder()
            .from(from_mailbox)
            .subject(self.subject_for(message));
        for to_addr in &self.to {
            let mailbox: Mailbox = to_addr.parse().map_err(|e| {
                NotifyError::MessageConversion(format!("invalid to address '{to_addr}': {e}"))
            })?;
            builder = builder.to(mailbox);
        }

        let email = builder
            .header(ContentType::TEXT_PLAIN)
            .singlepart(SinglePart::plain(self.body_for(message)))
            .map_err(|e| NotifyError::MessageConversion(format!("failed to build email: {e}")))?;

        let transport = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&self.host)
            .map_err(|e| NotifyError::ChannelAuth(format!("SMTP relay error: {e}")))?
            .port(self.port)
            .credentials(Credentials::new(
                self.username.clone(),
                self.password.clone(),
            ))
            .build();

        let response = transport
            .send(email)
            .await
            .map_err(|e| NotifyError::Network(format!("SMTP send failed: {e}")))?;

        Ok(SendReceipt {
            channel: Self::CHANNEL_NAME.into(),
            message_id: Some(response.message().collect::<Vec<_>>().join(" ")),
            raw_response: Some(format!("{response:?}")),
        })
    }
}

impl SmtpChannel {
    const CHANNEL_NAME: &'static str = "smtp";
    const CAPABILITIES: Capabilities = Capabilities::new()
        .with_markdown()
        .with_card()
        .with_mentions(false, true, true);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_body_is_converted_to_plain_text() {
        let channel = SmtpChannel::new(
            "smtp.example.com".into(),
            587,
            "user".into(),
            "password".into(),
            "from@example.com".into(),
            vec!["to@example.com".into()],
        );
        let message = Message::markdown(
            "[**Service down**](https://example.com)",
            Some("Alert".into()),
        );

        assert_eq!(
            channel.body_for(&message),
            "Service down (https://example.com)"
        );
    }
}

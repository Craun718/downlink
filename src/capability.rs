use crate::channel::NotifyError;
use crate::message::{Message, MessageBody};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capabilities {
    pub text: bool,
    pub markdown: bool,
    pub card: bool,
    pub mention_all: bool,
    pub mention_user: bool,
    pub mention_mobile: bool,
}

impl Default for Capabilities {
    fn default() -> Self {
        Self::new()
    }
}

impl Capabilities {
    pub const fn new() -> Self {
        Self {
            text: true,
            markdown: false,
            card: false,
            mention_all: false,
            mention_user: false,
            mention_mobile: false,
        }
    }

    pub const fn with_markdown(mut self) -> Self {
        self.markdown = true;
        self
    }

    pub const fn with_card(mut self) -> Self {
        self.card = true;
        self
    }

    pub const fn with_mentions(mut self, all: bool, user: bool, mobile: bool) -> Self {
        self.mention_all = all;
        self.mention_user = user;
        self.mention_mobile = mobile;
        self
    }

    pub fn supports(&self, body: &MessageBody) -> bool {
        match body {
            MessageBody::Text { .. } => self.text,
            MessageBody::Markdown { .. } => self.markdown,
            MessageBody::Card { .. } => self.card,
        }
    }

    /// Returns whether the channel can render every part of a message
    /// without losing information.
    pub fn supports_message(&self, message: &Message) -> bool {
        self.supports(&message.body)
            && message.mentions.iter().all(|mention| {
                if mention.is_mobile {
                    self.mention_mobile
                } else {
                    self.mention_user
                }
            })
    }

    /// Converts a message to the best representation supported by this
    /// channel. Unsupported mentions are removed rather than rendered as
    /// misleading plain text mentions.
    pub fn try_degrade(&self, message: &Message) -> Result<Message, NotifyError> {
        let body = if self.supports(&message.body) {
            message.body.clone()
        } else {
            match &message.body {
                MessageBody::Card { title, markdown } if self.markdown => MessageBody::Markdown {
                    text: markdown.clone(),
                    title: Some(title.clone()),
                },
                MessageBody::Card { markdown, .. }
                | MessageBody::Markdown { text: markdown, .. }
                    if self.text =>
                {
                    MessageBody::Text {
                        text: markdown.clone(),
                    }
                }
                MessageBody::Text { .. } if self.text => message.body.clone(),
                _ => {
                    return Err(NotifyError::UnsupportedCapability(
                        "channel cannot render the message body".into(),
                    ));
                }
            }
        };

        let mentions = message
            .mentions
            .iter()
            .filter(|mention| {
                if mention.is_mobile {
                    self.mention_mobile
                } else {
                    self.mention_user
                }
            })
            .cloned()
            .collect();

        Ok(Message {
            body,
            priority: message.priority,
            mentions,
        })
    }

    pub fn degrade(&self, message: &Message) -> Message {
        self.try_degrade(message)
            .unwrap_or_else(|_| message.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::{Mention, Priority};

    #[test]
    fn card_degrades_to_markdown_then_text() {
        let message = Message::card("Alert", "**details**").with_priority(Priority::High);
        let markdown = Capabilities::new().with_markdown();
        let degraded = markdown.try_degrade(&message).unwrap();
        assert_eq!(
            degraded.body,
            MessageBody::Markdown {
                text: "**details**".into(),
                title: Some("Alert".into())
            }
        );
        assert_eq!(degraded.priority, Priority::High);

        let plain = Capabilities::new();
        assert_eq!(
            plain.try_degrade(&message).unwrap().body,
            MessageBody::Text {
                text: "**details**".into()
            }
        );
    }

    #[test]
    fn unsupported_body_returns_an_error_when_no_fallback_exists() {
        let capabilities = Capabilities {
            text: false,
            ..Capabilities::new()
        };
        let error = capabilities
            .try_degrade(&Message::text("hello"))
            .unwrap_err();
        assert_eq!(
            error,
            NotifyError::UnsupportedCapability("channel cannot render the message body".into())
        );
    }

    #[test]
    fn unsupported_mentions_are_removed_and_supported_mentions_are_preserved() {
        let capabilities = Capabilities::new().with_mentions(false, true, false);
        let message = Message::text("hello").with_mentions(vec![
            Mention {
                id: "user-1".into(),
                is_mobile: false,
            },
            Mention {
                id: "13800000000".into(),
                is_mobile: true,
            },
        ]);

        let degraded = capabilities.try_degrade(&message).unwrap();
        assert_eq!(
            degraded.mentions,
            vec![Mention {
                id: "user-1".into(),
                is_mobile: false
            }]
        );
        assert!(!capabilities.supports_message(&message));
        assert!(capabilities.supports_message(&degraded));
    }
}

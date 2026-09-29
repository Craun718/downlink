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

    pub fn degrade(&self, message: &Message) -> Message {
        if self.supports(&message.body) {
            return message.clone();
        }
        let body = match &message.body {
            MessageBody::Card { title, markdown } if self.markdown => {
                MessageBody::Markdown { text: markdown.clone(), title: Some(title.clone()) }
            }
            MessageBody::Card { markdown, .. } | MessageBody::Markdown { text: markdown, .. }
                if self.text =>
            {
                MessageBody::Text { text: markdown.clone() }
            }
            _ => message.body.clone(),
        };
        Message {
            body,
            priority: message.priority,
            mentions: message.mentions.clone(),
        }
    }
}

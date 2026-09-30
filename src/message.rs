use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessageBody {
    Text { text: String },
    Markdown { text: String, title: Option<String> },
    Card { title: String, markdown: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    Low,
    #[default]
    Normal,
    High,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mention {
    pub id: String,
    pub is_mobile: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub body: MessageBody,
    #[serde(default)]
    pub priority: Priority,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mentions: Vec<Mention>,
}

impl Message {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            body: MessageBody::Text { text: text.into() },
            priority: Priority::Normal,
            mentions: Vec::new(),
        }
    }

    pub fn markdown(text: impl Into<String>, title: Option<String>) -> Self {
        Self {
            body: MessageBody::Markdown {
                text: text.into(),
                title,
            },
            priority: Priority::Normal,
            mentions: Vec::new(),
        }
    }

    pub fn card(title: impl Into<String>, markdown: impl Into<String>) -> Self {
        Self {
            body: MessageBody::Card {
                title: title.into(),
                markdown: markdown.into(),
            },
            priority: Priority::Normal,
            mentions: Vec::new(),
        }
    }

    pub fn with_priority(mut self, priority: Priority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_mentions(mut self, mentions: Vec<Mention>) -> Self {
        self.mentions = mentions;
        self
    }

    pub fn as_text(&self) -> String {
        match &self.body {
            MessageBody::Text { text } => text.clone(),
            MessageBody::Markdown { text, .. } | MessageBody::Card { markdown: text, .. } => {
                text.clone()
            }
        }
    }
}

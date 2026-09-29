#[cfg(feature = "channel-dingtalk")]
pub mod dingtalk;

#[cfg(feature = "channel-discord")]
pub mod discord;

#[cfg(feature = "channel-feishu")]
pub mod feishu;

#[cfg(feature = "channel-smtp")]
pub mod smtp;

#[cfg(feature = "channel-telegram")]
pub mod telegram;

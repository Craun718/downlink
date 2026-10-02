mod common;

#[cfg(feature = "default-client")]
mod real {
    use super::common::TestSecrets;
    use downlink::{ChannelConfig, Message, ReqwestClient, channel_from_config};

    async fn send_via(config: ChannelConfig, label: &str) {
        let channel = channel_from_config(&config, Box::new(ReqwestClient::new()))
            .expect("channel config must construct a channel");
        let message = Message::text(format!("downlink real test via {label}"));
        let receipt = channel
            .send_with_fallback(&message)
            .await
            .unwrap_or_else(|error| panic!("{label} real send failed: {error:?}"));
        println!("{label} receipt: {receipt:?}");
    }

    fn load_secrets(label: &str) -> Option<TestSecrets> {
        TestSecrets::load().or_else(|| {
            eprintln!(
                "skipped {label}: set NOTIFY_TEST_SECRETS_JSON or create tests/fixtures/secrets.json (see tests/fixtures/secrets.example.json)"
            );
            None
        })
    }

    fn config_for(secrets: TestSecrets, label: &str) -> Option<ChannelConfig> {
        let config = match label {
            "dingtalk" => secrets.dingtalk.map(|s| s.to_config()),
            "feishu" => secrets.feishu.map(|s| s.to_config()),
            "telegram" => secrets.telegram.map(|s| s.to_config()),
            "discord" => secrets.discord.map(|s| s.to_config()),
            _ => None,
        };
        if config.is_none() {
            eprintln!("skipped {label}: no {label} credential in secrets");
        }
        config
    }

    #[tokio::test]
    #[ignore = "hits the real DingTalk API; run with --ignored"]
    async fn real_dingtalk_sends_message() {
        let Some(secrets) = load_secrets("dingtalk") else {
            return;
        };
        let Some(config) = config_for(secrets, "dingtalk") else {
            return;
        };
        send_via(config, "dingtalk").await;
    }

    #[tokio::test]
    #[ignore = "hits the real Feishu API; run with --ignored"]
    async fn real_feishu_sends_message() {
        let Some(secrets) = load_secrets("feishu") else {
            return;
        };
        let Some(config) = config_for(secrets, "feishu") else {
            return;
        };
        send_via(config, "feishu").await;
    }

    #[tokio::test]
    #[ignore = "hits the real Telegram API; run with --ignored"]
    async fn real_telegram_sends_message() {
        let Some(secrets) = load_secrets("telegram") else {
            return;
        };
        let Some(config) = config_for(secrets, "telegram") else {
            return;
        };
        send_via(config, "telegram").await;
    }

    #[tokio::test]
    #[ignore = "hits the real Discord API; run with --ignored"]
    async fn real_discord_sends_message() {
        let Some(secrets) = load_secrets("discord") else {
            return;
        };
        let Some(config) = config_for(secrets, "discord") else {
            return;
        };
        send_via(config, "discord").await;
    }

    #[cfg(feature = "channel-smtp")]
    #[tokio::test]
    #[ignore = "sends a real email; run with --ignored and --features channel-smtp"]
    async fn real_smtp_sends_email() {
        let Some(secrets) = TestSecrets::load() else {
            eprintln!("skipped smtp: no secrets configured");
            return;
        };
        let Some(smtp) = secrets.smtp else {
            eprintln!("skipped smtp: no smtp secret");
            return;
        };
        let config = smtp.to_config();
        send_via(config, "smtp").await;
    }
}

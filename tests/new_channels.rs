mod common;

use common::{MockHttpClient, ok_response};
use downlink::channels::bark::BarkChannel;
use downlink::channels::discord::DiscordBotChannel;
use downlink::channels::gotify::GotifyChannel;
use downlink::channels::qmsg::QmsgChannel;
use downlink::channels::serverchan::ServerChanChannel;
use downlink::channels::webhook::CustomWebhookChannel;
use downlink::channels::wxpusher::WxPusherChannel;
use downlink::config::WebhookHeader;
use downlink::{
    Channel, ChannelConfig, HttpRequest, Mention, Message, NotifyError, Priority,
    channel_from_config,
};
use serde_json::{Value, json};

fn json_body(request: &HttpRequest) -> Value {
    request
        .body
        .clone()
        .expect("request must carry a JSON body")
}

fn header<'a>(request: &'a HttpRequest, name: &str) -> Option<&'a str> {
    request
        .headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

#[tokio::test]
async fn serverchan_sends_encoded_title_and_markdown() {
    let mock = MockHttpClient::new(ok_response(r#"{"code":0,"data":{"pushid":"42"}}"#));
    let channel = ServerChanChannel::new("sct-key".into(), Box::new(mock.clone()));
    let title = "T".repeat(40);

    let receipt = channel
        .send(&Message::markdown("hello **world**", Some(title)))
        .await
        .unwrap();

    let request = &mock.requests()[0];
    assert_eq!(
        request.url,
        format!(
            "https://sctapi.ftqq.com/sct-key.send?title={}&desp=hello%20%2A%2Aworld%2A%2A",
            "T".repeat(32)
        )
    );
    assert_eq!(receipt.channel, "serverchan");
    assert_eq!(receipt.message_id.as_deref(), Some("42"));
}

#[tokio::test]
async fn serverchan_new_send_key_uses_ft07_endpoint() {
    let mock = MockHttpClient::new(ok_response(r#"{"code":0}"#));
    let channel = ServerChanChannel::new("sctp123456tabcdef".into(), Box::new(mock.clone()));

    channel.send(&Message::text("hello")).await.unwrap();

    assert!(mock.requests()[0].url.starts_with(
        "https://123456.push.ft07.com/send/sctp123456tabcdef.send?title=Notification"
    ));
}

#[tokio::test]
async fn serverchan_maps_provider_error() {
    let mock = MockHttpClient::new(ok_response(r#"{"code":40001,"message":"bad key"}"#));
    let channel = ServerChanChannel::new("bad-key".into(), Box::new(mock));

    let error = channel.send(&Message::text("hello")).await.unwrap_err();

    match error {
        NotifyError::Provider {
            channel,
            code,
            message,
            ..
        } => {
            assert_eq!(channel, "serverchan");
            assert_eq!(code.as_deref(), Some("40001"));
            assert_eq!(message, "bad key");
        }
        other => panic!("expected provider error, got {other:?}"),
    }
}

#[tokio::test]
async fn bark_posts_json_payload() {
    let mock = MockHttpClient::new(ok_response(r#"{"code":200,"data":{"task_id":"42"}}"#));
    let channel = BarkChannel::new(
        "https://bark.example.com".into(),
        "device-key".into(),
        Box::new(mock.clone()),
    );

    let receipt = channel
        .send(&Message::card("Alert", "details"))
        .await
        .unwrap();

    let request = &mock.requests()[0];
    assert_eq!(request.url, "https://bark.example.com/push");
    let payload = json_body(request);
    assert_eq!(payload["title"], "Alert");
    assert_eq!(payload["body"], "details");
    assert_eq!(payload["device_key"], "device-key");
    assert_eq!(payload["group"], "downlink");
    assert_eq!(receipt.message_id.as_deref(), Some("42"));
}

#[tokio::test]
async fn bark_requires_provider_success_code() {
    let mock = MockHttpClient::new(ok_response(r#"{"code":400,"message":"bad device"}"#));
    let channel = BarkChannel::new(
        "https://bark.example.com".into(),
        "bad-key".into(),
        Box::new(mock),
    );

    let error = channel.send(&Message::text("hello")).await.unwrap_err();

    match error {
        NotifyError::Provider {
            channel,
            code,
            message,
            ..
        } => {
            assert_eq!(channel, "bark");
            assert_eq!(code.as_deref(), Some("400"));
            assert_eq!(message, "bad device");
        }
        other => panic!("expected provider error, got {other:?}"),
    }
}

#[tokio::test]
async fn gotify_posts_priority_and_token_header() {
    let mock = MockHttpClient::new(ok_response(r#"{"id":42}"#));
    let channel = GotifyChannel::new(
        "https://gotify.example.com/base".into(),
        "app-token".into(),
        Box::new(mock.clone()),
    );

    let receipt = channel
        .send(&Message::text("hello").with_priority(Priority::High))
        .await
        .unwrap();

    let request = &mock.requests()[0];
    assert_eq!(request.url, "https://gotify.example.com/base/message");
    assert_eq!(header(request, "X-Gotify-Key"), Some("app-token"));
    let payload = json_body(request);
    assert_eq!(payload["title"], "[HIGH] Notification");
    assert_eq!(payload["message"], "hello");
    assert_eq!(payload["priority"], 8);
    assert_eq!(receipt.message_id.as_deref(), Some("42"));
}

#[tokio::test]
async fn gotify_requires_message_id() {
    let mock = MockHttpClient::new(ok_response(
        r#"{"error":401,"errorDescription":"unauthorized"}"#,
    ));
    let channel = GotifyChannel::new(
        "https://gotify.example.com".into(),
        "bad-token".into(),
        Box::new(mock),
    );

    let error = channel.send(&Message::text("hello")).await.unwrap_err();

    match error {
        NotifyError::Provider {
            channel,
            code,
            message,
            ..
        } => {
            assert_eq!(channel, "gotify");
            assert_eq!(code.as_deref(), Some("401"));
            assert_eq!(message, "unauthorized");
        }
        other => panic!("expected provider error, got {other:?}"),
    }
}

#[tokio::test]
async fn qmsg_posts_message_and_optional_bot() {
    let mock = MockHttpClient::new(ok_response(r#"{"success":true,"result":42}"#));
    let channel = QmsgChannel::new(
        "https://qmsg.example.com".into(),
        "key".into(),
        "10001".into(),
        Some("10002".into()),
        Box::new(mock.clone()),
    );

    let receipt = channel
        .send(&Message::card("Alert", "details"))
        .await
        .unwrap();

    let request = &mock.requests()[0];
    assert_eq!(request.url, "https://qmsg.example.com/jsend/key");
    let payload = json_body(request);
    assert_eq!(payload["msg"], "Alert\ndetails");
    assert_eq!(payload["qq"], "10001");
    assert_eq!(payload["bot"], "10002");
    assert_eq!(receipt.message_id, Some("42".into()));
}

#[tokio::test]
async fn qmsg_maps_business_failure() {
    let mock = MockHttpClient::new(ok_response(
        r#"{"success":false,"code":500,"message":"denied"}"#,
    ));
    let channel = QmsgChannel::new(
        "https://qmsg.example.com".into(),
        "key".into(),
        "10001".into(),
        None,
        Box::new(mock),
    );

    let error = channel.send(&Message::text("hello")).await.unwrap_err();

    match error {
        NotifyError::Provider {
            channel,
            code,
            message,
            ..
        } => {
            assert_eq!(channel, "qmsg");
            assert_eq!(code.as_deref(), Some("500"));
            assert_eq!(message, "denied");
        }
        other => panic!("expected provider error, got {other:?}"),
    }
}

#[tokio::test]
async fn wxpusher_normal_push_uses_app_token_and_uid() {
    let mock = MockHttpClient::new(ok_response(r#"{"success":true}"#));
    let channel = WxPusherChannel::new(
        Some("app-token".into()),
        "uid-1".into(),
        Box::new(mock.clone()),
    );

    channel
        .send(&Message::card("Alert", "details"))
        .await
        .unwrap();

    let request = &mock.requests()[0];
    assert_eq!(
        request.url,
        "https://wxpusher.zjiecode.com/api/send/message"
    );
    let payload = json_body(request);
    assert_eq!(payload["appToken"], "app-token");
    assert_eq!(payload["content"], "Alert\n\ndetails");
    assert_eq!(payload["summary"], "Alert\n\ndetails");
    assert_eq!(payload["uids"], json!(["uid-1"]));
}

#[tokio::test]
async fn wxpusher_simple_push_uses_spt() {
    let mock = MockHttpClient::new(ok_response(r#"{"success":true}"#));
    let channel = WxPusherChannel::new(None, "spt-token".into(), Box::new(mock.clone()));

    channel.send(&Message::text("hello")).await.unwrap();

    let request = &mock.requests()[0];
    assert_eq!(
        request.url,
        "https://wxpusher.zjiecode.com/api/send/message/simple-push"
    );
    let payload = json_body(request);
    assert_eq!(payload["content"], "hello");
    assert_eq!(payload["spt"], "spt-token");
    assert!(payload.get("appToken").is_none());
}

#[tokio::test]
async fn wxpusher_maps_provider_error() {
    let mock = MockHttpClient::new(ok_response(
        r#"{"success":false,"code":400,"msg":"bad token"}"#,
    ));
    let channel = WxPusherChannel::new(Some("bad-token".into()), "uid".into(), Box::new(mock));

    let error = channel.send(&Message::text("hello")).await.unwrap_err();

    match error {
        NotifyError::Provider {
            channel,
            code,
            message,
            ..
        } => {
            assert_eq!(channel, "wxpusher");
            assert_eq!(code.as_deref(), Some("400"));
            assert_eq!(message, "bad token");
        }
        other => panic!("expected provider error, got {other:?}"),
    }
}

#[tokio::test]
async fn custom_webhook_sends_default_json_payload() {
    let mock = MockHttpClient::new(ok_response("{}"));
    let channel = CustomWebhookChannel::new(
        "https://hooks.example.com/notify".into(),
        Vec::new(),
        None,
        None,
        Box::new(mock.clone()),
    );

    channel
        .send(&Message::card("Alert", "details").with_priority(Priority::Critical))
        .await
        .unwrap();

    let request = &mock.requests()[0];
    assert_eq!(header(request, "Content-Type"), Some("application/json"));
    let payload = json_body(request);
    assert_eq!(payload["title"], "Alert");
    assert_eq!(payload["message"], "details");
    assert_eq!(payload["priority"], "critical");
}

#[tokio::test]
async fn custom_webhook_renders_nested_template_and_custom_headers() {
    let mock = MockHttpClient::new(ok_response("{}"));
    let channel = CustomWebhookChannel::new(
        "https://hooks.example.com/notify".into(),
        vec![WebhookHeader {
            name: "X-Notify-Token".into(),
            value: "secret".into(),
        }],
        Some(r#"{"event":"notify","data":{"title":"{title}","items":["{message}",{"priority":"{priority}"}]}}"#.into()),
        Some("application/json; charset=utf-8".into()),
        Box::new(mock.clone()),
    );

    channel
        .send(&Message::text("hello").with_priority(Priority::High))
        .await
        .unwrap();

    let request = &mock.requests()[0];
    assert_eq!(header(request, "X-Notify-Token"), Some("secret"));
    assert_eq!(
        header(request, "Content-Type"),
        Some("application/json; charset=utf-8")
    );
    let payload = json_body(request);
    assert_eq!(payload["data"]["title"], "Notification");
    assert_eq!(payload["data"]["items"][0], "hello");
    assert_eq!(payload["data"]["items"][1]["priority"], "high");
}

#[tokio::test]
async fn discord_bot_opens_dm_then_posts_message() {
    let mock = MockHttpClient::new(ok_response(r#"{"id":"dm-channel"}"#));
    mock.push_response(ok_response(r#"{"id":"message"}"#));
    let channel =
        DiscordBotChannel::new("bot-token".into(), "user-1".into(), Box::new(mock.clone()));

    let receipt = channel
        .send(&Message::text("hello").with_mentions(vec![Mention {
            id: "user-1".into(),
            is_mobile: false,
        }]))
        .await
        .unwrap();

    assert_eq!(receipt.channel, "discord-bot");
    assert_eq!(receipt.message_id.as_deref(), Some("message"));
    let open_request = &mock.requests()[0];
    assert_eq!(
        open_request.url,
        "https://discord.com/api/v10/users/@me/channels"
    );
    assert_eq!(header(open_request, "Authorization"), Some("Bot bot-token"));
    assert_eq!(header(open_request, "User-Agent"), Some("DiscordBot"));
    assert_eq!(json_body(open_request)["recipient_id"], "user-1");

    let send_request = &mock.requests()[1];
    assert_eq!(
        send_request.url,
        "https://discord.com/api/v10/channels/dm-channel/messages"
    );
    assert_eq!(json_body(send_request)["content"], "<@user-1>\nhello");
}

#[test]
fn channel_from_config_selects_new_channels() {
    let cases = [
        (
            ChannelConfig::DiscordBot {
                bot_token: "bot-token".into(),
                user_id: "42".into(),
            },
            "discord-bot",
        ),
        (
            ChannelConfig::ServerChan {
                send_key: "send-key".into(),
            },
            "serverchan",
        ),
        (
            ChannelConfig::Bark {
                server_url: "https://api.day.app".into(),
                device_key: "device-key".into(),
            },
            "bark",
        ),
        (
            ChannelConfig::Gotify {
                server_url: "https://gotify.example.com".into(),
                token: "app-token".into(),
            },
            "gotify",
        ),
        (
            ChannelConfig::Qmsg {
                server_url: "https://qmsg.example.com".into(),
                key: "key".into(),
                user_id: "42".into(),
                bot_id: Some("10002".into()),
            },
            "qmsg",
        ),
        (
            ChannelConfig::WxPusher {
                app_token: Some("app-token".into()),
                uid: "uid-1".into(),
            },
            "wxpusher",
        ),
        (
            ChannelConfig::CustomWebhook {
                url: "https://hooks.example.com/notify?source=downlink".into(),
                headers: Vec::new(),
                body_template: None,
                content_type: None,
            },
            "webhook",
        ),
    ];

    for (config, expected) in cases {
        let mock = MockHttpClient::new(ok_response("{}"));
        let channel = channel_from_config(&config, Box::new(mock)).unwrap();
        assert_eq!(channel.name(), expected);
    }
}

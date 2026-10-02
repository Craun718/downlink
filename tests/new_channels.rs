mod common;

use common::{MockHttpClient, ok_response};
use notify_core::channels::bark::BarkChannel;
use notify_core::channels::discord::DiscordBotChannel;
use notify_core::channels::gotify::GotifyChannel;
use notify_core::channels::qmsg::QmsgChannel;
use notify_core::channels::serverchan::ServerChanChannel;
use notify_core::channels::webhook::CustomWebhookChannel;
use notify_core::channels::wxpusher::WxPusherChannel;
use notify_core::config::WebhookHeader;
use notify_core::{
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
    assert_eq!(payload["group"], "notify-core");
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
fn new_channel_url_schemas_parse() {
    let cases = [
        ("discord-bot://bot-token@send?user_id=42", "discord-bot"),
        ("serverchan://send-key@send", "serverchan"),
        ("bark://device-key@send", "bark"),
        ("gotify://token@gotify.example.com", "gotify"),
        ("qmsg://key@send?user_id=42&bot=10002", "qmsg"),
        ("wxpusher://uid-1?app_token=app-token", "wxpusher"),
        (
            "webhook://hooks.example.com/notify?source=notify-core",
            "webhook",
        ),
    ];

    for (input, _) in cases {
        let config = ChannelConfig::from_url(input).unwrap();
        let mock = MockHttpClient::new(ok_response("{}"));
        let channel = channel_from_config(&config, Box::new(mock)).unwrap();
        let expected = match config {
            ChannelConfig::DiscordBot { .. } => "discord-bot",
            ChannelConfig::ServerChan { .. } => "serverchan",
            ChannelConfig::Bark { .. } => "bark",
            ChannelConfig::Gotify { .. } => "gotify",
            ChannelConfig::Qmsg { .. } => "qmsg",
            ChannelConfig::WxPusher { .. } => "wxpusher",
            ChannelConfig::CustomWebhook { .. } => "webhook",
            _ => unreachable!("test input must select a new channel"),
        };
        assert_eq!(channel.name(), expected);
    }
}

#[test]
fn server_backends_and_webhook_options_parse_from_url() {
    let bark = ChannelConfig::from_url("bark://device@send?server=https%3A%2F%2Fbark.example.com")
        .unwrap();
    assert!(matches!(
        bark,
        ChannelConfig::Bark { server_url, .. } if server_url == "https://bark.example.com/"
    ));

    let gotify = ChannelConfig::from_url("gotify://token@self.example.com:8443/base").unwrap();
    assert!(matches!(
        gotify,
        ChannelConfig::Gotify { server_url, .. } if server_url == "https://self.example.com:8443/base"
    ));

    let webhook = ChannelConfig::from_url(
        "webhook://hooks.example.com/notify?source=notify-core&header.X-Token=abc&content_type=text%2Fplain",
    )
    .unwrap();
    assert!(matches!(
        &webhook,
        ChannelConfig::CustomWebhook { url, headers, content_type, .. }
            if url == "https://hooks.example.com/notify?source=notify-core"
                && headers == &vec![WebhookHeader {
                    name: "X-Token".into(),
                    value: "abc".into(),
                }]
                && content_type.as_deref() == Some("text/plain")
    ));
}

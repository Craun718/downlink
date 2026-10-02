mod common;

use base64::Engine;
use common::{MockHttpClient, ok_response, status_response};
use hmac::{Hmac, Mac};
use downlink::channels::dingtalk::DingTalkChannel;
use downlink::channels::discord::DiscordChannel;
use downlink::channels::feishu::FeishuChannel;
use downlink::channels::telegram::TelegramChannel;
use downlink::{Channel, ChannelConfig, HttpRequest, Message, NotifyError};
use serde_json::Value;
use sha2::Sha256;

const DINGTALK_WEBHOOK: &str = "https://oapi.dingtalk.com/robot/send?access_token=test-token";

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

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap();
            out.push(u8::from_str_radix(hex, 16).expect("valid hex escape"));
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).expect("percent-decoded bytes must be UTF-8")
}

fn url_query_param<'a>(url: &'a str, key: &str) -> Option<String> {
    let query = url.split_once('?')?.1;
    query.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=')?;
        (k == key).then(|| percent_decode(v))
    })
}

#[tokio::test]
async fn dingtalk_text_message_posts_text_payload() {
    let mock = MockHttpClient::new(ok_response(r#"{"errcode":0,"errmsg":"ok"}"#));
    let channel = DingTalkChannel::new(DINGTALK_WEBHOOK.into(), None, Box::new(mock.clone()));

    channel.send(&Message::text("hello")).await.unwrap();

    let requests = mock.requests();
    assert_eq!(requests.len(), 1);
    let request = &requests[0];
    assert_eq!(request.url, DINGTALK_WEBHOOK);
    assert_eq!(header(request, "Content-Type"), Some("application/json"));
    let payload = json_body(request);
    assert_eq!(payload["msgtype"], "text");
    assert_eq!(payload["text"]["content"], "hello");
}

#[tokio::test]
async fn dingtalk_secret_signs_url_with_hmac() {
    let secret = "s3cret";
    let mock = MockHttpClient::new(ok_response(r#"{"errcode":0,"errmsg":"ok"}"#));
    let channel = DingTalkChannel::new(
        DINGTALK_WEBHOOK.into(),
        Some(secret.into()),
        Box::new(mock.clone()),
    );

    channel.send(&Message::text("hello")).await.unwrap();

    let url = &mock.requests()[0].url;
    assert!(url.starts_with(DINGTALK_WEBHOOK));
    let timestamp = url_query_param(url, "timestamp").expect("timestamp param");
    let sign = url_query_param(url, "sign").expect("sign param");
    assert!(!timestamp.is_empty());

    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(format!("{timestamp}\n{secret}").as_bytes());
    let expected = base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes());
    assert_eq!(sign, expected);
}

#[tokio::test]
async fn dingtalk_provider_error_maps_errcode() {
    let mock = MockHttpClient::new(ok_response(
        r#"{"errcode":310000,"errmsg":"sign not match"}"#,
    ));
    let channel = DingTalkChannel::new(DINGTALK_WEBHOOK.into(), None, Box::new(mock));

    let error = channel.send(&Message::text("hello")).await.unwrap_err();

    match error {
        NotifyError::Provider {
            channel,
            code,
            message,
            ..
        } => {
            assert_eq!(channel, "dingtalk");
            assert_eq!(code.as_deref(), Some("310000"));
            assert_eq!(message, "sign not match");
        }
        other => panic!("expected provider error, got {other:?}"),
    }
}

#[tokio::test]
async fn dingtalk_card_degrades_to_markdown() {
    let mock = MockHttpClient::new(ok_response(r#"{"errcode":0,"errmsg":"ok"}"#));
    let channel = DingTalkChannel::new(DINGTALK_WEBHOOK.into(), None, Box::new(mock.clone()));

    downlink::send(&channel, &Message::card("Alert", "**details**"))
        .await
        .unwrap();

    let payload = json_body(&mock.requests()[0]);
    assert_eq!(payload["msgtype"], "markdown");
    assert_eq!(payload["markdown"]["title"], "Alert");
    assert_eq!(payload["markdown"]["text"], "**details**");
}

#[tokio::test]
async fn feishu_text_message_posts_text_payload() {
    let mock = MockHttpClient::new(ok_response(r#"{"code":0}"#));
    let webhook = "https://open.feishu.cn/open-apis/bot/v2/hook/test-hook";
    let channel = FeishuChannel::new(webhook.into(), None, Box::new(mock.clone()));

    channel.send(&Message::text("hello")).await.unwrap();

    let request = &mock.requests()[0];
    assert_eq!(request.url, webhook);
    let payload = json_body(request);
    assert_eq!(payload["msg_type"], "text");
    assert_eq!(payload["content"]["text"], "hello");
    assert!(payload.get("sign").is_none());
    assert!(payload.get("timestamp").is_none());
}

#[tokio::test]
async fn feishu_secret_signs_payload_with_hmac() {
    let secret = "s3cret";
    let mock = MockHttpClient::new(ok_response(r#"{"code":0}"#));
    let webhook = "https://open.feishu.cn/open-apis/bot/v2/hook/test-hook";
    let channel = FeishuChannel::new(webhook.into(), Some(secret.into()), Box::new(mock.clone()));

    channel.send(&Message::text("hello")).await.unwrap();

    let payload = json_body(&mock.requests()[0]);
    let timestamp = payload["timestamp"].as_str().expect("timestamp field");
    let sign = payload["sign"].as_str().expect("sign field");
    assert!(!timestamp.is_empty());

    let mut mac =
        Hmac::<Sha256>::new_from_slice(format!("{timestamp}\n{secret}").as_bytes()).unwrap();
    mac.update(b"");
    let expected = base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes());
    assert_eq!(sign, expected);
}

#[tokio::test]
async fn feishu_markdown_degrades_to_text() {
    let mock = MockHttpClient::new(ok_response(r#"{"code":0}"#));
    let webhook = "https://open.feishu.cn/open-apis/bot/v2/hook/test-hook";
    let channel = FeishuChannel::new(webhook.into(), None, Box::new(mock.clone()));

    downlink::send(&channel, &Message::markdown("**details**", None))
        .await
        .unwrap();

    let payload = json_body(&mock.requests()[0]);
    assert_eq!(payload["msg_type"], "text");
    assert_eq!(payload["content"]["text"], "details");
}

#[tokio::test]
async fn telegram_sends_text_and_extracts_message_id() {
    let mock = MockHttpClient::new(ok_response(r#"{"ok":true,"result":{"message_id":7}}"#));
    let channel =
        TelegramChannel::new("123456:ABC-DEF".into(), "42".into(), Box::new(mock.clone()));

    let receipt = channel.send(&Message::text("hello")).await.unwrap();

    let request = &mock.requests()[0];
    assert_eq!(
        request.url,
        "https://api.telegram.org/bot123456:ABC-DEF/sendMessage"
    );
    let payload = json_body(request);
    assert_eq!(payload["chat_id"], "42");
    assert_eq!(payload["text"], "hello");
    assert_eq!(receipt.channel, "telegram");
    assert_eq!(receipt.message_id.as_deref(), Some("7"));
}

#[tokio::test]
async fn telegram_markdown_sets_parse_mode() {
    let mock = MockHttpClient::new(ok_response(r#"{"ok":true,"result":{"message_id":7}}"#));
    let channel =
        TelegramChannel::new("123456:ABC-DEF".into(), "42".into(), Box::new(mock.clone()));

    channel
        .send(&Message::markdown("**details**", None))
        .await
        .unwrap();

    let payload = json_body(&mock.requests()[0]);
    assert_eq!(payload["parse_mode"], "MarkdownV2");
}

#[tokio::test]
async fn telegram_rejected_request_maps_to_provider_error() {
    let mock = MockHttpClient::new(ok_response(
        r#"{"ok":false,"error_code":400,"description":"chat not found"}"#,
    ));
    let channel = TelegramChannel::new("123456:ABC-DEF".into(), "42".into(), Box::new(mock));

    let error = channel.send(&Message::text("hello")).await.unwrap_err();

    match error {
        NotifyError::Provider {
            channel,
            code,
            message,
            ..
        } => {
            assert_eq!(channel, "telegram");
            assert_eq!(code.as_deref(), Some("400"));
            assert_eq!(message, "chat not found");
        }
        other => panic!("expected provider error, got {other:?}"),
    }
}

#[tokio::test]
async fn discord_text_posts_content_and_extracts_message_id() {
    let mock = MockHttpClient::new(ok_response(r#"{"id":"9999"}"#));
    let webhook = "https://discord.com/api/webhooks/1/abc";
    let channel = DiscordChannel::new(webhook.into(), Box::new(mock.clone()));

    let receipt = channel.send(&Message::text("hello")).await.unwrap();

    let request = &mock.requests()[0];
    assert_eq!(request.url, webhook);
    let payload = json_body(request);
    assert_eq!(payload["content"], "hello");
    assert_eq!(receipt.message_id.as_deref(), Some("9999"));
}

#[tokio::test]
async fn discord_card_posts_embed() {
    let mock = MockHttpClient::new(ok_response(r#"{"id":"9999"}"#));
    let channel = DiscordChannel::new(
        "https://discord.com/api/webhooks/1/abc".into(),
        Box::new(mock.clone()),
    );

    channel
        .send(&Message::card("Alert", "**details**"))
        .await
        .unwrap();

    let payload = json_body(&mock.requests()[0]);
    assert_eq!(payload["embeds"][0]["title"], "Alert");
    assert_eq!(payload["embeds"][0]["description"], "**details**");
}

#[tokio::test]
async fn discord_http_error_keeps_channel_context() {
    let mock = MockHttpClient::new(status_response(404, r#"{"message": "Unknown Webhook"}"#));
    let channel = DiscordChannel::new(
        "https://discord.com/api/webhooks/1/abc".into(),
        Box::new(mock),
    );

    let error = channel.send(&Message::text("hello")).await.unwrap_err();

    assert_eq!(error.channel(), Some("discord"));
    assert!(matches!(error, NotifyError::HttpStatus { .. }));
}

#[test]
fn config_url_and_json_forms_parse_consistently() {
    let dingtalk = ChannelConfig::from_url("dingtalk://tok123?secret=abc").unwrap();
    assert!(matches!(
        &dingtalk,
        ChannelConfig::DingTalk { webhook_url, secret: Some(secret) }
            if webhook_url.contains("access_token=tok123") && secret == "abc"
    ));

    let smtp = ChannelConfig::from_url("smtp://user:pass@smtp.example.com:465?to=a@b.com,c@d.com")
        .unwrap();
    assert!(matches!(
        &smtp,
        ChannelConfig::Smtp { host, port: 465, username, password, .. }
            if host == "smtp.example.com" && username == "user" && password == "pass"
    ));

    let telegram =
        ChannelConfig::from_json(r#"{"channel":"telegram","bot_token":"123:ABC","chat_id":"42"}"#)
            .unwrap();
    assert!(
        telegram
            .to_json()
            .unwrap()
            .contains(r#""bot_token":"123:ABC""#)
    );
}

#[test]
fn test_secrets_use_separate_keys_and_build_urls_at_runtime() {
    let raw = r#"{
        "dingtalk": {"access_token":"tok","secret":"s"},
        "feishu": {"hook_id":"hook-1","secret":null},
        "telegram": {"bot_token":"123:ABC","chat_id":"42"},
        "discord": {"webhook_id":"999","webhook_token":"wt"}
    }"#;
    let secrets: common::TestSecrets = serde_json::from_str(raw).unwrap();

    let dingtalk = secrets.dingtalk.as_ref().unwrap();
    assert_eq!(dingtalk.access_token, "tok");
    assert_eq!(dingtalk.secret.as_deref(), Some("s"));
    let ChannelConfig::DingTalk {
        webhook_url,
        secret,
    } = dingtalk.to_config()
    else {
        panic!("expected dingtalk config");
    };
    assert_eq!(
        webhook_url,
        "https://oapi.dingtalk.com/robot/send?access_token=tok"
    );
    assert_eq!(secret.as_deref(), Some("s"));

    let feishu = secrets.feishu.as_ref().unwrap();
    assert_eq!(feishu.hook_id, "hook-1");
    let ChannelConfig::Feishu {
        webhook_url,
        secret,
    } = feishu.to_config()
    else {
        panic!("expected feishu config");
    };
    assert_eq!(
        webhook_url,
        "https://open.feishu.cn/open-apis/bot/v2/hook/hook-1"
    );
    assert_eq!(secret, None);

    let telegram = secrets.telegram.as_ref().unwrap();
    assert_eq!(telegram.bot_token, "123:ABC");
    assert_eq!(telegram.chat_id, "42");

    let discord = secrets.discord.as_ref().unwrap();
    let ChannelConfig::Discord { webhook_url } = discord.to_config() else {
        panic!("expected discord config");
    };
    assert_eq!(webhook_url, "https://discord.com/api/webhooks/999/wt");
}

#[test]
fn example_secrets_file_stays_parseable() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/secrets.example.json");
    let raw = std::fs::read_to_string(path).unwrap();
    let secrets: common::TestSecrets = serde_json::from_str(&raw).unwrap();
    assert_eq!(secrets.dingtalk.unwrap().access_token, "YOUR_ACCESS_TOKEN");
    assert_eq!(secrets.feishu.unwrap().hook_id, "YOUR_HOOK_ID");
    assert_eq!(
        secrets.telegram.unwrap().bot_token,
        "123456:ABC-DEF_YOUR_BOT_TOKEN"
    );
    assert_eq!(secrets.discord.unwrap().webhook_id, "YOUR_WEBHOOK_ID");
}

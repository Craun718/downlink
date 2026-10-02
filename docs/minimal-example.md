# 最小示例

下面的示例使用默认的 `ReqwestClient` 和 Telegram 渠道发送一条纯文本通知。

先在你的项目 `Cargo.toml` 中加入依赖：

```toml
[dependencies]
downlink = "0.1"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

然后在 `src/main.rs` 中写入：

```rust
use downlink::{ChannelConfig, Message, NotifyError, ReqwestClient, channel_from_config};

#[tokio::main]
async fn main() -> Result<(), NotifyError> {
    let config = ChannelConfig::Telegram {
        bot_token: "BOT_TOKEN".into(),
        chat_id: "CHAT_ID".into(),
    };
    let channel = channel_from_config(&config, Box::new(ReqwestClient::new()))?;
    let message = Message::text("Hello from downlink");

    let receipt = channel.send_with_fallback(&message).await?;
    println!("sent by {}", receipt.channel);

    Ok(())
}
```

把 `BOT_TOKEN` 和 `CHAT_ID` 替换成自己的 Telegram Bot 凭证后，运行：

```sh
cargo run
```

`downlink` 默认启用常用通知渠道和 `default-client`。如果只需要 Telegram，可以在依赖中改用：

```toml
downlink = { version = "0.1", default-features = false, features = ["channel-telegram", "default-client"] }
```

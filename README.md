# downlink

通知聚合协议的 Rust core SDK：一套统一协议，覆盖多个通知渠道，跨平台复用。

## 设计目标

### 定位

- 做成 **SDK / core 库**，不做常驻服务、不做消息平台
- 后续桌面端与移动端项目通过绑定（Kotlin / Swift / C ABI）复用同一份核心逻辑
- 协议本身（消息模型 + 渠道配置 schema）是核心资产，Rust 实现是它的参考实现

### 只发不收

- 首期只支持单向发送，不做消息接收、不做 bot 事件订阅
- 无 WebSocket、无常驻连接、无后台任务，适配器均为无状态的一次性 HTTP/SMTP 调用

### 跨平台

- 编译目标覆盖：macOS、Windows、Linux、Android、iOS
- 远期通过 WASM 支持浏览器/Electron 场景
- 不依赖单平台 API，核心逻辑全部平台无关

### 当前渠道

| 渠道 | 接入方式 | 说明 |
|------|----------|------|
| 钉钉 | 群机器人 Webhook | 支持加签 secret |
| 飞书 | 自定义机器人 Webhook | 支持签名 secret |
| Telegram | Bot API | Bot token + chat id |
| Discord | Webhook | 频道 Webhook，配置成本最低 |
| Discord Bot | Bot API 私聊 | Bot token + 用户 id，先创建私聊频道再发送 |
| 邮箱 | SMTP | 独立 feature：`channel-smtp` |
| Server酱 | HTTP API | Send Key，支持 `sctapi` 与 `ft07` 域名 |
| Bark | HTTP API | 设备 Key，支持官方服务与自建服务器 |
| Gotify | HTTP API | App token，支持自建服务器与子路径 |
| Qmsg | HTTP API | Key + QQ，可选指定机器人 |
| WxPusher | HTTP API | 应用 token + UID，或 simple-push token |
| 自定义 Webhook | HTTP JSON POST | 支持自定义 header、content-type 与 JSON body 模板 |

新通知渠道默认随 `default` feature 编译；SMTP 因依赖完整邮件客户端而保持为可选
feature。宿主可以按渠道裁剪依赖，例如只启用 `channel-telegram`。

### 配置示例

Rust 内使用类型化的 `ChannelConfig`：

```rust
let config = ChannelConfig::Telegram {
    bot_token: bot_token,
    chat_id: chat_id,
};
```

需要持久化或跨语言传递时，使用同一结构的 JSON 形态：

```json
{
  "channel": "telegram",
  "bot_token": "BOT_TOKEN",
  "chat_id": "CHAT_ID"
}
```

### 架构原则

- **统一消息模型**：文本 / Markdown / 卡片 / @ / 优先级，与渠道无关
- **渠道适配器 trait**：每个渠道一个 `send` 实现，按 trait 注册
- **能力声明与统一降级**：库级 `send(channel, message)` 和 `Channel::send_with_fallback` 统一协商消息能力；卡片优先降级为 Markdown，再降级为纯文本，不支持的 @ 类型会被移除。无可用正文格式时返回 `UnsupportedCapability`
- **Markdown 纯文本降级**：由内置零依赖 crate `markdown-plain-text` 剥离常用 Markdown 语法，保留正文、链接目标和代码内容
- **配置驱动**：渠道配置采用 Rust 类型化结构与 JSON 双形态，字段显式、便于序列化和跨语言复用

### 密钥与安全

- **SDK 不内置任何密钥**，所有凭证由最终用户自己填写
- SDK 不做持久化存储，只定义存储接口，由宿主实现（Android Keystore / iOS Keychain / 桌面 secret service）
- 不在日志中输出任何凭证字段

### 非目标

- 不做消息接收、双向交互、按钮回调（远期再议）
- 不做密钥托管、云端账号体系
- 不追求 Apprise 级别的渠道数量；优先覆盖用户高频渠道并保持每个适配器的错误语义与能力降级一致

### 产物体积优化

本库默认关闭 reqwest 的 HTTP/2 与压缩等非必要 feature，仅保留 JSON 序列化与
native-tls。消费端可在自身 `Cargo.toml` 中加入以下 release profile 进一步缩小
链接后的二进制体积：

```toml
[profile.release]
lto = true
codegen-units = 1
strip = true
opt-level = "z"
panic = "abort"
```

实测效果（Bark 渠道最小示例）：默认 release 约 4.4 MB → 优化后约 1.2 MB。

## 许可

MIT OR Apache-2.0 双许可，任选其一。

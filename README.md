# notify-core

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

### 首发渠道

| 渠道 | 接入方式 | 说明 |
|------|----------|------|
| 钉钉 | 群机器人 Webhook | 用户自建机器人，填 webhook + 加签 secret |
| 飞书 | 自定义机器人 Webhook | 用户自建，填 webhook + 签名 secret |
| Telegram | Bot API | 用户找 BotFather 建 bot，填 token |
| Discord | 频道 Webhook | 优先于 bot token，用户配置成本最低 |
| 邮箱 | SMTP | 唯一需要完整客户端实现的渠道 |

### 架构原则

- **统一消息模型**：文本 / Markdown / 卡片 / @ / 优先级，与渠道无关
- **渠道适配器 trait**：每个渠道一个 `send` 实现，按 trait 注册
- **能力声明**：适配器声明支持的消息形态（如 webhook 渠道不支持卡片则自动降级为文本）
- **配置驱动**：渠道配置采用 URL schema（参考 Apprise）与 JSON 双形态，便于序列化、导入导出

### 密钥与安全

- **SDK 不内置任何密钥**，所有凭证由最终用户自己填写
- SDK 不做持久化存储，只定义存储接口，由宿主实现（Android Keystore / iOS Keychain / 桌面 secret service）
- 不在日志中输出任何凭证字段

### 非目标

- 不做消息接收、双向交互、按钮回调（远期再议）
- 不做密钥托管、云端账号体系
- 不追求 Apprise 级别的渠道数量，首发只做上表五个，做深做稳

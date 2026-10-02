# 编译与体积优化

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

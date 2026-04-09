# 变更记录

## 2026-04-09

### 新增功能

#### 飞书 Webhook 通知 (`src/feishu.rs`)

新增 `FeishuNotifier` 模块，在每次套利成交后通过飞书机器人推送消息。

- **模拟交易成交** 推送内容：方向、数量、买入/卖出价、毛利、手续费、净利润
- **实盘成交** 推送内容：方向、数量、买入/卖出均价、净利润
- 通知通过配置的 HTTP 代理发出，与主程序代理设置一致
- 飞书 Webhook 地址通过 `config.toml` 的 `[monitor]` 段配置，字段缺失时静默禁用

配置示例：

```toml
[monitor]
refresh_ms = 200
feishu_webhook = "https://open.feishu.cn/open-apis/bot/v2/hook/<your-token>"
```

### Bug 修复

#### WebSocket 代理连接 TLS 握手失败 (`src/binance/ws.rs`)

**问题**：通过本地 HTTP CONNECT 代理（如 Clash）连接 Binance WebSocket 时，概率性出现 TLS 错误：
- `TLS error: native-tls error: connection closed via error`
- `TLS error: native-tls error: record overflow`

**根因**：原实现用单次 `read()` 读取代理的 `200 Connection Established` 响应。若响应分多个 TCP 包到达，TLS 握手会在 HTTP 头部未完全消费时启动，导致 TLS 层把剩余的 HTTP 数据当作 TLS 记录解析，触发 `record overflow` 警告并断开连接。

**修复**：改为逐字节读取直到遇到 HTTP 头部结束标志 `\r\n\r\n`，确保 TLS 握手在 HTTP 协商完全结束后才开始。同时在 CONNECT 请求中加入 `Proxy-Connection: Keep-Alive` 头，增强与不同代理实现的兼容性。

### 配置变更

| 文件 | 变更内容 |
|------|----------|
| `config.toml` | `[monitor]` 段新增 `feishu_webhook` 字段 |
| `src/config.rs` | `MonitorConfig` 新增 `feishu_webhook: Option<String>` 字段 |
| `src/main.rs` | 引入 `feishu` 模块，在成交回调中触发通知 |
| `src/binance/ws.rs` | 修复代理响应读取逻辑，修复 TLS 握手失败问题 |
| `src/feishu.rs` | 新增文件 |

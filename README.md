# btc-arb

基于 Rust 的 BTC 跨稳定币套利机器人，监控 Binance 上 **BTCFDUSD** 与 **BTCUSDT** 两个交易对之间的价差，在价差超过设定阈值时自动执行套利交易。

支持**模拟交易**和**实盘交易**两种模式，并可通过飞书机器人推送成交通知。

---

## 套利原理

FDUSD 与 USDT 均为 1:1 锚定美元的稳定币，理论上 BTCFDUSD 和 BTCUSDT 价格应相同。当两者出现价差且扣除手续费后仍有盈余时，即可同时在价低的一侧买入、价高的一侧卖出，锁定无风险收益。

```
方向一：在 BTCFDUSD 买入 BTC，同时在 BTCUSDT 卖出 BTC
方向二：在 BTCUSDT  买入 BTC，同时在 BTCFDUSD 卖出 BTC
```

---

## 环境要求

- Rust 1.80+（推荐通过 [rustup](https://rustup.rs/) 安装）
- 可访问 Binance 的网络环境（支持 HTTP 代理）

---

## 快速开始

### 1. 克隆项目

```bash
git clone https://github.com/dionysus151515/btc-arb.git
cd btc-arb
```

### 2. 编辑配置文件

复制并按需修改 `config.toml`（默认已有可用配置）：

```toml
[trading]
symbols = ["BTCFDUSD", "BTCUSDT"]
min_profit_bps = 5          # 触发交易的最低净利润（万分之5 = 0.05%）
max_trade_usdt = 1000.0     # 单次最大交易金额（USDT）
taker_fee_bps = 10          # Taker 手续费（万分之10 = 0.1%）
fdusd_maker_fee_bps = 0     # FDUSD 交易对 Maker 手续费（享受0费率时填0）

[paper_trading]
initial_usdt = 10000.0      # 模拟账户初始 USDT
initial_fdusd = 10000.0     # 模拟账户初始 FDUSD
enabled = true

[live_trading]
enabled = false             # 开启实盘前请先用模拟模式验证
api_key_env = "BINANCE_API_KEY"
secret_key_env = "BINANCE_SECRET_KEY"
max_daily_loss_usdt = 50.0  # 每日最大亏损，超出后自动停止
max_position_btc = 0.01     # 单次最大仓位（BTC）
cooldown_ms = 5000          # 两次交易之间的冷却时间（毫秒）

[network]
proxy = "http://127.0.0.1:7890"   # 本地代理，不需要可删除此行

[websocket]
depth_level = 20            # 订单簿深度
update_speed_ms = 100       # 行情推送频率（毫秒）

[monitor]
refresh_ms = 200            # 控制台刷新频率（毫秒）
feishu_webhook = "https://open.feishu.cn/open-apis/bot/v2/hook/<your-token>"
                            # 飞书机器人 Webhook，不需要可删除此行
```

### 3. 编译并运行（模拟交易）

```bash
cargo run
```

---

## 实盘交易

> **警告：实盘交易涉及真实资金，请充分测试后再启用。**

**第一步**：在 Binance 创建 API Key，开启现货交易权限，**不要开启提现权限**。

**第二步**：设置环境变量：

```bash
export BINANCE_API_KEY="your_api_key"
export BINANCE_SECRET_KEY="your_secret_key"
```

**第三步**：在 `config.toml` 中启用实盘：

```toml
[live_trading]
enabled = true
```

**第四步**：运行：

```bash
cargo run
```

---

## 控制台界面

```
╔══════════════════════════════════════════════════════════════╗
║       BTC Arbitrage Monitor (FDUSD / USDT)  [PAPER]        ║
╠══════════════════════════════════════════════════════════════╣
║           BTC/FDUSD           │           BTC/USDT           ║
║  Bid:      70853.74           │ Bid:      70801.30           ║
║  Ask:      70853.75           │ Ask:      70801.31           ║
║  Spread:      0.00 bps        │ Spread:      0.00 bps        ║
╠══════════════════════════════════════════════════════════════╣
║  Cross-pair Spread:                                          ║
║    FDUSD->USDT:    -7.40 bps    USDT->FDUSD:    +7.41 bps  ║
╠══════════════════════════════════════════════════════════════╣
║  Paper Trading:                                              ║
║    USDT:     10000.00    FDUSD:     10000.00                 ║
║    Trades:      0   Win Rate:   0.0%   P&L:    +0.0000 USDT ║
╚══════════════════════════════════════════════════════════════╝
```

---

## 飞书通知

在 `config.toml` 的 `[monitor]` 段填入飞书自定义机器人的 Webhook 地址后，每次成交会推送如下消息：

**模拟交易成交：**
```
[BTC-ARB PAPER] BUY_USDT->SELL_FDUSD
Qty: 0.001234 BTC
Buy: 70801.31  Sell: 70853.74
Gross: 0.0648  Fees: 0.0141  Net: 0.0507 USDT
```

**实盘成交：**
```
[BTC-ARB LIVE] BUY_FDUSD->SELL_USDT
Qty: 0.001000 BTC
Buy: 70853.75  Sell: 70920.00
Net P&L: 0.0663 USDT
```

飞书机器人创建方式：飞书群 → 设置 → 机器人 → 添加机器人 → 自定义机器人。

---

## 成交记录

模拟交易的每笔成交会追加写入项目根目录的 `trades.csv`，字段如下：

```
timestamp, direction, btc_qty, buy_price, sell_price, gross_profit_usdt, fees_usdt, net_profit_usdt
```

---

## 配置参数说明

| 参数 | 说明 | 默认值 |
|------|------|--------|
| `min_profit_bps` | 触发交易的最低净利润（万分之一为单位）| 5 |
| `max_trade_usdt` | 单次最大交易金额 | 1000 |
| `taker_fee_bps` | Taker 手续费率 | 10（0.1%）|
| `fdusd_maker_fee_bps` | FDUSD 交易对 Maker 费率 | 0 |
| `max_daily_loss_usdt` | 实盘每日最大亏损上限，触发后暂停交易 | 50 |
| `max_position_btc` | 实盘单次最大 BTC 仓位 | 0.01 |
| `cooldown_ms` | 实盘两次交易之间的最短间隔 | 5000 |

---

## 风险提示

- 本程序仅供学习与研究，不构成投资建议
- 套利窗口可能在订单实际成交前消失，导致滑点亏损
- 实盘使用前请充分了解 Binance API 限频规则
- 请妥善保管 API Key，建议绑定 IP 白名单并禁用提现权限

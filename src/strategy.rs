use chrono::Utc;
use tracing::{info, warn, error};

use crate::binance::rest::BinanceClient;
use crate::config::Config;
use crate::orderbook::OrderBook;

/// Direction of the arbitrage
#[derive(Debug, Clone, Copy)]
pub enum ArbDirection {
    /// Buy on FDUSD pair (cheaper), sell on USDT pair (more expensive)
    BuyFdusdSellUsdt,
    /// Buy on USDT pair (cheaper), sell on FDUSD pair (more expensive)
    BuyUsdtSellFdusd,
}

impl std::fmt::Display for ArbDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArbDirection::BuyFdusdSellUsdt => write!(f, "BUY_FDUSD->SELL_USDT"),
            ArbDirection::BuyUsdtSellFdusd => write!(f, "BUY_USDT->SELL_FDUSD"),
        }
    }
}

/// A detected arbitrage opportunity
#[derive(Debug, Clone)]
pub struct ArbSignal {
    pub direction: ArbDirection,
    pub buy_price: f64,
    pub sell_price: f64,
    pub spread_bps: f64,
    pub net_profit_bps: f64,
    pub max_btc_qty: f64,
    pub max_profit_usdt: f64,
    pub timestamp: chrono::DateTime<Utc>,
}

/// Paper trading state
#[derive(Debug)]
pub struct PaperTrader {
    pub usdt_balance: f64,
    pub fdusd_balance: f64,
    pub btc_balance: f64,
    pub total_trades: u64,
    pub winning_trades: u64,
    pub total_profit_usdt: f64,
    pub trades: Vec<PaperTrade>,
}

#[derive(Debug, Clone)]
pub struct PaperTrade {
    pub timestamp: chrono::DateTime<Utc>,
    pub direction: ArbDirection,
    pub btc_qty: f64,
    pub buy_price: f64,
    pub sell_price: f64,
    pub gross_profit_usdt: f64,
    pub fees_usdt: f64,
    pub net_profit_usdt: f64,
}

impl PaperTrader {
    pub fn new(initial_usdt: f64, initial_fdusd: f64) -> Self {
        Self {
            usdt_balance: initial_usdt,
            fdusd_balance: initial_fdusd,
            btc_balance: 0.0,
            total_trades: 0,
            winning_trades: 0,
            total_profit_usdt: 0.0,
            trades: Vec::new(),
        }
    }

    pub fn win_rate(&self) -> f64 {
        if self.total_trades == 0 {
            0.0
        } else {
            self.winning_trades as f64 / self.total_trades as f64 * 100.0
        }
    }
}

/// Check for arbitrage opportunities between two order books
pub fn detect_arbitrage(
    fdusd_book: &OrderBook,
    usdt_book: &OrderBook,
    config: &Config,
) -> Option<ArbSignal> {
    let fdusd_ask = fdusd_book.best_ask()?;
    let fdusd_bid = fdusd_book.best_bid()?;
    let usdt_ask = usdt_book.best_ask()?;
    let usdt_bid = usdt_book.best_bid()?;

    let taker_fee = config.taker_fee_ratio();
    let fdusd_fee = config.fdusd_maker_fee_ratio();

    // Direction 1: Buy on FDUSD (pay ask), sell on USDT (receive bid)
    let spread1_bps = (usdt_bid.price / fdusd_ask.price - 1.0) * 10000.0;
    let net1_bps = spread1_bps - (fdusd_fee + taker_fee) * 10000.0;

    // Direction 2: Buy on USDT (pay ask), sell on FDUSD (receive bid)
    let spread2_bps = (fdusd_bid.price / usdt_ask.price - 1.0) * 10000.0;
    let net2_bps = spread2_bps - (taker_fee + fdusd_fee) * 10000.0;

    let min_profit = config.min_profit_ratio() * 10000.0;

    if net1_bps > min_profit && net1_bps >= net2_bps {
        let max_qty = fdusd_ask.qty.min(usdt_bid.qty);
        let max_trade_btc = config.trading.max_trade_usdt / fdusd_ask.price;
        let qty = max_qty.min(max_trade_btc);
        let profit = qty * fdusd_ask.price * net1_bps / 10000.0;

        Some(ArbSignal {
            direction: ArbDirection::BuyFdusdSellUsdt,
            buy_price: fdusd_ask.price,
            sell_price: usdt_bid.price,
            spread_bps: spread1_bps,
            net_profit_bps: net1_bps,
            max_btc_qty: qty,
            max_profit_usdt: profit,
            timestamp: Utc::now(),
        })
    } else if net2_bps > min_profit {
        let max_qty = usdt_ask.qty.min(fdusd_bid.qty);
        let max_trade_btc = config.trading.max_trade_usdt / usdt_ask.price;
        let qty = max_qty.min(max_trade_btc);
        let profit = qty * usdt_ask.price * net2_bps / 10000.0;

        Some(ArbSignal {
            direction: ArbDirection::BuyUsdtSellFdusd,
            buy_price: usdt_ask.price,
            sell_price: fdusd_bid.price,
            spread_bps: spread2_bps,
            net_profit_bps: net2_bps,
            max_btc_qty: qty,
            max_profit_usdt: profit,
            timestamp: Utc::now(),
        })
    } else {
        None
    }
}

/// Execute a paper trade based on an arbitrage signal
pub fn execute_paper_trade(
    signal: &ArbSignal,
    trader: &mut PaperTrader,
    config: &Config,
) -> Option<PaperTrade> {
    let taker_fee = config.taker_fee_ratio();
    let fdusd_fee = config.fdusd_maker_fee_ratio();

    let btc_qty = signal.max_btc_qty;
    if btc_qty <= 0.0 {
        return None;
    }

    let (buy_cost, sell_revenue, buy_fee_rate, sell_fee_rate) = match signal.direction {
        ArbDirection::BuyFdusdSellUsdt => {
            let cost = btc_qty * signal.buy_price;
            let revenue = btc_qty * signal.sell_price;
            if trader.fdusd_balance < cost {
                return None;
            }
            (cost, revenue, fdusd_fee, taker_fee)
        }
        ArbDirection::BuyUsdtSellFdusd => {
            let cost = btc_qty * signal.buy_price;
            let revenue = btc_qty * signal.sell_price;
            if trader.usdt_balance < cost {
                return None;
            }
            (cost, revenue, taker_fee, fdusd_fee)
        }
    };

    let buy_fees = buy_cost * buy_fee_rate;
    let sell_fees = sell_revenue * sell_fee_rate;
    let total_fees = buy_fees + sell_fees;
    let gross_profit = sell_revenue - buy_cost;
    let net_profit = gross_profit - total_fees;

    // Update balances
    match signal.direction {
        ArbDirection::BuyFdusdSellUsdt => {
            trader.fdusd_balance -= buy_cost + buy_fees;
            trader.usdt_balance += sell_revenue - sell_fees;
        }
        ArbDirection::BuyUsdtSellFdusd => {
            trader.usdt_balance -= buy_cost + buy_fees;
            trader.fdusd_balance += sell_revenue - sell_fees;
        }
    }

    trader.total_trades += 1;
    if net_profit > 0.0 {
        trader.winning_trades += 1;
    }
    trader.total_profit_usdt += net_profit;

    let trade = PaperTrade {
        timestamp: signal.timestamp,
        direction: signal.direction,
        btc_qty,
        buy_price: signal.buy_price,
        sell_price: signal.sell_price,
        gross_profit_usdt: gross_profit,
        fees_usdt: total_fees,
        net_profit_usdt: net_profit,
    };

    info!(
        "PAPER TRADE: {} | qty={:.6} BTC | buy={:.2} sell={:.2} | net={:.4} USDT",
        trade.direction, trade.btc_qty, trade.buy_price, trade.sell_price, trade.net_profit_usdt
    );

    trader.trades.push(trade.clone());
    Some(trade)
}

// ===== Live Trading =====

/// Live trading state
pub struct LiveTrader {
    pub client: BinanceClient,
    pub daily_pnl: f64,
    pub max_daily_loss: f64,
    pub max_position_btc: f64,
    pub last_trade_time: Option<std::time::Instant>,
    pub cooldown: std::time::Duration,
    pub total_trades: u64,
    pub total_profit_usdt: f64,
    pub halted: bool,
    pub trades: Vec<LiveTrade>,
}

#[derive(Debug, Clone)]
pub struct LiveTrade {
    pub timestamp: chrono::DateTime<Utc>,
    pub direction: ArbDirection,
    pub buy_order_id: u64,
    pub sell_order_id: u64,
    pub btc_qty: f64,
    pub buy_quote_spent: f64,
    pub sell_quote_received: f64,
    pub net_profit_usdt: f64,
}

impl LiveTrader {
    pub fn new(client: BinanceClient, config: &Config) -> Self {
        Self {
            client,
            daily_pnl: 0.0,
            max_daily_loss: config.live_trading.max_daily_loss_usdt,
            max_position_btc: config.live_trading.max_position_btc,
            last_trade_time: None,
            cooldown: std::time::Duration::from_millis(config.live_trading.cooldown_ms),
            total_trades: 0,
            total_profit_usdt: 0.0,
            halted: false,
            trades: Vec::new(),
        }
    }

    /// Check if trading is allowed based on safety limits
    fn can_trade(&self, btc_qty: f64) -> Result<(), String> {
        if self.halted {
            return Err("Trading halted due to daily loss limit".into());
        }
        if self.daily_pnl <= -self.max_daily_loss {
            return Err(format!(
                "Daily loss limit reached: {:.2} / -{:.2}",
                self.daily_pnl, self.max_daily_loss
            ));
        }
        if btc_qty > self.max_position_btc {
            return Err(format!(
                "Position too large: {:.6} > {:.6} BTC",
                btc_qty, self.max_position_btc
            ));
        }
        if let Some(last) = self.last_trade_time {
            if last.elapsed() < self.cooldown {
                let remaining = self.cooldown - last.elapsed();
                return Err(format!("Cooldown active: {:.1}s remaining", remaining.as_secs_f64()));
            }
        }
        Ok(())
    }

    pub fn cooldown_remaining(&self) -> Option<std::time::Duration> {
        self.last_trade_time.and_then(|last| {
            let elapsed = last.elapsed();
            if elapsed < self.cooldown {
                Some(self.cooldown - elapsed)
            } else {
                None
            }
        })
    }
}

/// Execute a live trade: place two market orders simultaneously
pub async fn execute_live_trade(
    signal: &ArbSignal,
    trader: &mut LiveTrader,
) -> Result<LiveTrade, Box<dyn std::error::Error + Send + Sync>> {
    // Safety checks
    if let Err(reason) = trader.can_trade(signal.max_btc_qty) {
        warn!("Trade blocked: {}", reason);
        return Err(reason.into());
    }

    let btc_qty = signal.max_btc_qty;
    let (buy_symbol, sell_symbol) = match signal.direction {
        ArbDirection::BuyFdusdSellUsdt => ("BTCFDUSD", "BTCUSDT"),
        ArbDirection::BuyUsdtSellFdusd => ("BTCUSDT", "BTCFDUSD"),
    };

    info!(
        "LIVE TRADE: {} | qty={:.6} BTC | buy {} sell {}",
        signal.direction, btc_qty, buy_symbol, sell_symbol
    );

    // Place both orders concurrently
    let buy_fut = trader.client.place_market_order(buy_symbol, "BUY", btc_qty);
    let sell_fut = trader.client.place_market_order(sell_symbol, "SELL", btc_qty);

    let (buy_result, sell_result) = tokio::join!(buy_fut, sell_fut);

    let buy_order = match buy_result {
        Ok(o) => o,
        Err(e) => {
            error!("Buy order failed: {e}");
            return Err(e);
        }
    };

    let sell_order = match sell_result {
        Ok(o) => o,
        Err(e) => {
            error!("Sell order failed (buy already placed!): {e}");
            // Buy went through but sell failed - this needs manual attention
            warn!(
                "WARNING: Buy order {} filled but sell failed. Manual intervention needed!",
                buy_order.order_id
            );
            return Err(e);
        }
    };

    let buy_spent = buy_order.cumulative_quote_qty.parse::<f64>().unwrap_or(0.0);
    let sell_received = sell_order.cumulative_quote_qty.parse::<f64>().unwrap_or(0.0);
    let net_profit = sell_received - buy_spent;

    trader.daily_pnl += net_profit;
    trader.total_profit_usdt += net_profit;
    trader.total_trades += 1;
    trader.last_trade_time = Some(std::time::Instant::now());

    if trader.daily_pnl <= -trader.max_daily_loss {
        warn!("Daily loss limit reached! Halting trading.");
        trader.halted = true;
    }

    let trade = LiveTrade {
        timestamp: Utc::now(),
        direction: signal.direction,
        buy_order_id: buy_order.order_id,
        sell_order_id: sell_order.order_id,
        btc_qty,
        buy_quote_spent: buy_spent,
        sell_quote_received: sell_received,
        net_profit_usdt: net_profit,
    };

    info!(
        "LIVE TRADE DONE: buy_spent={:.2} sell_recv={:.2} net={:.4} USDT",
        trade.buy_quote_spent, trade.sell_quote_received, trade.net_profit_usdt
    );

    trader.trades.push(trade.clone());
    Ok(trade)
}

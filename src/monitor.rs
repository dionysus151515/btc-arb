use std::io::{self, Write};

use crate::orderbook::OrderBook;
use crate::strategy::{LiveTrader, PaperTrader};

/// Clear screen and render the monitoring dashboard
pub fn render(
    fdusd_book: &OrderBook,
    usdt_book: &OrderBook,
    paper_trader: &PaperTrader,
    live_trader: Option<&LiveTrader>,
    is_live: bool,
    opportunities_seen: u64,
) {
    // Move cursor to top-left and clear screen
    print!("\x1b[2J\x1b[H");

    let mode_tag = if is_live { "[LIVE]" } else { "[PAPER]" };

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!(
        "║       BTC Arbitrage Monitor (FDUSD / USDT) {:>8}        ║",
        mode_tag
    );
    println!("╠══════════════════════════════════════════════════════════════╣");

    // Order book display
    let fdusd_bid = fdusd_book.best_bid().map(|l| l.price).unwrap_or(0.0);
    let fdusd_ask = fdusd_book.best_ask().map(|l| l.price).unwrap_or(0.0);
    let usdt_bid = usdt_book.best_bid().map(|l| l.price).unwrap_or(0.0);
    let usdt_ask = usdt_book.best_ask().map(|l| l.price).unwrap_or(0.0);

    println!("║  {:^28} │ {:^28} ║", "BTC/FDUSD", "BTC/USDT");
    println!("║  ─────────────────────────── │ ─────────────────────────── ║");
    println!(
        "║  Bid: {:>12.2}           │ Bid: {:>12.2}           ║",
        fdusd_bid, usdt_bid
    );
    println!(
        "║  Ask: {:>12.2}           │ Ask: {:>12.2}           ║",
        fdusd_ask, usdt_ask
    );

    let fdusd_spread = if fdusd_bid > 0.0 {
        (fdusd_ask / fdusd_bid - 1.0) * 10000.0
    } else {
        0.0
    };
    let usdt_spread = if usdt_bid > 0.0 {
        (usdt_ask / usdt_bid - 1.0) * 10000.0
    } else {
        0.0
    };
    println!(
        "║  Spread: {:>6.2} bps          │ Spread: {:>6.2} bps          ║",
        fdusd_spread, usdt_spread
    );

    println!("╠══════════════════════════════════════════════════════════════╣");

    // Cross-pair spread
    let cross_spread_1 = if fdusd_ask > 0.0 {
        (usdt_bid / fdusd_ask - 1.0) * 10000.0
    } else {
        0.0
    };
    let cross_spread_2 = if usdt_ask > 0.0 {
        (fdusd_bid / usdt_ask - 1.0) * 10000.0
    } else {
        0.0
    };

    println!("║  Cross-pair Spread:                                        ║");
    println!(
        "║    FDUSD->USDT: {:>+8.2} bps    USDT->FDUSD: {:>+8.2} bps  ║",
        cross_spread_1, cross_spread_2
    );

    println!("╠══════════════════════════════════════════════════════════════╣");

    // Trading stats - show live or paper depending on mode
    if let Some(lt) = live_trader {
        let status = if lt.halted { "HALTED" } else { "ACTIVE" };
        let cooldown_str = match lt.cooldown_remaining() {
            Some(d) => format!("{:.1}s", d.as_secs_f64()),
            None => "ready".to_string(),
        };
        println!(
            "║  Live Trading [{}]:                                     ║",
            status
        );
        println!(
            "║    Trades: {:>6}   Daily P&L: {:>+10.4} / -{:.2} USDT   ║",
            lt.total_trades, lt.daily_pnl, lt.max_daily_loss
        );
        println!(
            "║    Total P&L: {:>+10.4} USDT   Cooldown: {:>8}        ║",
            lt.total_profit_usdt, cooldown_str
        );

        // Last 5 live trades
        if !lt.trades.is_empty() {
            println!("║  Recent Live Trades:                                       ║");
            let start = lt.trades.len().saturating_sub(5);
            for trade in &lt.trades[start..] {
                let time = trade.timestamp.format("%H:%M:%S");
                println!(
                    "║  {} {} {:.6} BTC net {:>+.4} USDT           ║",
                    time,
                    match trade.direction {
                        crate::strategy::ArbDirection::BuyFdusdSellUsdt => "F->U",
                        crate::strategy::ArbDirection::BuyUsdtSellFdusd => "U->F",
                    },
                    trade.btc_qty,
                    trade.net_profit_usdt,
                );
            }
        }
    } else {
        println!("║  Paper Trading:                                            ║");
        println!(
            "║    USDT: {:>12.2}    FDUSD: {:>12.2}                ║",
            paper_trader.usdt_balance, paper_trader.fdusd_balance
        );
        println!(
            "║    Trades: {:>6}   Win Rate: {:>5.1}%   P&L: {:>+10.4} USDT  ║",
            paper_trader.total_trades,
            paper_trader.win_rate(),
            paper_trader.total_profit_usdt
        );

        // Last 5 paper trades
        if !paper_trader.trades.is_empty() {
            let start = paper_trader.trades.len().saturating_sub(5);
            for trade in &paper_trader.trades[start..] {
                let time = trade.timestamp.format("%H:%M:%S");
                println!(
                    "║  {} {} {:.6} BTC net {:>+.4} USDT           ║",
                    time,
                    match trade.direction {
                        crate::strategy::ArbDirection::BuyFdusdSellUsdt => "F->U",
                        crate::strategy::ArbDirection::BuyUsdtSellFdusd => "U->F",
                    },
                    trade.btc_qty,
                    trade.net_profit_usdt,
                );
            }
        }
    }

    println!(
        "║    Opportunities seen: {:>6}                               ║",
        opportunities_seen
    );

    println!("╠══════════════════════════════════════════════════════════════╣");
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC");
    let fdusd_age = chrono::Utc::now().timestamp_millis() - fdusd_book.update_time_ms;
    let usdt_age = chrono::Utc::now().timestamp_millis() - usdt_book.update_time_ms;
    println!(
        "║  {} | Latency: F={:>4}ms U={:>4}ms      ║",
        now, fdusd_age, usdt_age
    );
    println!("║  Press Ctrl+C to exit                                      ║");
    println!("╚══════════════════════════════════════════════════════════════╝");

    let _ = io::stdout().flush();
}

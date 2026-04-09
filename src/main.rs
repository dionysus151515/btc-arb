mod binance;
mod config;
mod logger;
mod monitor;
mod orderbook;
mod strategy;

use tokio::sync::mpsc;
use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("btc_arb=info".parse().unwrap()),
        )
        .with_target(false)
        .init();

    // Load config
    let config = config::Config::load("config.toml")?;
    info!(
        "Config loaded: symbols={:?}, min_profit={}bps, max_trade={}USDT",
        config.trading.symbols, config.trading.min_profit_bps, config.trading.max_trade_usdt
    );

    // Initialize order books
    let mut fdusd_book = orderbook::OrderBook::new("BTCFDUSD");
    let mut usdt_book = orderbook::OrderBook::new("BTCUSDT");

    // Initialize paper trader
    let mut paper_trader = strategy::PaperTrader::new(
        config.paper_trading.initial_usdt,
        config.paper_trading.initial_fdusd,
    );

    // Initialize live trader if enabled
    let mut live_trader = if config.live_trading.enabled {
        match (config.live_trading.api_key(), config.live_trading.secret_key()) {
            (Ok(api_key), Ok(secret_key)) => {
                let client = binance::rest::BinanceClient::new(
                    api_key,
                    secret_key,
                    config.network.proxy.as_deref(),
                );
                info!("Live trading ENABLED, querying account...");
                match client.get_account().await {
                    Ok(account) => {
                        for b in &account.balances {
                            let free: f64 = b.free.parse().unwrap_or(0.0);
                            let locked: f64 = b.locked.parse().unwrap_or(0.0);
                            if free > 0.0 || locked > 0.0 {
                                info!("  {}: free={} locked={}", b.asset, b.free, b.locked);
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to query account: {e}");
                    }
                }
                Some(strategy::LiveTrader::new(client, &config))
            }
            (Err(e), _) | (_, Err(e)) => {
                error!("Live trading enabled but API keys missing: {e}");
                error!("Falling back to paper trading only.");
                None
            }
        }
    } else {
        info!("Live trading disabled, paper trading only.");
        None
    };

    let is_live = live_trader.is_some();

    // Initialize CSV logger
    let mut csv_logger = logger::CsvLogger::new("trades.csv")?;
    info!("CSV logger initialized: trades.csv");

    // Start WebSocket connection
    let (tx, mut rx) = mpsc::unbounded_channel();
    let symbols = config.trading.symbols.clone();
    let depth_level = config.websocket.depth_level;
    let update_speed = config.websocket.update_speed_ms;
    let ws_proxy = config.network.proxy.clone();

    tokio::spawn(async move {
        binance::ws::run_ws(&symbols, depth_level, update_speed, ws_proxy, tx).await;
    });

    let mut opportunities_seen: u64 = 0;
    let mut last_render = std::time::Instant::now();
    let render_interval = std::time::Duration::from_millis(config.monitor.refresh_ms);

    info!("Starting main loop, waiting for data...");

    loop {
        // Process all pending messages (non-blocking drain)
        let mut updated = false;
        while let Ok(update) = rx.try_recv() {
            match update.symbol.as_str() {
                "BTCFDUSD" => {
                    fdusd_book.update(
                        update.data.parse_bids(),
                        update.data.parse_asks(),
                        update.data.last_update_id,
                        update.recv_time_ms,
                    );
                    updated = true;
                }
                "BTCUSDT" => {
                    usdt_book.update(
                        update.data.parse_bids(),
                        update.data.parse_asks(),
                        update.data.last_update_id,
                        update.recv_time_ms,
                    );
                    updated = true;
                }
                _ => {}
            }
        }

        // If no messages were available, wait for one (blocking)
        if !updated {
            if let Some(update) = rx.recv().await {
                match update.symbol.as_str() {
                    "BTCFDUSD" => {
                        fdusd_book.update(
                            update.data.parse_bids(),
                            update.data.parse_asks(),
                            update.data.last_update_id,
                            update.recv_time_ms,
                        );
                    }
                    "BTCUSDT" => {
                        usdt_book.update(
                            update.data.parse_bids(),
                            update.data.parse_asks(),
                            update.data.last_update_id,
                            update.recv_time_ms,
                        );
                    }
                    _ => {}
                }
            } else {
                break; // Channel closed
            }
        }

        // Check for arbitrage opportunities
        if let Some(signal) = strategy::detect_arbitrage(&fdusd_book, &usdt_book, &config) {
            opportunities_seen += 1;

            // Live trading takes priority
            if let Some(ref mut lt) = live_trader {
                match strategy::execute_live_trade(&signal, lt).await {
                    Ok(trade) => {
                        info!("Live trade profit: {:.4} USDT", trade.net_profit_usdt);
                    }
                    Err(e) => {
                        warn!("Live trade skipped: {e}");
                    }
                }
            } else if config.paper_trading.enabled {
                if let Some(trade) =
                    strategy::execute_paper_trade(&signal, &mut paper_trader, &config)
                {
                    let _ = csv_logger.log_trade(&trade);
                }
            }
        }

        // Render dashboard at configured interval
        if last_render.elapsed() >= render_interval {
            monitor::render(
                &fdusd_book,
                &usdt_book,
                &paper_trader,
                live_trader.as_ref(),
                is_live,
                opportunities_seen,
            );
            last_render = std::time::Instant::now();
        }
    }

    Ok(())
}

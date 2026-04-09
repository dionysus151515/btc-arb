use serde::Deserialize;
use std::fs;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub trading: TradingConfig,
    pub paper_trading: PaperTradingConfig,
    pub live_trading: LiveTradingConfig,
    pub network: NetworkConfig,
    pub websocket: WebSocketConfig,
    pub monitor: MonitorConfig,
}

#[derive(Debug, Deserialize)]
pub struct TradingConfig {
    pub symbols: Vec<String>,
    pub min_profit_bps: f64,
    pub max_trade_usdt: f64,
    pub taker_fee_bps: f64,
    pub fdusd_maker_fee_bps: f64,
}

#[derive(Debug, Deserialize)]
pub struct PaperTradingConfig {
    pub initial_usdt: f64,
    pub initial_fdusd: f64,
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct WebSocketConfig {
    pub depth_level: u32,
    pub update_speed_ms: u32,
}

#[derive(Debug, Deserialize)]
pub struct NetworkConfig {
    pub proxy: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LiveTradingConfig {
    pub enabled: bool,
    pub api_key_env: String,
    pub secret_key_env: String,
    pub max_daily_loss_usdt: f64,
    pub max_position_btc: f64,
    pub cooldown_ms: u64,
}

impl LiveTradingConfig {
    pub fn api_key(&self) -> Result<String, Box<dyn std::error::Error>> {
        std::env::var(&self.api_key_env)
            .map_err(|_| format!("env var {} not set", self.api_key_env).into())
    }

    pub fn secret_key(&self) -> Result<String, Box<dyn std::error::Error>> {
        std::env::var(&self.secret_key_env)
            .map_err(|_| format!("env var {} not set", self.secret_key_env).into())
    }
}

#[derive(Debug, Deserialize)]
pub struct MonitorConfig {
    pub refresh_ms: u64,
    pub feishu_webhook: Option<String>,
}

impl Config {
    pub fn load(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    /// Taker fee as a ratio (e.g. 0.001 for 10 bps)
    pub fn taker_fee_ratio(&self) -> f64 {
        self.trading.taker_fee_bps / 10000.0
    }

    /// FDUSD maker fee as a ratio
    pub fn fdusd_maker_fee_ratio(&self) -> f64 {
        self.trading.fdusd_maker_fee_bps / 10000.0
    }

    /// Minimum profit ratio to trigger a trade
    pub fn min_profit_ratio(&self) -> f64 {
        self.trading.min_profit_bps / 10000.0
    }
}

use hmac::{Hmac, Mac};
use reqwest::header::{HeaderMap, HeaderValue};
use serde::Deserialize;
use sha2::Sha256;
use tracing::{error, info};

type HmacSha256 = Hmac<Sha256>;

const BASE_URL: &str = "https://api.binance.com";

#[derive(Clone)]
pub struct BinanceClient {
    client: reqwest::Client,
    api_key: String,
    secret_key: String,
}

#[derive(Debug, Deserialize)]
pub struct OrderResponse {
    pub symbol: String,
    #[serde(rename = "orderId")]
    pub order_id: u64,
    pub status: String,
    #[serde(rename = "executedQty")]
    pub executed_qty: String,
    #[serde(rename = "cummulativeQuoteQty")]
    pub cumulative_quote_qty: String,
    pub side: String,
    #[serde(rename = "type")]
    pub order_type: String,
}

#[derive(Debug, Deserialize)]
pub struct AccountInfo {
    pub balances: Vec<BalanceInfo>,
}

#[derive(Debug, Deserialize)]
pub struct BalanceInfo {
    pub asset: String,
    pub free: String,
    pub locked: String,
}

#[derive(Debug, Deserialize)]
pub struct ApiError {
    pub code: i64,
    pub msg: String,
}

impl BinanceClient {
    pub fn new(api_key: String, secret_key: String, proxy: Option<&str>) -> Self {
        let mut builder = reqwest::Client::builder();
        if let Some(proxy_url) = proxy {
            if let Ok(p) = reqwest::Proxy::all(proxy_url) {
                builder = builder.proxy(p);
            }
        }
        let client = builder.build().unwrap_or_else(|_| reqwest::Client::new());
        Self {
            client,
            api_key,
            secret_key,
        }
    }

    fn sign(&self, query_string: &str) -> String {
        let mut mac =
            HmacSha256::new_from_slice(self.secret_key.as_bytes()).expect("HMAC key error");
        mac.update(query_string.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }

    fn headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            "X-MBX-APIKEY",
            HeaderValue::from_str(&self.api_key).expect("invalid API key"),
        );
        headers
    }

    fn timestamp_ms() -> u64 {
        chrono::Utc::now().timestamp_millis() as u64
    }

    /// Place a market order
    pub async fn place_market_order(
        &self,
        symbol: &str,
        side: &str,
        quantity: f64,
    ) -> Result<OrderResponse, Box<dyn std::error::Error + Send + Sync>> {
        let timestamp = Self::timestamp_ms();
        let qty_str = format!("{:.6}", quantity);
        let query = format!(
            "symbol={}&side={}&type=MARKET&quantity={}&recvWindow=5000&timestamp={}",
            symbol, side, qty_str, timestamp
        );
        let signature = self.sign(&query);
        let url = format!("{}/api/v3/order?{}&signature={}", BASE_URL, query, signature);

        info!("Placing order: {} {} {} {}", side, qty_str, symbol, "MARKET");

        let resp = self
            .client
            .post(&url)
            .headers(self.headers())
            .send()
            .await?;

        let status = resp.status();
        let body = resp.text().await?;

        if !status.is_success() {
            let api_err: ApiError =
                serde_json::from_str(&body).unwrap_or(ApiError { code: -1, msg: body.clone() });
            error!("Order failed: code={} msg={}", api_err.code, api_err.msg);
            return Err(format!("Binance API error {}: {}", api_err.code, api_err.msg).into());
        }

        let order: OrderResponse = serde_json::from_str(&body)?;
        info!(
            "Order filled: id={} status={} qty={} quote={}",
            order.order_id, order.status, order.executed_qty, order.cumulative_quote_qty
        );
        Ok(order)
    }

    /// Get account balances
    pub async fn get_account(
        &self,
    ) -> Result<AccountInfo, Box<dyn std::error::Error + Send + Sync>> {
        let timestamp = Self::timestamp_ms();
        let query = format!("recvWindow=5000&timestamp={}", timestamp);
        let signature = self.sign(&query);
        let url = format!(
            "{}/api/v3/account?{}&signature={}",
            BASE_URL, query, signature
        );

        let resp = self
            .client
            .get(&url)
            .headers(self.headers())
            .send()
            .await?;

        let status = resp.status();
        let body = resp.text().await?;

        if !status.is_success() {
            let api_err: ApiError =
                serde_json::from_str(&body).unwrap_or(ApiError { code: -1, msg: body.clone() });
            return Err(format!("Binance API error {}: {}", api_err.code, api_err.msg).into());
        }

        let account: AccountInfo = serde_json::from_str(&body)?;
        Ok(account)
    }

    /// Get balance for a specific asset
    pub async fn get_balance(
        &self,
        asset: &str,
    ) -> Result<f64, Box<dyn std::error::Error + Send + Sync>> {
        let account = self.get_account().await?;
        let balance = account
            .balances
            .iter()
            .find(|b| b.asset == asset)
            .map(|b| b.free.parse::<f64>().unwrap_or(0.0))
            .unwrap_or(0.0);
        Ok(balance)
    }
}

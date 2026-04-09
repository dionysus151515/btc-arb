use serde::Deserialize;

/// A single price level in the order book: [price, quantity]
#[derive(Debug, Clone)]
pub struct PriceLevel {
    pub price: f64,
    pub qty: f64,
}

/// Binance partial depth stream message
#[derive(Debug, Deserialize)]
pub struct DepthStreamMsg {
    #[serde(rename = "lastUpdateId")]
    pub last_update_id: u64,
    pub bids: Vec<(String, String)>,
    pub asks: Vec<(String, String)>,
}

/// Combined stream wrapper
#[derive(Debug, Deserialize)]
pub struct CombinedStreamMsg {
    pub stream: String,
    pub data: DepthStreamMsg,
}

impl DepthStreamMsg {
    pub fn parse_bids(&self) -> Vec<PriceLevel> {
        self.bids
            .iter()
            .filter_map(|(p, q)| {
                Some(PriceLevel {
                    price: p.parse().ok()?,
                    qty: q.parse().ok()?,
                })
            })
            .collect()
    }

    pub fn parse_asks(&self) -> Vec<PriceLevel> {
        self.asks
            .iter()
            .filter_map(|(p, q)| {
                Some(PriceLevel {
                    price: p.parse().ok()?,
                    qty: q.parse().ok()?,
                })
            })
            .collect()
    }
}

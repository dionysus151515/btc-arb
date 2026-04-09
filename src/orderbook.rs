use crate::binance::models::PriceLevel;

/// Maintains a local order book snapshot for one symbol
#[derive(Debug, Clone)]
pub struct OrderBook {
    pub symbol: String,
    pub bids: Vec<PriceLevel>, // sorted descending by price (best bid first)
    pub asks: Vec<PriceLevel>, // sorted ascending by price (best ask first)
    pub last_update_id: u64,
    pub update_time_ms: i64,
}

impl OrderBook {
    pub fn new(symbol: &str) -> Self {
        Self {
            symbol: symbol.to_string(),
            bids: Vec::new(),
            asks: Vec::new(),
            last_update_id: 0,
            update_time_ms: 0,
        }
    }

    pub fn update(&mut self, bids: Vec<PriceLevel>, asks: Vec<PriceLevel>, update_id: u64, time_ms: i64) {
        self.bids = bids;
        self.asks = asks;
        self.last_update_id = update_id;
        self.update_time_ms = time_ms;
    }

    /// Best bid (highest buy price)
    pub fn best_bid(&self) -> Option<&PriceLevel> {
        self.bids.first()
    }

    /// Best ask (lowest sell price)
    pub fn best_ask(&self) -> Option<&PriceLevel> {
        self.asks.first()
    }

    /// Calculate how much BTC can be bought at a given USDT budget, walking the ask book
    pub fn simulate_buy(&self, budget: f64) -> Option<(f64, f64)> {
        let mut remaining = budget;
        let mut total_btc = 0.0;
        let mut avg_price = 0.0;

        for level in &self.asks {
            let level_cost = level.price * level.qty;
            if remaining >= level_cost {
                total_btc += level.qty;
                remaining -= level_cost;
            } else {
                let partial_qty = remaining / level.price;
                total_btc += partial_qty;
                remaining = 0.0;
            }
            if remaining <= 0.0 {
                break;
            }
        }

        if total_btc > 0.0 {
            avg_price = (budget - remaining) / total_btc;
            Some((total_btc, avg_price))
        } else {
            None
        }
    }

    /// Calculate how much quote currency received by selling a given BTC amount, walking the bid book
    pub fn simulate_sell(&self, btc_amount: f64) -> Option<(f64, f64)> {
        let mut remaining_btc = btc_amount;
        let mut total_quote = 0.0;

        for level in &self.bids {
            if remaining_btc >= level.qty {
                total_quote += level.price * level.qty;
                remaining_btc -= level.qty;
            } else {
                total_quote += level.price * remaining_btc;
                remaining_btc = 0.0;
            }
            if remaining_btc <= 0.0 {
                break;
            }
        }

        let filled_btc = btc_amount - remaining_btc;
        if filled_btc > 0.0 {
            let avg_price = total_quote / filled_btc;
            Some((total_quote, avg_price))
        } else {
            None
        }
    }
}

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;

use crate::strategy::PaperTrade;

pub struct CsvLogger {
    file: File,
}

impl CsvLogger {
    pub fn new(path: &str) -> Result<Self, std::io::Error> {
        let exists = Path::new(path).exists();
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;

        let mut logger = Self { file };
        if !exists {
            logger.write_header()?;
        }
        Ok(logger)
    }

    fn write_header(&mut self) -> Result<(), std::io::Error> {
        writeln!(
            self.file,
            "timestamp,direction,btc_qty,buy_price,sell_price,gross_profit_usdt,fees_usdt,net_profit_usdt"
        )
    }

    pub fn log_trade(&mut self, trade: &PaperTrade) -> Result<(), std::io::Error> {
        writeln!(
            self.file,
            "{},{},{:.8},{:.2},{:.2},{:.6},{:.6},{:.6}",
            trade.timestamp.to_rfc3339(),
            trade.direction,
            trade.btc_qty,
            trade.buy_price,
            trade.sell_price,
            trade.gross_profit_usdt,
            trade.fees_usdt,
            trade.net_profit_usdt,
        )
    }
}

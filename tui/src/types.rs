use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

pub type Side = Vec<(f64, f64)>;

const MAX_CANDLES: usize = 500;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct BookSnapshot {
    pub bids: Side,
    pub asks: Side,
}

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct Candle {
    pub timestamp: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub is_closed: bool,
}

impl Candle {
    pub fn is_bullish(&self) -> bool {
        self.close >= self.open
    }
}

#[derive(Default)]
pub struct Candles(VecDeque<Candle>);

impl Candles {
    /// Replaces newest candle on timestamp match, else appends and drops oldest.
    pub fn upsert(&mut self, candle: Candle) {
        if let Some(existing) = self.0.back_mut().filter(|c| c.timestamp == candle.timestamp) {
            *existing = candle;
            return;
        }

        self.0.push_back(candle);
        if self.0.len() > MAX_CANDLES {
            self.0.pop_front();
        }
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn last_price(&self) -> f64 {
        self.0.back().map(|c| c.close).unwrap_or(0.0)
    }

    /// Percent move from the oldest candle's open to the latest close.
    pub fn pct_change(&self) -> f64 {
        let (Some(first), Some(last)) = (self.0.front(), self.0.back()) else {
            return 0.0;
        };

        if first.open == 0.0 {
            return 0.0;
        }

        (last.close - first.open) / first.open * 100.0
    }

    pub fn iter(&self) -> impl Iterator<Item = &Candle> {
        self.0.iter()
    }
}

impl From<Vec<Candle>> for Candles {
    fn from(value: Vec<Candle>) -> Self {
        Candles(value.into())
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    BookSnapshot(BookSnapshot),
    Candle(Candle),
    CandleSnapshot(Vec<Candle>),
}

pub struct SideSnapshot {
    pub max_cumulative: f64,
    pub depths: Vec<(f64, f64, f64)>, // (price, amount, row_cumulative)
}

#[derive(Default)]
pub struct Orderbook {
    bids: Side,
    asks: Side,
}

impl Orderbook {
    pub fn bids(&self, limit: usize) -> SideSnapshot {
        self.snapshot_side(&self.bids, limit)
    }

    pub fn asks(&self, limit: usize) -> SideSnapshot {
        let mut snapshot = self.snapshot_side(&self.asks, limit);
        snapshot.depths.reverse();
        snapshot
    }

    fn snapshot_side(&self, side: &Side, limit: usize) -> SideSnapshot {
        let mut running = 0.0;
        let depths: Vec<(f64, f64, f64)> = side
            .iter()
            .take(limit)
            .map(|&(p, q)| {
                running += q;
                (p, q, running)
            })
            .collect();
        SideSnapshot {
            max_cumulative: running,
            depths,
        }
    }

    pub fn mid(&self) -> Option<f64> {
        self.bbo().map(|(bid, ask)| (bid + ask) / 2.0)
    }

    pub fn spread(&self) -> Option<f64> {
        self.bbo().map(|(bid, ask)| ask - bid)
    }

    fn bbo(&self) -> Option<(f64, f64)> {
        let best_bid = self.bids.iter().map(|(p, _)| *p).fold(f64::MIN, |a, b| a.max(b));

        let best_ask = self.asks.iter().map(|(p, _)| *p).fold(f64::MAX, |a, b| a.min(b));

        if best_bid > f64::MIN && best_ask < f64::MAX {
            Some((best_bid, best_ask))
        } else {
            None
        }
    }

    pub fn apply_depth(&mut self, depth: BookSnapshot) {
        self.bids = depth.bids;
        self.asks = depth.asks;
    }
}

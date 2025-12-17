use crate::l2_book;
use crate::l2_book::types::f64_to_u64;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct DepthUpdate<'a> {
    #[serde(rename = "e")]
    #[serde(borrow)]
    pub event_type: std::borrow::Cow<'a, str>,

    #[serde(rename = "s")]
    #[serde(borrow)]
    pub symbol: std::borrow::Cow<'a, str>,

    #[serde(rename = "b")]
    pub bids: Vec<PriceSize>,

    #[serde(rename = "a")]
    pub asks: Vec<PriceSize>,

    #[serde(flatten)]
    pub seq: DepthUpdateSeq,
}

impl<'a> From<&DepthUpdate<'a>> for DepthUpdateSeq {
    fn from(val: &DepthUpdate<'a>) -> Self {
        val.seq
    }
}

impl<'a> From<DepthUpdate<'a>> for l2_book::Order<DepthUpdateSeq> {
    fn from(val: DepthUpdate<'a>) -> Self {
        let bids = val.bids.iter().cloned().map(Into::into).collect();
        let asks = val.asks.iter().cloned().map(Into::into).collect();
        let seq = val.seq;

        l2_book::Order {
            id: l2_book::Sequence(seq.last_update_id),
            bids,
            asks,
            is_snapshot: false,
            ts_ms: val.seq.transaction_time_ms,
            o: seq,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DepthUpdateSeq {
    #[serde(rename = "U")]
    pub first_update_id: u64,

    #[serde(rename = "u")]
    pub last_update_id: u64,

    #[serde(rename = "pu")]
    pub previous_update_id: u64,

    #[serde(rename = "E")]
    pub event_time_ms: u64,

    #[serde(rename = "T")]
    pub transaction_time_ms: u64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DepthSnapshot {
    #[serde(rename = "lastUpdateId")]
    pub last_update_id: u64,

    #[serde(rename = "E")]
    pub event_time_ms: u64,

    #[serde(rename = "T")]
    pub transaction_time_ms: u64,

    pub bids: Vec<PriceSize>,
    pub asks: Vec<PriceSize>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PriceSize(
    #[serde(with = "f64_to_u64")] pub u64, // price
    #[serde(with = "f64_to_u64")] pub u64, // size
);

impl From<PriceSize> for l2_book::PriceSize {
    fn from(val: PriceSize) -> Self {
        l2_book::PriceSize(l2_book::Price(val.0), l2_book::Size(val.1))
    }
}

#[cfg(test)]
mod test {
    use crate::binance::types::{DepthUpdate, PriceSize};

    #[test]
    fn deserialize_depth_update() {
        use crate::l2_book::types::FLOAT_SCALE;
        let d = r#"{"e":"depthUpdate","E":1571889248277,"T":1571889248276,"s":"BTCUSDT","U":390497796,"u":390497878,"pu":390497794,"b":[["7403.89","0.002"],["7403.90","3.906"],["7404.00","1.428"]],"a":[["7405.96","3.340"],["7406.63","4.525"],["7407.08","2.475"]]}"#;
        let depth_update: DepthUpdate = serde_json::from_str(d).unwrap();

        assert_eq!(depth_update.event_type, "depthUpdate");
        assert_eq!(depth_update.seq.event_time_ms, 1571889248277);
        assert_eq!(depth_update.seq.transaction_time_ms, 1571889248276);
        assert_eq!(depth_update.symbol, "BTCUSDT");

        let expected_bids = vec![
            PriceSize((7403.89 * FLOAT_SCALE) as u64, (0.002 * FLOAT_SCALE) as u64),
            PriceSize((7403.90 * FLOAT_SCALE) as u64, (3.906 * FLOAT_SCALE) as u64),
            PriceSize((7404.00 * FLOAT_SCALE) as u64, (1.428 * FLOAT_SCALE) as u64),
        ];

        let expected_asks = vec![
            PriceSize((7405.96 * FLOAT_SCALE) as u64, (3.340 * FLOAT_SCALE) as u64),
            PriceSize((7406.63 * FLOAT_SCALE) as u64, (4.525 * FLOAT_SCALE) as u64),
            PriceSize((7407.08 * FLOAT_SCALE) as u64, (2.475 * FLOAT_SCALE) as u64),
        ];

        assert_eq!(depth_update.bids, expected_bids);
        assert_eq!(depth_update.asks, expected_asks);
    }
}

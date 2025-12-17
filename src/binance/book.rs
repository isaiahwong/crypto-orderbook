use super::types::DepthUpdateSeq;
use crate::binance::api::Rest;
use crate::binance::api::UM;
use crate::l2_book::tokio::{Book as AsyncBook, SnapshotFetcher};
use crate::l2_book::{BookSequencer, Order, PriceSize, Sequence};
use std::time::Duration;

struct BinanceBookSequencer;

impl BookSequencer<DepthUpdateSeq> for BinanceBookSequencer {
    fn is_first_event(&self, cur_seq: Sequence, update: &Order<DepthUpdateSeq>) -> bool {
        update.o.first_update_id <= cur_seq.val() && cur_seq.val() <= update.o.last_update_id
    }

    fn is_stale(&self, cur_seq: Sequence, update: &Order<DepthUpdateSeq>) -> bool {
        cur_seq.val() < update.o.first_update_id
    }

    fn is_next(&self, cur_seq: Sequence, update: &Order<DepthUpdateSeq>) -> bool {
        cur_seq.val() == update.o.previous_update_id
    }
}

pub struct BinanceSnapshotFetcher<A> {
    api: A,
}

impl<A: Rest + Sync> SnapshotFetcher<DepthUpdateSeq> for BinanceSnapshotFetcher<A> {
    type Error = A::Error;

    async fn fetch_snapshot(&self, symbol: &str) -> Result<Order<DepthUpdateSeq>, Self::Error> {
        let res = self.api.get_orderbook(symbol).await?;

        let bids = res.bids.into_iter().map(PriceSize::from).collect();
        let asks = res.asks.into_iter().map(PriceSize::from).collect();

        let seq = DepthUpdateSeq {
            first_update_id: res.last_update_id,
            last_update_id: res.last_update_id,
            previous_update_id: res.last_update_id,
            event_time_ms: res.event_time_ms,
            transaction_time_ms: res.transaction_time_ms,
        };

        Ok(Order {
            id: Sequence(res.last_update_id),
            bids,
            asks,
            is_snapshot: true,
            ts_ms: seq.transaction_time_ms,
            o: seq,
        })
    }
}

pub struct Book;

impl Book {
    pub fn new_um(symbol: impl Into<String>, depth: usize, interval: Duration) -> AsyncBook<DepthUpdateSeq> {
        AsyncBook::new(
            symbol.into(),
            BinanceBookSequencer,
            BinanceSnapshotFetcher { api: UM },
            depth,
            interval,
        )
    }
}

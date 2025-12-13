use super::types::DepthUpdateSeq;
use crate::binance::api::Rest;
use crate::binance::api::UM;
use crate::l2_book::tokio::{Book as AsyncBook, SnapshotFetcher};
use crate::l2_book::{BookSequence, Order, PriceSize, Sequence};

struct BinanceBookSequence;

impl BookSequence<DepthUpdateSeq> for BinanceBookSequence {
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
            event_time: res.event_time,
            transaction_time: res.transaction_time,
        };

        Ok(Order {
            id: Sequence(res.last_update_id),
            bids,
            asks,
            is_snapshot: true,
            o: seq,
        })
    }
}

pub struct Book;

impl Book {
    pub fn new_um(symbol: impl Into<String>) -> AsyncBook<DepthUpdateSeq> {
        AsyncBook::new(
            symbol.into(),
            BinanceBookSequence,
            BinanceSnapshotFetcher { api: UM },
        )
    }
}

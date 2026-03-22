use super::types::{Order, Sequence};
use crate::l2_book::types::{Price, Size, ZERO_SIZE};
use std::cmp::Reverse;
use std::collections::{BTreeMap, VecDeque};

pub trait BookSequencer<O> {
    fn is_first_event(&self, cur_seq: Sequence, update: &Order<O>) -> bool;
    fn is_stale(&self, cur_seq: Sequence, update: &Order<O>) -> bool;
    fn is_next(&self, cur_seq: Sequence, update: &Order<O>) -> bool;
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum BookAction {
    RetrieveSnapshot,
    Ok,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(dead_code)]
enum BookState {
    Init,
    WaitingForSnapshot,
    Synchronizing,
    Processing,
}

#[derive(Debug, Clone)]
pub struct BookSnapshot {
    pub asks: Vec<(Price, Size)>,
    pub bids: Vec<(Price, Size)>,
    pub ts_ms: u64,
}

impl BookSnapshot {
    pub fn mid(&self) -> Price {
        let best_bid = self.bids.first().map(|(p, _)| p.0).unwrap_or(0);
        let best_ask = self.asks.first().map(|(p, _)| p.0).unwrap_or(0);

        if best_bid == 0 || best_ask == 0 {
            return Price(0);
        }

        Price((best_bid + best_ask) / 2)
    }
}

pub struct BookFsm<O, S: BookSequencer<O>> {
    state: BookState,
    asks: BTreeMap<Price, Size>,
    bids: BTreeMap<Reverse<Price>, Size>,
    buffer: VecDeque<Order<O>>,
    cur_sequence: Sequence,
    sequencer: S,
    ts_ms: u64,
}

impl<O, S> BookFsm<O, S>
where
    S: BookSequencer<O>,
{
    pub fn new(sequencer: S) -> Self {
        Self {
            buffer: VecDeque::with_capacity(100),
            state: BookState::Init,
            asks: BTreeMap::new(),
            bids: BTreeMap::new(),
            cur_sequence: Sequence(0),
            sequencer,
            ts_ms: 0,
        }
    }

    pub fn snapshot(&self, depth: usize) -> BookSnapshot {
        BookSnapshot {
            asks: self.asks.iter().take(depth).map(|(&p, &s)| (p, s)).collect(),
            bids: self.bids.iter().take(depth).map(|(Reverse(p), &s)| (*p, s)).collect(),
            ts_ms: self.ts_ms,
        }
    }

    pub fn update(&mut self, order: Order<O>) -> BookAction {
        self.process_order(order)
    }

    fn process_order(&mut self, order: Order<O>) -> BookAction {
        match self.state {
            BookState::Init => {
                self.state = BookState::WaitingForSnapshot;
                BookAction::RetrieveSnapshot
            }
            BookState::WaitingForSnapshot => {
                if order.is_snapshot {
                    self.apply_order(&order);
                    self.state = BookState::Synchronizing;
                    self.drain_buffer()
                } else {
                    // Buffer until we get a snapshot
                    self.buffer.push_back(order);
                    BookAction::Ok
                }
            }
            BookState::Synchronizing => {
                if self.sequencer.is_first_event(self.cur_sequence, &order) {
                    self.apply_order(&order);
                    self.state = BookState::Processing;
                    return BookAction::Ok;
                }

                // Drops update if it's stale
                match self.sequencer.is_stale(self.cur_sequence, &order) {
                    false => BookAction::Ok,
                    true => self.reset(),
                }
            }
            BookState::Processing => match self.sequencer.is_next(self.cur_sequence, &order) {
                true => {
                    self.apply_order(&order);
                    BookAction::Ok
                }
                false => self.reset(),
            },
        }
    }

    fn drain_buffer(&mut self) -> BookAction {
        let mut action = BookAction::Ok;
        while let Some(buffered_order) = self.buffer.pop_front() {
            action = self.process_order(buffered_order);
            if action != BookAction::Ok {
                return action;
            }
        }

        action
    }

    fn reset(&mut self) -> BookAction {
        self.state = BookState::WaitingForSnapshot;
        self.buffer.clear();
        BookAction::RetrieveSnapshot
    }

    fn apply_order(&mut self, order: &Order<O>) {
        self.cur_sequence = order.id;
        self.ts_ms = order.ts_ms;

        if order.is_snapshot {
            self.asks.clear();
            self.bids.clear();
        }

        for pxsz in order.bids.iter() {
            match pxsz.size() {
                ZERO_SIZE => self.bids.remove(&Reverse(pxsz.price())),
                _ => self.bids.insert(Reverse(pxsz.price()), pxsz.size()),
            };
        }

        for pxsz in order.asks.iter() {
            match pxsz.size() {
                ZERO_SIZE => self.asks.remove(&pxsz.price()),
                _ => self.asks.insert(pxsz.price(), pxsz.size()),
            };
        }
    }
}

#[cfg(test)]
mod test {

    use super::*;
    use crate::l2_book::types::{Order, Sequence};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn test_basic() {
        let mut fsm = BookFsm::new(TestSequencer);

        // Send inc update, expect fsm to ask to retrieve snapshot
        assert_eq!(BookAction::RetrieveSnapshot, fsm.update(inc(2, 3, 5)));
        assert_eq!(BookState::WaitingForSnapshot, fsm.state);

        // Send inc update
        assert_eq!(BookAction::Ok, fsm.update(inc(5, 7, 10)));
        assert_eq!(BookState::WaitingForSnapshot, fsm.state);

        // Send snapshot
        assert_eq!(BookAction::Ok, fsm.update(snap(0, 0, 7)));
        assert_eq!(BookState::Processing, fsm.state);

        // Buffer drained
        assert_eq!(0, fsm.buffer.len());
    }

    #[test]
    fn test_first_event_not_found() {
        let mut fsm = BookFsm::new(TestSequencer);

        // Send inc update, expect fsm to ask to retrieve snapshot
        assert_eq!(BookAction::RetrieveSnapshot, fsm.update(inc(2, 3, 5)));
        assert_eq!(BookState::WaitingForSnapshot, fsm.state);

        // Send inc update
        assert_eq!(BookAction::Ok, fsm.update(inc(5, 7, 10)));
        assert_eq!(BookState::WaitingForSnapshot, fsm.state);

        // Send snapshot
        assert_eq!(BookAction::Ok, fsm.update(snap(0, 0, 11)));
        assert_eq!(BookState::Synchronizing, fsm.state);

        // Buffer drained
        assert_eq!(0, fsm.buffer.len());

        // First event not found, send an update after snapshot
        assert_eq!(BookAction::RetrieveSnapshot, fsm.update(inc(10, 13, 14)))
    }

    struct TestOrder {
        pub prev_seq: Sequence,
        pub start_seq: Sequence,
        pub end_seq: Sequence,
    }

    struct TestSequencer;

    impl BookSequencer<TestOrder> for TestSequencer {
        fn is_first_event(&self, cur_seq: Sequence, update: &Order<TestOrder>) -> bool {
            update.o.start_seq <= cur_seq && cur_seq <= update.o.end_seq
        }

        fn is_stale(&self, cur_seq: Sequence, update: &Order<TestOrder>) -> bool {
            cur_seq < update.o.start_seq
        }

        fn is_next(&self, cur_seq: Sequence, update: &Order<TestOrder>) -> bool {
            cur_seq == update.o.prev_seq
        }
    }

    fn mk_order(is_snapshot: bool, o: TestOrder) -> Order<TestOrder> {
        Order {
            id: o.end_seq,
            bids: vec![],
            asks: vec![],
            is_snapshot,
            ts_ms: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64,
            o,
        }
    }

    fn inc(prev: u64, start: u64, end: u64) -> Order<TestOrder> {
        mk_order(
            false,
            TestOrder {
                prev_seq: Sequence(prev),
                start_seq: Sequence(start),
                end_seq: Sequence(end),
            },
        )
    }

    fn snap(prev: u64, start: u64, end: u64) -> Order<TestOrder> {
        mk_order(
            true,
            TestOrder {
                prev_seq: Sequence(prev),
                start_seq: Sequence(start),
                end_seq: Sequence(end),
            },
        )
    }
}

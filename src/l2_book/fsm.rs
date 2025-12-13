use crate::l2_book::types::{Price, Size, ZERO_SIZE};

use super::types::{Order, Sequence};
use std::collections::{BTreeMap, VecDeque};

pub trait BookSequence<O> {
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

pub struct BookFsm<O, S>
where
    S: BookSequence<O>,
{
    state: BookState,
    asks: BTreeMap<Price, Size>,
    bids: BTreeMap<Price, Size>,
    buffer: VecDeque<Order<O>>,
    cur_sequence: Sequence,
    sequencer: S,
}

impl<O, S> BookFsm<O, S>
where
    S: BookSequence<O>,
{
    pub fn new(sequence: S) -> Self {
        Self {
            buffer: VecDeque::with_capacity(100),
            state: BookState::Init,
            asks: BTreeMap::new(),
            bids: BTreeMap::new(),
            cur_sequence: Sequence(0),
            sequencer: sequence,
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
        if order.is_snapshot {
            self.asks.clear();
            self.bids.clear();
        }

        for pxsz in order.bids.iter() {
            match pxsz.size() {
                ZERO_SIZE => self.bids.remove(&pxsz.price()),
                _ => self.bids.insert(pxsz.price(), pxsz.size()),
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

    impl BookSequence<TestOrder> for TestSequencer {
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

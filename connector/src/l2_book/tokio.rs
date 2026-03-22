use crate::l2_book::fsm::BookSnapshot;

use super::fsm::{BookAction, BookFsm, BookSequencer};
use super::types::Order;
use std::future::Future;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::time::Instant;

pub trait SnapshotFetcher<O> {
    type Error: std::fmt::Debug + Send;

    fn fetch_snapshot(&self, symbol: &str) -> impl Future<Output = Result<Order<O>, Self::Error>> + Send;
}

pub enum BookMessage<O> {
    Update(Order<O>),
    RequestSnapshot(oneshot::Sender<BookSnapshot>),
}

pub struct BookProcessor<O, S, F>
where
    S: BookSequencer<O>,
    F: SnapshotFetcher<O>,
{
    fsm: BookFsm<O, S>,
    fetcher: F,
    symbol: String,
    snapshot_at: Option<Instant>,
    book_msg_rx: mpsc::Receiver<BookMessage<O>>,
    book_snap_tx: mpsc::Sender<BookSnapshot>,
    depth: usize,
}

impl<O, S, F> BookProcessor<O, S, F>
where
    O: Send + 'static,
    S: BookSequencer<O> + Send + 'static,
    F: SnapshotFetcher<O> + Send + 'static,
{
    pub fn new(
        symbol: String,
        sequencer: S,
        fetcher: F,
        depth: usize,
        book_msg_rx: mpsc::Receiver<BookMessage<O>>,
        book_snap_tx: mpsc::Sender<BookSnapshot>,
    ) -> Self {
        Self {
            fsm: BookFsm::new(sequencer),
            fetcher,
            symbol,
            snapshot_at: None,
            book_msg_rx,
            book_snap_tx,
            depth,
        }
    }

    pub async fn run(mut self) {
        while let Some(msg) = self.book_msg_rx.recv().await {
            match msg {
                BookMessage::Update(order) => self.on_update(order).await,
                BookMessage::RequestSnapshot(tx) => {
                    let _ = tx.send(self.fsm.snapshot(self.depth));
                }
            }
        }
    }

    async fn on_update(&mut self, mut order: Order<O>) {
        while let BookAction::RetrieveSnapshot = self.fsm.update(order) {
            self.snapshot_at = None;

            match self.fetcher.fetch_snapshot(&self.symbol).await {
                Ok(snapshot) => order = snapshot,
                Err(e) => {
                    // TODO: think of how to handle error
                    eprintln!("failed to fetch: {:?}", e);
                    return;
                }
            };

            self.snapshot_at = Some(Instant::now());
        }

        // Publish
        match self.book_snap_tx.send(self.fsm.snapshot(self.depth)).await {
            Ok(_) => (),
            Err(_) => return,
        }
    }
}

/// Book handle that spawns a indefinite tokio future
pub struct Book<O> {
    book_msg_tx: mpsc::Sender<BookMessage<O>>,
    book_pub_rx: mpsc::Receiver<BookSnapshot>,
}

impl<O> Book<O>
where
    O: Send + 'static,
{
    pub fn new<S, F>(symbol: String, sequence: S, fetcher: F, depth: usize) -> Self
    where
        S: BookSequencer<O> + Send + 'static,
        F: SnapshotFetcher<O> + Send + 'static,
    {
        let (book_msg_tx, book_msg_rx) = mpsc::channel(50);
        let (book_pub_tx, book_pub_rx) = mpsc::channel(1000);

        let processor = BookProcessor::new(symbol, sequence, fetcher, depth, book_msg_rx, book_pub_tx);
        tokio::spawn(processor.run());

        Self { book_msg_tx, book_pub_rx }
    }

    pub async fn recv(&mut self) -> Option<BookSnapshot> {
        self.book_pub_rx.recv().await
    }

    pub fn writer(&self) -> BookWriter<O> {
        BookWriter {
            tx: self.book_msg_tx.clone(),
        }
    }
}

#[derive(Clone)]
pub struct BookWriter<O> {
    tx: mpsc::Sender<BookMessage<O>>,
}

impl<O> BookWriter<O>
where
    O: Send + 'static,
{
    pub async fn send(&self, order: Order<O>) {
        let _ = self.tx.send(BookMessage::Update(order)).await;
    }
}

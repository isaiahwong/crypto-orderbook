use super::fsm::{BookAction, BookFsm, BookSequence};
use super::types::Order;
use std::future::Future;
use tokio::sync::mpsc::{self, Receiver, Sender};

pub trait SnapshotFetcher<O> {
    type Error: std::fmt::Debug + Send;

    fn fetch_snapshot(
        &self,
        symbol: &str,
    ) -> impl Future<Output = Result<Order<O>, Self::Error>> + Send;
}

pub enum BookMessage<O> {
    Update(Order<O>),
}

pub struct BookProcessor<O, S, F>
where
    S: BookSequence<O>,
    F: SnapshotFetcher<O>,
{
    fsm: BookFsm<O, S>,
    rx: Receiver<BookMessage<O>>,
    fetcher: F,
    symbol: String,
}

impl<O, S, F> BookProcessor<O, S, F>
where
    O: Send + 'static,
    S: BookSequence<O> + Send + 'static,
    F: SnapshotFetcher<O> + Send + 'static,
{
    pub fn new(symbol: String, sequence: S, fetcher: F, rx: Receiver<BookMessage<O>>) -> Self {
        Self {
            fsm: BookFsm::new(sequence),
            rx,
            fetcher,
            symbol,
        }
    }

    pub async fn run(mut self) {
        while let Some(msg) = self.rx.recv().await {
            match msg {
                BookMessage::Update(order) => self.on_update(order).await,
            }
        }
    }

    async fn on_update(&mut self, mut order: Order<O>) {
        while let BookAction::RetrieveSnapshot = self.fsm.update(order) {
            match self.fetcher.fetch_snapshot(&self.symbol).await {
                Ok(snapshot) => order = snapshot,
                Err(e) => {
                    // TODO: think of how to handle error
                    eprintln!("failed to fetch: {:?}", e);
                    break;
                }
            }
        }
    }
}

/// Book handle that spawns a indefinite tokio future
#[derive(Clone)]
pub struct Book<O> {
    tx: Sender<BookMessage<O>>,
}

impl<O> Book<O>
where
    O: Send + 'static,
{
    pub fn new<S, F>(symbol: String, sequence: S, fetcher: F) -> Self
    where
        S: BookSequence<O> + Send + 'static,
        F: SnapshotFetcher<O> + Send + 'static,
    {
        let (tx, rx) = mpsc::channel(50);
        let processor = BookProcessor::new(symbol, sequence, fetcher, rx);
        tokio::spawn(processor.run());

        Self { tx }
    }

    pub async fn update(&self, order: Order<O>) {
        let _ = self.tx.send(BookMessage::Update(order)).await;
    }
}

mod candle;
mod dom;
mod types;
mod ui;
mod ws;

use rustls::crypto;
use tokio::sync::mpsc::channel;
use types::Message;
use ws::run;

const SYMBOL: &str = "btcusdt";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls ring provider");

    let (tx, rx) = channel::<Result<Message, anyhow::Error>>(100);

    // ws
    tokio::spawn(async move {
        loop {
            if let Err(e) = run(SYMBOL.to_string(), tx.clone()).await {
                eprintln!("WebSocket error: {:?}", e);
            }
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    });

    // tui
    ui::App::new(SYMBOL, rx)?.run().await?;

    Ok(())
}

use orderbook::binance::types::{DepthUpdate, DepthUpdateSeq};
use orderbook::ws::connect;
use serde_json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let _ = tokio_rustls::rustls::crypto::ring::default_provider().install_default();
    let url = "wss://fstream.binance.com/ws/btcusdt@depth";

    let mut ws = connect(url).await?;

    let book = orderbook::binance::Book::new_um("BTCUSDT");

    while let Some(res) = ws.rx.recv().await {
        let json = match res {
            Ok(msg) => msg,
            Err(e) => {
                eprintln!("Connection error: {}", e);
                break;
            }
        };

        match serde_json::from_slice::<DepthUpdate>(&json) {
            Ok(depth_update) => {
                let order: orderbook::l2_book::Order<DepthUpdateSeq> = depth_update.into();
                book.update(order).await;
            }
            Err(e) => {
                eprintln!("Error parsing message: {:?}", e);
            }
        }
    }

    Ok(())
}

use orderbook::binance::types::DepthUpdate;
use orderbook::ws::connect;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let _ = tokio_rustls::rustls::crypto::ring::default_provider().install_default();
    let url = "wss://fstream.binance.com/ws/btcusdt@depth";
    let mut ws = connect(url).await?;

    let mut book = orderbook::binance::Book::new_um("BTCUSDT", 1000, Duration::from_millis(0));

    let writer = book.writer();
    tokio::spawn(async move {
        loop {
            let Some(res) = ws.rx.recv().await else { break };

            let mut json = match res {
                Ok(msg) => msg,
                Err(e) => {
                    eprintln!("Connection error: {}", e);
                    break;
                }
            };

            match simd_json::from_slice::<DepthUpdate>(&mut json) {
                Ok(depth_update) => {
                    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;
                    let latency = now - depth_update.seq.event_time_ms;
                    println!("Received depth update latency: {}ms", latency);

                    writer.update(depth_update.into()).await;
                }
                Err(e) => {
                    eprintln!("Error parsing message: {:?}", e);
                }
            }
        }
    });

    while let Some(snapshot) = book.recv().await {
        let mid: f64 = snapshot.mid().into();
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;
        let latency = now - snapshot.ts_ms;
        println!("Received snapshot mid price: {:.2}, latency: {}ms", mid, latency);
    }

    Ok(())
}

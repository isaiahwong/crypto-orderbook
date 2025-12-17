# Crypto Orderbook

In-progress project serving as a launchpad for learning Rust and systems programming. The goal is to implement a runtime-agnostic, local-orderbook for crypto exchanges.

## Quick Start

The following example uses tokio for async runtime.
```rust
use orderbook::binance::Book;
use std::time::Duration;

#[tokio::main]
async fn main() {
    let depth = 5000;
    let interval = Duration::from_millis(100);
    let mut book = Book::new_um("BTCUSDT", depth, interval);

    let writer = book.writer();

    // send book updates
    writer.update(...); 

    while let Some(snapshot) = book.recv().await {
        let mid: f64 = snapshot.mid().into();
        println!("Mid Price: {}", mid));
    }
}
```

### Running Example

**serde-json**
```bash
cargo run --example binance_ws
```

**simd-json**
```bash
cargo run --example binance_ws_simd 
```

## Benchmarks

**Result**: `serde_json` can be faster for small payloads such as crypto websocket feed. 

### Running Benchmark
```bash
cargo bench
```

## Goals
- **Runtime Agnostic Design**: Decoupling the core orderbook logic from specific async runtimes.
- **Efficient Serialization**: Exploring efficient serialization techniques for WebSocket data streams, such as zero-copy and simd.



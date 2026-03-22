use criterion::{Criterion, criterion_group, criterion_main};
use connector::binance::types::{DepthUpdate, DepthUpdateSeq, PriceSize};
use serde::{Deserialize, Serialize};

/// Bids Asks as is, without [f64_to_u64] deserializer
#[derive(Serialize, Deserialize, Debug)]
pub struct DepthUpdateString<'a> {
    #[serde(rename = "e")]
    #[serde(borrow)]
    pub event_type: std::borrow::Cow<'a, str>,
    #[serde(rename = "s")]
    #[serde(borrow)]
    pub symbol: std::borrow::Cow<'a, str>,
    #[serde(rename = "b")]
    pub bids: Vec<(String, String)>,
    #[serde(rename = "a")]
    pub asks: Vec<(String, String)>,
    #[serde(flatten)]
    pub seq: DepthUpdateSeq,
}

/// benchmark for [f64_to_u64] deserializer
fn bench_f64_to_u64(c: &mut Criterion) {
    let raw_json = generate_json(50);
    let raw_bytes = raw_json.as_bytes();

    let mut serde_group = c.benchmark_group("serde_json");

    serde_group.bench_function("f64_to_u64", |b| {
        b.iter(|| {
            let _: DepthUpdate = serde_json::from_str(&raw_json).unwrap();
        })
    });

    serde_group.bench_function("strings", |b| {
        b.iter(|| {
            let _: DepthUpdateString = serde_json::from_str(&raw_json).unwrap();
        })
    });

    serde_group.finish();

    let mut simd_group = c.benchmark_group("simd_json");

    simd_group.bench_function("f64_to_u64", |b| {
        b.iter_batched(
            || raw_bytes.to_vec(),
            |mut data| {
                let _: DepthUpdate = simd_json::from_slice(&mut data).unwrap();
            },
            criterion::BatchSize::SmallInput,
        )
    });

    simd_group.bench_function("strings", |b| {
        b.iter_batched(
            || raw_bytes.to_vec(),
            |mut data| {
                let _: DepthUpdateString = simd_json::from_slice(&mut data).unwrap();
            },
            criterion::BatchSize::SmallInput,
        )
    });

    simd_group.finish();
}

/// Owned Strings for simd_json benchmark
#[derive(Serialize, Deserialize, Debug)]
pub struct DepthUpdateOwned {
    #[serde(rename = "e")]
    pub event_type: String,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "b")]
    pub bids: Vec<PriceSize>,
    #[serde(rename = "a")]
    pub asks: Vec<PriceSize>,
    #[serde(flatten)]
    pub seq: DepthUpdateSeq,
}

/// benchmark for owned String
fn bench_owned_string(c: &mut Criterion) {
    let raw_json = generate_json(1000);
    let raw_bytes = raw_json.as_bytes();

    let mut group = c.benchmark_group("owned_string");

    group.bench_function("serde_json::from_str", |b| {
        b.iter(|| {
            let _: DepthUpdateOwned = serde_json::from_str(&raw_json).unwrap();
        })
    });

    group.bench_function("simd_json::from_slice", |b| {
        b.iter_batched(
            || raw_bytes.to_vec(),
            |mut data| {
                let _: DepthUpdateOwned = simd_json::from_slice(&mut data).unwrap();
            },
            criterion::BatchSize::SmallInput,
        )
    });

    group.finish();
}

// Generates binance depth JSON
fn generate_json(n: i32) -> String {
    let mut bids = String::new();
    let mut asks = String::new();
    for i in 0..n {
        if i > 0 {
            bids.push(',');
            asks.push(',');
        }
        bids.push_str(&format!(r#"["{}.0", "1.0"]"#, 10000 + i));
        asks.push_str(&format!(r#"["{}.0", "1.0"]"#, 20000 + i));
    }

    format!(
        r#"{{"e":"depthUpdate","E":1571889248277,"T":1571889248276,"s":"BTCUSDT","U":390497796,"u":390497878,"pu":390497794,"b":[{}],"a":[{}]}}"#,
        bids, asks
    )
}

criterion_group!(benches, bench_f64_to_u64, bench_owned_string);
criterion_main!(benches);

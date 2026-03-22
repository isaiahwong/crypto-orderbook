#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Price(pub u64);
impl From<f64> for Price {
    fn from(val: f64) -> Self {
        Self((val * FLOAT_SCALE) as u64)
    }
}

impl From<Price> for f64 {
    fn from(val: Price) -> Self {
        val.0 as f64 / FLOAT_SCALE
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Size(pub u64);
impl From<f64> for Size {
    fn from(val: f64) -> Self {
        Self((val * FLOAT_SCALE) as u64)
    }
}

impl From<Size> for f64 {
    fn from(val: Size) -> Self {
        val.0 as f64 / FLOAT_SCALE
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PriceSize(pub Price, pub Size);
impl PriceSize {
    pub fn price(&self) -> Price {
        self.0
    }

    pub fn size(&self) -> Size {
        self.1
    }
}

pub const ZERO_SIZE: Size = Size(0);

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Sequence(pub u64);
impl Sequence {
    pub fn val(&self) -> u64 {
        self.0
    }
}

pub struct Order<O> {
    pub id: Sequence,
    pub bids: Vec<PriceSize>,
    pub asks: Vec<PriceSize>,
    pub is_snapshot: bool,
    pub ts_ms: u64,
    pub o: O,
}

pub const FLOAT_SCALE: f64 = 10_000_000_000.0;

/// Serializer and Deserializer for converting float to u64
/// Currently limited to precision of 1e10.
pub mod f64_to_u64 {
    use super::FLOAT_SCALE;
    use serde::{Deserialize, Deserializer, Serializer, de};

    trait ToU64<E: de::Error> {
        fn to_u64(self) -> Result<u64, E>;
    }

    impl<D: de::Error> ToU64<D> for f64 {
        fn to_u64(self) -> Result<u64, D> {
            let n = (self * FLOAT_SCALE).floor();
            if !n.is_finite() || n < 0.0 || n > u64::MAX as f64 {
                return Err(de::Error::custom("cannot convert to u64, invalid float"));
            }
            Ok(n as u64)
        }
    }

    pub fn serialize<S>(value: &u64, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let f = (*value as f64) / FLOAT_SCALE;
        serializer.serialize_f64(f)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<u64, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum NumOrStr<'a> {
            Str(&'a str),
            Float(f64),
        }

        match NumOrStr::deserialize(deserializer)? {
            NumOrStr::Str(s) => {
                let f: f64 = s.parse().map_err(de::Error::custom)?;
                f.to_u64()
            }
            NumOrStr::Float(f) => f.to_u64(),
        }
    }
}

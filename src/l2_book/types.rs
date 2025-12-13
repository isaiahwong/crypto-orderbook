#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Price(pub u64);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Size(pub u64);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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
    pub o: O,
}

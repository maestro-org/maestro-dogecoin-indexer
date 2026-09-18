use std::fmt::{self, Display, Formatter};

use dune::Dune;
use terms::Terms;

pub mod chain;
pub mod dune;
pub mod dune_id;
pub mod dunestone;
pub mod edict;
pub mod etching;
pub mod flag;
pub mod mint;
pub mod pile;
pub mod spaced_dune;
pub mod tag;
pub mod terms;
pub mod varint;

pub(crate) const CLAIM_BIT: u128 = 1 << 48;
pub const MAX_DIVISIBILITY: u8 = 38;
pub(crate) const MAX_LIMIT: u128 = u64::MAX as u128;
const RESERVED: u128 = 6402364363415443603228541259936211926;

#[derive(Debug, PartialEq)]
pub enum MintError {
    Cap(u128),
    End(u64),
    Start(u64),
    Unmintable,
}

impl Display for MintError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            MintError::Cap(cap) => write!(f, "limited to {cap} mints"),
            MintError::End(end) => write!(f, "mint ended on block {end}"),
            MintError::Start(start) => write!(f, "mint starts on block {start}"),
            MintError::Unmintable => write!(f, "not mintable"),
        }
    }
}

pub fn claim(id: u128) -> Option<u128> {
    (id & CLAIM_BIT != 0).then_some(id ^ CLAIM_BIT)
}

pub struct Allocation {
    pub balance: u128,
    pub divisibility: u8,
    pub id: u128,
    pub mint: Option<Terms>,
    pub dune: Dune,
    pub premine: u128,
    pub spacers: u32,
    pub symbol: Option<char>,
    pub turbo: bool,
}

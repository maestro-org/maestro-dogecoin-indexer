use serde::{Deserialize, Serialize};

use super::*;

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub(crate) struct Pile {
    pub(crate) amount: u128,
    pub(crate) divisibility: u8,
    pub(crate) symbol: Option<char>,
}

impl Display for Pile {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let cutoff = 10u128.pow(self.divisibility.into());

        let whole = self.amount / cutoff;
        let mut fractional = self.amount % cutoff;

        if fractional == 0 {
            write!(f, "{whole}")?;
        } else {
            let mut width = usize::from(self.divisibility);
            while fractional % 10 == 0 {
                fractional /= 10;
                width -= 1;
            }

            write!(f, "{whole}.{fractional:0>width$}")?;
        }

        write!(f, "\u{A0}{}", self.symbol.unwrap_or('¤'))?;

        Ok(())
    }
}

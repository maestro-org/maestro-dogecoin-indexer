use std::str::FromStr;

use anyhow::{anyhow, bail};
use chain::Chain;
use ordinals::Height;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::inscriptions::inscription_id::DeserializeFromStr;

use super::*;

const SUBSIDY_HALVING_INTERVAL_10X: u32 =
    bitcoin::blockdata::constants::SUBSIDY_HALVING_INTERVAL * 10;

#[derive(Default, Debug, PartialEq, Copy, Clone, PartialOrd, Ord, Eq)]
pub struct Dune(pub u128);

impl Dune {
    const STEPS: &'static [u128] = &[
        0,
        26,
        702,
        18278,
        475254,
        12356630,
        321272406,
        8353082582,
        217180147158,
        5646683826134,
        146813779479510,
        3817158266467286,
        99246114928149462,
        2580398988131886038,
        67090373691429037014,
        1744349715977154962390,
        45353092615406029022166,
        1179180408000556754576342,
        30658690608014475618984918,
        797125955808376366093607894,
        20725274851017785518433805270,
        538857146126462423479278937046,
        14010285799288023010461252363222,
        364267430781488598271992561443798,
        9470953200318703555071806597538774,
        246244783208286292431866971536008150,
        6402364363415443603228541259936211926,
        166461473448801533683942072758341510102,
    ];

    pub(crate) fn minimum_at_height(chain: Chain, height: Height) -> Self {
        let offset = height.0.saturating_add(1);

        const INTERVAL: u32 = SUBSIDY_HALVING_INTERVAL_10X / 12;

        let start = chain.first_dune_height();

        let end = start + SUBSIDY_HALVING_INTERVAL_10X;

        if offset < start {
            return Dune(Self::STEPS[12]);
        }

        if offset >= end {
            return Dune(0);
        }

        let progress = offset.saturating_sub(start);

        let length = 12u32.saturating_sub((progress as u32) / INTERVAL);

        let end = Self::STEPS[usize::try_from(length - 1).unwrap()];

        let start = Self::STEPS[usize::try_from(length).unwrap()];

        let remainder = u128::from((progress as u32) % INTERVAL);

        Dune(start - ((start - end) * remainder / u128::from(INTERVAL)))
    }

    pub(crate) fn is_reserved(self) -> bool {
        self.0 >= RESERVED
    }

    pub(crate) fn reserved(n: u128) -> Self {
        Dune(RESERVED.checked_add(n).unwrap())
    }
}

impl Serialize for Dune {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Dune {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(DeserializeFromStr::deserialize(deserializer)?.0)
    }
}

impl Display for Dune {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let mut n = self.0;
        if n == u128::max_value() {
            return write!(f, "BCGDENLQRQWDSLRUGSNLBTMFIJAV");
        }

        n += 1;
        let mut symbol = String::new();
        while n > 0 {
            symbol.push(
                "ABCDEFGHIJKLMNOPQRSTUVWXYZ"
                    .chars()
                    .nth(((n - 1) % 26) as usize)
                    .unwrap(),
            );
            n = (n - 1) / 26;
        }

        for c in symbol.chars().rev() {
            write!(f, "{c}")?;
        }

        Ok(())
    }
}

impl FromStr for Dune {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        let mut x = 0u128;
        for (i, c) in s.chars().enumerate() {
            if i > 0 {
                x += 1;
            }
            x = x.checked_mul(26).ok_or_else(|| anyhow!("out of range"))?;
            match c {
                'A'..='Z' => {
                    x = x
                        .checked_add(c as u128 - 'A' as u128)
                        .ok_or_else(|| anyhow!("out of range"))?;
                }
                _ => bail!("invalid character in dune name: {c}"),
            }
        }
        Ok(Dune(x))
    }
}

use std::str::FromStr;

use anyhow::{bail, Context};
use dune::Dune;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::storage::inscriptions::inscription_id::DeserializeFromStr;

use super::*;

#[derive(Copy, Clone, Debug, PartialEq, Ord, PartialOrd, Eq)]
pub struct SpacedDune {
    pub(crate) dune: Dune,
    pub(crate) spacers: u32,
}

impl SpacedDune {
    pub fn new(dune: Dune, spacers: u32) -> Self {
        Self { dune, spacers }
    }
}

impl FromStr for SpacedDune {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut dune = String::new();
        let mut spacers = 0u32;

        for c in s.chars() {
            match c {
                'A'..='Z' => dune.push(c),
                '.' | '•' => {
                    let flag = 1 << dune.len().checked_sub(1).context("leading spacer")?;
                    if spacers & flag != 0 {
                        bail!("double spacer");
                    }
                    spacers |= flag;
                }
                _ => bail!("invalid character"),
            }
        }

        if 32 - spacers.leading_zeros() >= dune.len().try_into().unwrap() {
            bail!("trailing spacer")
        }

        Ok(SpacedDune {
            dune: dune.parse()?,
            spacers,
        })
    }
}

impl Display for SpacedDune {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let dune = self.dune.to_string();

        for (i, c) in dune.chars().enumerate() {
            write!(f, "{c}")?;

            if i < dune.len() - 1 && self.spacers & 1 << i != 0 {
                write!(f, "•")?;
            }
        }

        Ok(())
    }
}

impl Serialize for SpacedDune {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for SpacedDune {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(DeserializeFromStr::deserialize(deserializer)?.0)
    }
}

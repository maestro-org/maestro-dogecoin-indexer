use crate::inscriptions::inscription_id::DeserializeFromStr;
use anyhow::anyhow;

use {
    serde::{Deserialize, Deserializer, Serialize, Serializer},
    std::{
        fmt::{self, Display, Formatter},
        num::TryFromIntError,
        str::FromStr,
    },
};

#[derive(Debug, PartialEq, Copy, Clone, Hash, Eq, Ord, PartialOrd)]
pub struct DuneId {
    pub height: u64,
    pub index: u32,
}

impl TryFrom<u128> for DuneId {
    type Error = TryFromIntError;

    fn try_from(n: u128) -> Result<Self, Self::Error> {
        Ok(Self {
            height: u64::try_from(n >> 16)?,
            index: u32::try_from(n & 0xFFFF).unwrap(),
        })
    }
}

impl From<DuneId> for u128 {
    fn from(id: DuneId) -> Self {
        u128::from(id.height) << 16 | u128::from(id.index)
    }
}

impl Display for DuneId {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}:{}", self.height, self.index,)
    }
}

impl FromStr for DuneId {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (height, index) = s
            .split_once(':')
            .ok_or_else(|| anyhow!("invalid dune ID: {s}"))?;

        Ok(Self {
            height: height.parse()?,
            index: index.parse()?,
        })
    }
}

impl Serialize for DuneId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for DuneId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(DeserializeFromStr::deserialize(deserializer)?.0)
    }
}

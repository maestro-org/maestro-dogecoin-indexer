use bitcoin::BlockHash;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Point {
    pub height: u64,
    pub hash: BlockHash,
}

/// A serialization-friendly chain Point struct using a hex-encoded hash
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PointArg {
    Origin,
    Specific(u64, String),
}

impl From<Point> for PointArg {
    fn from(other: Point) -> Self {
        PointArg::Specific(other.height, other.hash.to_string())
    }
}

impl FromStr for PointArg {
    type Err = crate::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            x if s.contains(',') => {
                let mut parts: Vec<_> = x.split(',').collect();
                let height = parts
                    .remove(0)
                    .parse()
                    .map_err(|_| Self::Err::message("can't parse height number"))?;

                let hash = parts.remove(0).to_owned();
                Ok(PointArg::Specific(height, hash))
            }
            "origin" => Ok(PointArg::Origin),
            _ => Err(Self::Err::message(
                "Can't parse chain point value, expecting `height,hex-hash` format",
            )),
        }
    }
}

impl ToString for PointArg {
    fn to_string(&self) -> String {
        match self {
            PointArg::Origin => "origin".to_string(),
            PointArg::Specific(height, hash) => format!("{},{}", height, hash),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "type", content = "value")]
pub enum IntersectConfig {
    Tip,
    Origin,
    Point(u64, String),
}

impl IntersectConfig {
    pub fn get_point(&self) -> Option<Point> {
        match self {
            IntersectConfig::Point(height, hash) => Some(Point {
                height: *height,
                hash: BlockHash::from_str(hash).expect("valid block hash"),
            }),
            _ => None,
        }
    }
}

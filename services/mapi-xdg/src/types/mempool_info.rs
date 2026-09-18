use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr, PickFirst};
use utoipa::ToSchema;

#[serde_as]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub struct MempoolInfo {
    /// Total memory usage for the mempool (in bytes)
    #[schema(example = 82420)]
    pub bytes: u64,

    /// Maximum memory usage for the mempool (in bytes)
    #[serde(alias = "maxmempool")]
    #[schema(example = 300000000)]
    pub max_mempool: u64,

    /// The minimum fee rate (in DOGE/kB) for mempool transactions
    #[serde_as(as = "PickFirst<(DisplayFromStr, _)>")]
    #[serde(alias = "mempoolminfee")]
    #[schema(example = "0.0", value_type = String)]
    pub mempool_min_fee: f64,

    /// Number of transactions in the mempool
    #[schema(example = 23)]
    pub size: u64,

    /// Total usage of the mempool (in bytes)
    #[schema(example = 176512)]
    pub usage: u64,
}

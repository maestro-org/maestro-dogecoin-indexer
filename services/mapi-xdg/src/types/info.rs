use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::ToSchema;

use crate::util::deserialize_softforks;

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct BlockchainInfo {
    /// Current network name as defined in BIP70 (main, test, regtest)
    pub chain: String,
    /// The current number of blocks processed in the server
    pub blocks: u64,
    /// The current number of headers we have validated
    pub headers: u64,
    /// The hash of the currently best block
    #[serde(rename = "bestblockhash")]
    pub best_block_hash: String,
    /// The current difficulty
    pub difficulty: f64,
    /// Median time for the current best block
    #[serde(rename = "mediantime")]
    pub median_time: u64,
    /// Estimate of verification progress [0..1]
    #[serde(rename = "verificationprogress")]
    pub verification_progress: f64,
    /// Estimate of whether this node is in Initial Block Download mode
    #[serde(rename = "initialblockdownload")]
    pub initial_block_download: bool,
    /// Total amount of work in active chain, in hexadecimal
    #[serde(rename = "chainwork")]
    pub chain_work: String,
    /// The estimated size of the block and undo files on disk
    pub size_on_disk: u64,
    /// If the blocks are subject to pruning
    pub pruned: bool,
    /// Lowest-height complete block stored (only present if pruning is enabled)
    pub prune_height: Option<u64>,
    /// Whether automatic pruning is enabled (only present if pruning is enabled)
    pub automatic_pruning: Option<bool>,
    /// The target size used by pruning (only present if automatic pruning is enabled)
    pub prune_target_size: Option<u64>,
    /// Status of softforks in progress
    #[serde(deserialize_with = "deserialize_softforks")]
    pub softforks: HashMap<String, Softfork>,
    /// Any network and blockchain warnings.
    pub warnings: String,
}

impl From<bitcoincore_rpc_async::json::GetBlockchainInfoResult> for BlockchainInfo {
    fn from(value: bitcoincore_rpc_async::json::GetBlockchainInfoResult) -> Self {
        Self {
            chain: value.chain,
            blocks: value.blocks,
            headers: value.headers,
            best_block_hash: hex::encode(value.best_block_hash),
            difficulty: value.difficulty,
            median_time: value.median_time,
            verification_progress: value.verification_progress,
            initial_block_download: value.initial_block_download,
            chain_work: hex::encode(value.chain_work),
            size_on_disk: value.size_on_disk,
            pruned: value.pruned,
            prune_height: value.prune_height,
            automatic_pruning: value.automatic_pruning,
            prune_target_size: value.prune_target_size,
            softforks: value
                .softforks
                .into_iter()
                .map(|(k, v)| (k, v.into()))
                .collect(),
            warnings: value.warnings,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct Softfork {
    #[serde(rename = "type")]
    pub type_: Option<SoftforkType>,
    pub bip9: Option<Bip9SoftforkInfo>,
    pub height: Option<u32>,
    #[serde(default = "default_true")]
    pub active: bool,
}

fn default_true() -> bool {
    true
}

impl From<bitcoincore_rpc_async::json::Softfork> for Softfork {
    fn from(value: bitcoincore_rpc_async::json::Softfork) -> Self {
        Self {
            type_: Some(value.type_.into()),
            bip9: value.bip9.map(Into::into),
            height: value.height,
            active: value.active,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct Bip9SoftforkInfo {
    pub status: Bip9SoftforkStatus,
    pub bit: Option<u8>,
    // Can be -1 for 0.18.x inactive ones.
    pub start_time: i64,
    pub timeout: u64,
    pub since: u32,
    pub statistics: Option<Bip9SoftforkStatistics>,
}

impl From<bitcoincore_rpc_async::json::Bip9SoftforkInfo> for Bip9SoftforkInfo {
    fn from(value: bitcoincore_rpc_async::json::Bip9SoftforkInfo) -> Self {
        Self {
            status: value.status.into(),
            bit: value.bit,
            start_time: value.start_time,
            timeout: value.timeout,
            since: value.since,
            statistics: value.statistics.map(Into::into),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum SoftforkType {
    Buried,
    Bip9,
}

impl From<bitcoincore_rpc_async::json::SoftforkType> for SoftforkType {
    fn from(value: bitcoincore_rpc_async::json::SoftforkType) -> Self {
        match value {
            bitcoincore_rpc_async::json::SoftforkType::Buried => Self::Buried,
            bitcoincore_rpc_async::json::SoftforkType::Bip9 => Self::Bip9,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Bip9SoftforkStatus {
    Defined,
    Started,
    LockedIn,
    Active,
    Failed,
}

impl From<bitcoincore_rpc_async::json::Bip9SoftforkStatus> for Bip9SoftforkStatus {
    fn from(value: bitcoincore_rpc_async::json::Bip9SoftforkStatus) -> Self {
        match value {
            bitcoincore_rpc_async::json::Bip9SoftforkStatus::Defined => Self::Defined,
            bitcoincore_rpc_async::json::Bip9SoftforkStatus::Started => Self::Started,
            bitcoincore_rpc_async::json::Bip9SoftforkStatus::LockedIn => Self::LockedIn,
            bitcoincore_rpc_async::json::Bip9SoftforkStatus::Active => Self::Active,
            bitcoincore_rpc_async::json::Bip9SoftforkStatus::Failed => Self::Failed,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct Bip9SoftforkStatistics {
    pub period: u32,
    pub threshold: u32,
    pub elapsed: u32,
    pub count: u32,
    pub possible: bool,
}

impl From<bitcoincore_rpc_async::json::Bip9SoftforkStatistics> for Bip9SoftforkStatistics {
    fn from(value: bitcoincore_rpc_async::json::Bip9SoftforkStatistics) -> Self {
        Self {
            period: value.period,
            threshold: value.threshold,
            elapsed: value.elapsed,
            count: value.count,
            possible: value.possible,
        }
    }
}

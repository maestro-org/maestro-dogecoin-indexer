use bitcoincore_rpc_async::bitcoin::BlockHash;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{Address, TransactionId};

#[derive(PartialEq, Eq, Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct TransactionResult {
    #[serde(flatten)]
    pub info: WalletTxInfo,
    pub amount: Option<SignedAmount>,
    pub fee: Option<SignedAmount>,
    #[serde(default)]
    pub details: Vec<Detail>,
    pub hex: String,
}

impl From<bitcoincore_rpc_async::json::GetTransactionResult> for TransactionResult {
    fn from(value: bitcoincore_rpc_async::json::GetTransactionResult) -> Self {
        Self {
            info: value.info.into(),
            amount: Some(SignedAmount(value.amount.as_sat())),
            fee: value.fee.map(|x| SignedAmount(x.as_sat())),
            details: value.details.into_iter().map(Into::into).collect(),
            hex: hex::encode(value.hex),
        }
    }
}

#[derive(PartialEq, Eq, Clone, Debug, Serialize, Deserialize, ToSchema)]
pub enum Bip125Replaceable {
    Yes,
    No,
    Unknown,
}

impl From<bitcoincore_rpc_async::json::Bip125Replaceable> for Bip125Replaceable {
    fn from(value: bitcoincore_rpc_async::json::Bip125Replaceable) -> Self {
        match value {
            bitcoincore_rpc_async::json::Bip125Replaceable::Yes => Bip125Replaceable::Yes,
            bitcoincore_rpc_async::json::Bip125Replaceable::No => Bip125Replaceable::No,
            bitcoincore_rpc_async::json::Bip125Replaceable::Unknown => Bip125Replaceable::Unknown,
        }
    }
}

#[derive(PartialEq, Eq, Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct WalletTxInfo {
    pub confirmations: i32,
    pub blockhash: Option<BlockHash>,
    pub blockindex: Option<usize>,
    pub blocktime: Option<u64>,
    pub blockheight: Option<u32>,
    pub txid: TransactionId,
    pub time: u64,
    pub timereceived: Option<u64>,
    #[serde(rename = "bip125-replaceable")]
    pub bip125_replaceable: Option<Bip125Replaceable>,
}

impl From<bitcoincore_rpc_async::json::WalletTxInfo> for WalletTxInfo {
    fn from(value: bitcoincore_rpc_async::json::WalletTxInfo) -> Self {
        Self {
            confirmations: value.confirmations,
            blockhash: value.blockhash,
            blockindex: value.blockindex,
            blocktime: value.blocktime,
            blockheight: value.blockheight,
            txid: value.txid.into(),
            time: value.time,
            timereceived: Some(value.timereceived),
            bip125_replaceable: Some(value.bip125_replaceable.into()),
        }
    }
}

#[derive(PartialEq, Eq, Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct Detail {
    pub address: Option<Address>,
    pub category: DetailCategory,
    pub amount: SignedAmount,
    pub label: Option<String>,
    pub vout: u32,
    pub fee: Option<SignedAmount>,
    pub abandoned: Option<bool>,
}

impl From<bitcoincore_rpc_async::json::GetTransactionResultDetail> for Detail {
    fn from(value: bitcoincore_rpc_async::json::GetTransactionResultDetail) -> Self {
        Self {
            address: value.address.map(Into::into),
            category: value.category.into(),
            amount: SignedAmount(value.amount.as_sat()),
            label: value.label,
            vout: value.vout,
            fee: value.fee.map(|x| SignedAmount(x.as_sat())),
            abandoned: value.abandoned,
        }
    }
}

#[derive(PartialEq, Eq, Clone, Debug, Serialize, Deserialize, ToSchema)]
pub enum DetailCategory {
    Send,
    Receive,
    Generate,
    Immature,
    Orphan,
}

impl From<bitcoincore_rpc_async::json::GetTransactionResultDetailCategory> for DetailCategory {
    fn from(value: bitcoincore_rpc_async::json::GetTransactionResultDetailCategory) -> Self {
        match value {
            bitcoincore_rpc_async::json::GetTransactionResultDetailCategory::Send => {
                DetailCategory::Send
            }
            bitcoincore_rpc_async::json::GetTransactionResultDetailCategory::Receive => {
                DetailCategory::Receive
            }
            bitcoincore_rpc_async::json::GetTransactionResultDetailCategory::Generate => {
                DetailCategory::Generate
            }
            bitcoincore_rpc_async::json::GetTransactionResultDetailCategory::Immature => {
                DetailCategory::Immature
            }
            bitcoincore_rpc_async::json::GetTransactionResultDetailCategory::Orphan => {
                DetailCategory::Orphan
            }
        }
    }
}

#[derive(PartialEq, Eq, Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct SignedAmount(i64);

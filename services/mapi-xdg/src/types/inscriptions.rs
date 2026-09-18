use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ToSchema, Hash)]
pub struct Drc20TickerAndBalance {
    pub ticker: String,
    pub ticker_hex: String,
    pub balances: Drc20Balances,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ToSchema, Hash)]
pub struct Drc20Balances {
    pub total: String,
    pub available: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ToSchema, Hash)]
pub struct Drc20Holder {
    pub address: Option<String>,
    pub script_pubkey: String,
    pub balance: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ToSchema, Hash)]
pub struct Drc20Ticker(String);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ToSchema, Hash)]
pub struct Drc20Info {
    pub ticker: String,
    pub ticker_hex: String,
    pub deploy_inscription: String,
    pub holders: u64,
    pub minted_supply: String,
    pub terms: Drc20Terms,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ToSchema, Hash)]
pub struct Drc20Terms {
    pub max: String,
    pub limit: String,
    pub dec: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ToSchema, Hash)]
pub struct TransferInscriptionByAddress {
    // Ticker of the inscribed DRC20 token.
    pub ticker: String,
    /// String representation of an inscription ID, whose first coordinate is the reveal
    /// transaction hash, and the second coordinate is the index of the new inscription.
    pub inscription_id: String,
    /// Number of tokens locked in the UTxO.
    pub token_amount: String,
    /// Number of sats locked in the UTxO.
    pub sat_amount: u64,
    /// Tx ID of the UTxO containing the inscription.
    pub utxo_txid: String,
    /// Output index of the UTxO containing the inscription.
    pub utxo_vout: u32,
    /// Offset of inscribed sat in the UTxO containing it.
    pub utxo_sat_offset: u32,
    /// Block height of the UTxO containing the inscription.
    pub utxo_block_height: u64,
    /// Number of confirmations of the block where the UTxO containing the inscription was created.
    pub utxo_confirmations: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ToSchema, Hash)]
pub struct InscriptionByAddress {
    /// String representation of the inscription ID, whose first coordinate is the reveal
    /// transaction hash, and the second coordinate is the index of inscription in the reveal
    /// transaction.
    pub inscription_id: String,
    /// Total number of satoshis in the UTxO containing the inscription.
    pub satoshis: String,
    /// Inscribed sat offset in the UTxO containing it.
    pub utxo_sat_offset: u32,
    /// Tx ID of the UTxO containing the inscription.
    pub utxo_txid: String,
    /// Output index of the UTxO containing the inscription.
    pub utxo_vout: u32,
    /// Block height of the UTxO containing the inscription.
    pub utxo_block_height: u64,
    /// Number of confirmations of the block where the UTxO containing the inscription was created.
    pub utxo_confirmations: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ToSchema, Hash)]
pub struct InscriptionInfo {
    /// String representation of the inscription ID, whose first coordinate is the reveal
    /// transaction hash, and the second coordinate is the index of inscription in the reveal
    /// transaction.
    pub inscription_id: String,
    /// Block height of the reveal transaction.
    pub created_at: u64,
    /// Global inscription number.
    pub inscription_num: u64,
    /// Type of the content body.
    pub content_type: Option<String>,
    /// Preview of inscription content body raw data. Max: 100 bytes.
    /// Supported types: "text/plain", "text/plain;charset=utf-8", "application/json".
    pub content_body_preview: Option<String>,
    /// Length of entire inscription content body bytes array.
    pub content_length: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ToSchema, Hash)]
pub struct ContentBody {
    /// Base64-encoded representation of a slice of the inscription content body. All types supported.
    pub content_body_page: String,
    /// Number of bytes in entire inscription content body.
    pub total_length: u64,
    /// Number of bytes remaining in the inscription content body.
    pub remaining_bytes: u64,
}

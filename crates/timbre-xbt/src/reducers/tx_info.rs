use crate::{Decode, Encode, TimbreVec, VarUInt};

#[derive(Clone, Debug, Encode, Decode)]
/// max size 35
pub struct Key {
    /// Block height.
    /// Casting type: u64.
    pub height: VarUInt,

    /// Index of this transaction in the block.
    /// Casting type: usize.
    pub tx_index: VarUInt,
}

#[derive(Clone, Debug, Encode, Decode, PartialEq, Eq)]
/// max size: 32 + 1 + 17 + 1 + 17 + 1 + 17 + 1 + 17 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + (17 + (242 * number of inputs)) + 1 + (17 + (209 * number of outputs))
pub struct Value {
    /// Block hash.
    pub block_hash: Option<[u8; 32]>,

    /// The timestamp of the block, as claimed by the miner.
    /// Casting type: u32.
    pub timestamp: Option<VarUInt>,

    /// Total number of satoshis that went through this transaction, minus fees.
    /// Casting type: u64.
    pub volume: VarUInt,

    /// Fees paid to the miner.
    /// Casting type: u64.
    pub fees: VarUInt,

    /// Satoshis per vB of the transaction.
    /// Casting type: u64.
    pub sats_per_vb: VarUInt,

    /// Whether any of the inputs or outputs of the transaction contains inscriptions.
    pub involves_inscriptions: bool,

    /// Whether any of the inputs or outputs of the transaction contains runes.
    pub involves_runes: bool,

    /// Whether the transaction involves BRC-20.
    pub involves_brc20: bool,

    /// List of inputs, in the same order as the transaction.
    pub inputs: TimbreVec<TxIn>,

    /// List of outputs, in the same order as the transaction.
    pub outputs: TimbreVec<TxOut>,
}

/// max size: 242 = 32 + 1 + 17 + 1 + 20 + 1 + 17 + 1 + (17 + (17 + 32 + 17)) + 1 + (17 + (17 + 17 + 17))
#[derive(Clone, Debug, Encode, Decode, PartialEq, Eq)]
pub struct TxIn {
    /// Hash of the transaction that produced this UTxO.
    pub utxo_hash: [u8; 32],

    /// Output index in the transaction that produced this UTxO.0
    /// Casting type: u32.
    pub utxo_vout: VarUInt,

    /// Script hash controlling this UTxO.
    pub script_hash: [u8; 20],

    /// Satoshis in this UTxO.
    /// Casting type: u64.
    pub satoshis: VarUInt,

    /// Inscriptions in the input, each one represented as (offset, (reveal tx hash, inscription index in reveal tx)).
    /// Casting type: Vec<(u32, ([u8; 32], u32))>
    pub inscriptions: TimbreVec<(VarUInt, ([u8; 32], VarUInt))>,

    /// Runes in the input, each one represented as (rune ID, amount of runes in the input), where rune ID is (block of etching, tx of etching).
    /// Casting type: Vec<((u64, u32), u128)>.
    pub runes: TimbreVec<((VarUInt, VarUInt), VarUInt)>,
}

/// max size: 209 = 20 + 1 + 17 + 1 + (17 + (17 + 32 + 17)) + 1 + (17 + (17 + 17 + 17))
#[derive(Clone, Debug, Encode, Decode, PartialEq, Eq)]
pub struct TxOut {
    /// Script hash controlling this UTxO.
    pub script_hash: [u8; 20],

    /// Satoshis in this UTxO.
    /// Casting type: u64,
    pub satoshis: VarUInt,

    /// Inscriptions in the output, each one represented as (offset, (reveal tx hash, inscription index in reveal tx)).
    /// Casting type: Vec<(u32, ([u8; 32], u32))>
    pub inscriptions: TimbreVec<(VarUInt, ([u8; 32], VarUInt))>,

    /// Runes in the output, each one represented as (rune ID, amount of runes), where rune ID is (block of etching, tx of etching).
    /// Casting type: Vec<((u64, u32), u128)>.
    pub runes: TimbreVec<((VarUInt, VarUInt), VarUInt)>,
}

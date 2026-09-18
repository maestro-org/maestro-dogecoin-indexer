use crate::{Decode, Encode};

#[derive(Clone, Debug, Encode, Decode)]
/// max size 20 + 1 + 8 + 1 + 4 + 1 + 32
pub struct Key {
    // Script hash.
    pub script_hash: [u8; 20],

    // Block height.
    pub height: u64,

    // Index of tx in the block.
    pub activity_tx_index: u32,

    // Transaction hash.
    pub tx_hash: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
/// max size (4 + number of self-transfers * 56) + 1 + (4 + number of sent inscriptions * 77) + 1 + (4 + number of received inscriptions * 80)
pub struct Value {
    // List of self-transferred inscriptions.
    pub self_transfers: Vec<SelfTransferredInscription>,

    // List of sent inscriptions.
    pub sent: Vec<SentInscription>,

    // List of received inscriptions.
    pub received: Vec<ReceivedInscription>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
/// max size: (32 + 4) + 1 + 4 + 1 + 4 + 1 + 4 + 1 + 4
pub struct SelfTransferredInscription {
    pub inscription_id: ([u8; 32], u32),

    pub input_index: u32,

    pub input_sat_offset: u32,

    pub output_index: u32,

    pub output_sat_offset: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
/// max size: (32 + 4) + 1 + 8 + 1 + 4 + 1 + 8 + 1 + 4 + 1 + 20
pub struct SentInscription {
    pub inscription_id: ([u8; 32], u32),

    pub input_index: u32,

    pub input_sat_offset: u32,

    pub output_index: u32,

    pub output_sat_offset: u32,

    pub output_script_hash: [u8; 20],
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
/// max size: (32 + 4) + 1 + (1 + 8) + 1 + (1 + 4) + 1 + (1 + 20) + 1 + 8 + 1 + 4
pub struct ReceivedInscription {
    pub inscription_id: ([u8; 32], u32),

    pub input_index: Option<u32>,

    pub input_sat_offset: Option<u32>,

    pub input_script_hash: Option<[u8; 20]>,

    pub output_index: u32,

    pub output_sat_offset: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
pub struct Cursor {
    // Block height.
    pub height: u64,

    // Index of tx in the block.
    pub activity_tx_index: u32,

    // Transaction hash.
    pub tx_hash: [u8; 32],
}

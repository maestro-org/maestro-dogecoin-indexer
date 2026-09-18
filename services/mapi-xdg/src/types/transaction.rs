use bitcoincore_rpc_async::bitcoin::hashes::hex::ToHex;
use bitcoincore_rpc_async::bitcoin::Txid;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(PartialEq, Eq, Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct TransactionId(String);

impl From<Txid> for TransactionId {
    fn from(value: Txid) -> Self {
        Self(value.to_hex())
    }
}

#[derive(PartialEq, Eq, Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct Transaction {
    /// The protocol version, is currently expected to be 1 or 2 (BIP 68).
    pub version: i32,
    /// Block number before which this transaction is valid, or 0 for
    /// valid immediately.
    pub lock_time: u32,
    /// List of inputs
    pub input: Vec<TxIn>,
    /// List of outputs
    pub output: Vec<TxOut>,
}

impl From<bitcoincore_rpc_async::bitcoin::Transaction> for Transaction {
    fn from(value: bitcoincore_rpc_async::bitcoin::Transaction) -> Self {
        Self {
            version: value.version,
            lock_time: value.lock_time,
            input: value.input.into_iter().map(Into::into).collect(),
            output: value.output.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(PartialEq, Eq, Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct TxIn {
    /// The reference to the previous output that is being used an an input
    pub previous_output: OutPoint,
    /// The script which pushes values on the stack which will cause
    /// the referenced output's script to accept
    pub script_sig: Script,
    /// The sequence number, which suggests to miners which of two
    /// conflicting transactions should be preferred, or 0xFFFFFFFF
    /// to ignore this feature. This is generally never used since
    /// the miner behaviour cannot be enforced.
    pub sequence: u32,
    /// Witness data: an array of byte-arrays.
    /// Note that this field is *not* (de)serialized with the rest of the TxIn in
    /// Encodable/Decodable, as it is (de)serialized at the end of the full
    /// Transaction. It *is* (de)serialized with the rest of the TxIn in other
    /// (de)serialization routines.
    pub witness: Vec<Vec<u8>>,
}

impl From<bitcoincore_rpc_async::bitcoin::TxIn> for TxIn {
    fn from(value: bitcoincore_rpc_async::bitcoin::TxIn) -> Self {
        Self {
            previous_output: value.previous_output.into(),
            script_sig: value.script_sig.into(),
            sequence: value.sequence,
            witness: value.witness,
        }
    }
}

#[derive(PartialEq, Eq, Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct TxOut {
    /// The value of the output, in satoshis
    pub value: u64,
    /// The script which must satisfy for the output to be spent
    pub script_pubkey: Script,
}

impl From<bitcoincore_rpc_async::bitcoin::TxOut> for TxOut {
    fn from(value: bitcoincore_rpc_async::bitcoin::TxOut) -> Self {
        Self {
            value: value.value,
            script_pubkey: value.script_pubkey.into(),
        }
    }
}

#[derive(PartialEq, Eq, Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct OutPoint {
    /// The referenced transaction's txid
    pub txid: TransactionId,
    /// The index of the referenced output in its transaction's vout
    pub vout: u32,
}

impl From<bitcoincore_rpc_async::bitcoin::OutPoint> for OutPoint {
    fn from(value: bitcoincore_rpc_async::bitcoin::OutPoint) -> Self {
        Self {
            txid: value.txid.into(),
            vout: value.vout,
        }
    }
}

#[derive(PartialEq, Eq, Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct Script(Box<[u8]>);

impl From<bitcoincore_rpc_async::bitcoin::Script> for Script {
    fn from(value: bitcoincore_rpc_async::bitcoin::Script) -> Self {
        Self(value.to_bytes().into())
    }
}

#[derive(PartialEq, Eq, Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct InvolvedTransaction {
    /// The transaction's txid
    pub tx_hash: String,
    /// Height of the block which included the transaction
    pub height: u64,
    /// Address/pubkey controlled an input UTxO
    pub input: bool,
    /// Address/pubkey controlled an output UTxO
    pub output: bool,
}

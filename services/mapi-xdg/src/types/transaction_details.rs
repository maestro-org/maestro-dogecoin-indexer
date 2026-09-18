use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr, PickFirst};
use utoipa::ToSchema;

#[serde_as]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
/// Represents the details of a Dogecoin transaction.
pub struct TransactionDetails {
    /// The hash of the block that contains this transaction.
    #[serde(alias = "blockhash")]
    #[schema(example = "1c3e7cd9e46bd6d0adb1ae0b52ec1a1ddfaa0cb61a41a1b1c26f0d1b0f42a06c")]
    pub block_hash: String,

    /// The time the block containing this transaction was mined (in UNIX timestamp).
    #[serde(alias = "blocktime")]
    #[schema(example = 1726478580)]
    pub block_time: u64,

    /// The number of confirmations for this transaction.
    #[schema(example = 15)]
    pub confirmations: u64,

    /// The transaction hash (TXID).
    #[schema(example = "faf89773336656903f55450b478917b004cf506a200349e808adbbe4f9cda96b")]
    pub hash: String,

    /// The raw transaction in hexadecimal form.
    #[schema(
        example = "010000000001011c3e7cd9e46bd6d0adb1ae0b52ec1a1ddfaa0cb61a41a1b1c26f0d1b0f42a06c..."
    )]
    pub hex: String,

    /// The locktime of the transaction, which specifies the earliest time or block when the transaction can be included in a block.
    #[serde(alias = "locktime")]
    #[schema(example = 0)]
    pub lock_time: u64,

    /// The size of the transaction in bytes.
    #[schema(example = 394)]
    pub size: u64,

    /// The timestamp when the transaction was included in the block (in UNIX timestamp).
    #[schema(example = 1726478580)]
    pub time: u64,

    /// The transaction ID (TXID) of this transaction.
    #[serde(alias = "txid")]
    #[schema(example = "74788e29e7bbf22f08d2a9eac1b63e31d5baca455586227734d675b80c9af36e")]
    pub tx_id: String,

    /// The version number of the transaction, indicating the format or type of the transaction.
    #[schema(example = 1)]
    pub version: u32,

    /// The transaction inputs, which refer to the previous transaction outputs that are being spent.
    pub vin: Vec<Vin>,

    /// The transaction outputs, which specify how much Dogecoin is being sent to which addresses.
    pub vout: Vec<Vout>,

    /// The virtual size of the transaction.
    #[serde(alias = "vsize")]
    #[schema(example = 367)]
    pub v_size: u64,

    /// The weight of the transaction (v_size * 4).
    #[schema(example = 1468)]
    pub weight: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
/// Represents the input of a Dogecoin transaction.
pub struct Vin {
    /// The coinbase transaction data (for block rewards) if this is a coinbase transaction.
    #[schema(
        example = "0364250d1b4d696e656420627920416e74506f6f6c39373048000900b63fae52fabe6d6d6987414626bf1e7395227273838943f7e2212274359ab5c925dede7bc5f648ff10000000000000000000b405ecec000000000000"
    )]
    pub coinbase: Option<String>,

    /// The sequence number for the input, which is sequence number.
    #[schema(example = 4294967)]
    pub sequence: u64,

    /// The witness data for the transaction input (unused on Dogecoin).
    #[serde(alias = "txinwitness")]
    #[schema(example = json!(["1c3e7cd9e46bd6d0adb1ae0b52ec1a1ddfaa0cb61a41a1b1c26f0d1b0f42a06c"]))]
    pub txin_witness: Option<Vec<String>>,
}

#[serde_as]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
/// Represents the output of a Dogecoin transaction.
pub struct Vout {
    /// The index of this output in the transaction (0-based).
    #[schema(example = 0)]
    pub n: u64,

    /// The amount of Dogecoin (in DOGE) sent to this output.
    #[schema(example = 0.00000546)]
    #[serde_as(as = "PickFirst<(DisplayFromStr, _)>")]
    pub value: f64,

    /// The public key script (scriptPubKey) specifying the conditions to spend the output.
    #[serde(alias = "scriptPubKey")]
    pub script_pub_key: ScriptPubKey,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
/// Represents the script used to lock/unlock Dogecoin in an output.
pub struct ScriptPubKey {
    /// The Dogecoin address to which this output belongs.
    #[schema(example = "D8UC42uehZdBYAfMBJumQKuBNs9vvarxtt")]
    pub address: Option<String>,

    /// The assembly (ASM) representation of the script, showing the operation codes and operands.
    #[schema(example = "OP_HASH160 42402a28dd61f2718a4b27ae72a4791d5bbdade7 OP_EQUAL")]
    pub asm: String,

    /// The detailed descriptor of the scriptPubKey, providing more information about the address and script.
    #[schema(example = "addr(D8UC42uehZdBYAfMBJumQKuBNs9vvarxtt)#avhxp88d")]
    pub desc: String,

    /// The raw hexadecimal representation of the script.
    #[schema(example = "a91442402a28dd61f2718a4b27ae72a4791d5bbdade787")]
    pub hex: String,

    /// The type of the script (e.g., "scripthash", "pubkeyhash").
    #[schema(example = "scripthash")]
    pub r#type: String,
}

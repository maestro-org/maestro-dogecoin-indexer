use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Block {
    pub auxpow: AuxPow,
    pub bits: String,
    pub chainwork: String,
    pub confirmations: i64,
    pub difficulty: f64,
    pub hash: String,
    pub height: i64,
    pub mediantime: i64,
    pub merkleroot: String,
    pub nonce: i64,
    pub previousblockhash: String,
    pub size: i64,
    pub strippedsize: i64,
    pub time: i64,
    pub tx: Vec<String>,
    pub version: i64,
    #[serde(rename = "versionHex")]
    pub version_hex: String,
    pub weight: i64,
}

#[derive(Serialize, Deserialize)]
pub struct AuxPow {
    pub chainindex: i64,
    pub chainmerklebranch: Vec<String>,
    pub index: i64,
    pub merklebranch: Vec<String>,
    pub parentblock: String,
    pub tx: Tx,
}

#[derive(Serialize, Deserialize)]
pub struct Tx {
    pub blockhash: String,
    pub hash: String,
    pub hex: String,
    pub locktime: i64,
    pub size: i64,
    pub txid: String,
    pub version: i64,
    pub vin: Vec<Vin>,
    pub vout: Vec<Vout>,
    pub vsize: i64,
}

#[derive(Serialize, Deserialize)]
pub struct Vin {
    pub coinbase: String,
    pub sequence: i64,
}

#[derive(Serialize, Deserialize)]
pub struct Vout {
    pub n: i64,
    #[serde(rename = "scriptPubKey")]
    pub script_pubkey: Option<ScriptPubKey>,
    pub value: f64,
}

#[derive(Serialize, Deserialize)]
pub struct ScriptPubKey {
    pub addresses: Vec<String>,
    pub asm: String,
    pub hex: String,
    #[serde(rename = "reqSigs")]
    pub req_sigs: i64,
    pub r#type: String,
}

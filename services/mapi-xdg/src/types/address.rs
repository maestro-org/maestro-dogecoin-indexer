use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(PartialEq, Eq, Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct Address {
    /// The type of the address
    pub payload: Payload,
    /// The network on which this address is usable
    pub network: Network,
}

impl From<bitcoincore_rpc_async::bitcoin::Address> for Address {
    fn from(value: bitcoincore_rpc_async::bitcoin::Address) -> Self {
        Self {
            payload: value.payload.into(),
            network: value.network.into(),
        }
    }
}

#[derive(PartialEq, Eq, Clone, Debug, Serialize, Deserialize, ToSchema)]
pub enum Network {
    Mainnet,
    Testnet,
    Signet,
    Regtest,
}

impl From<bitcoincore_rpc_async::bitcoin::Network> for Network {
    fn from(value: bitcoincore_rpc_async::bitcoin::Network) -> Self {
        match value {
            bitcoincore_rpc_async::bitcoin::Network::Bitcoin => Self::Mainnet,
            bitcoincore_rpc_async::bitcoin::Network::Testnet => Self::Testnet,
            bitcoincore_rpc_async::bitcoin::Network::Signet => Self::Signet,
            bitcoincore_rpc_async::bitcoin::Network::Regtest => Self::Regtest,
        }
    }
}

#[derive(PartialEq, Eq, Clone, Debug, Serialize, Deserialize, ToSchema)]
pub enum Payload {
    /// P2PKH address
    PubkeyHash(PubkeyHash),
    /// P2SH address
    ScriptHash(ScriptHash),
    /// Segwit addresses
    WitnessProgram {
        /// The witness program version
        version: u8,
        /// The witness program
        program: Vec<u8>,
    },
}
impl From<bitcoincore_rpc_async::bitcoin::util::address::Payload> for Payload {
    fn from(value: bitcoincore_rpc_async::bitcoin::util::address::Payload) -> Self {
        match value {
            bitcoincore_rpc_async::bitcoin::util::address::Payload::PubkeyHash(s) => {
                Payload::PubkeyHash(s.into())
            }
            bitcoincore_rpc_async::bitcoin::util::address::Payload::ScriptHash(s) => {
                Payload::ScriptHash(s.into())
            }
            bitcoincore_rpc_async::bitcoin::util::address::Payload::WitnessProgram {
                version,
                program,
            } => Payload::WitnessProgram {
                version: version.into(),
                program,
            },
        }
    }
}

#[derive(PartialEq, Eq, Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct PubkeyHash([u8; 20]);

impl From<bitcoincore_rpc_async::bitcoin::PubkeyHash> for PubkeyHash {
    fn from(value: bitcoincore_rpc_async::bitcoin::PubkeyHash) -> Self {
        Self(value.to_vec().try_into().unwrap())
    }
}

#[derive(PartialEq, Eq, Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct ScriptHash([u8; 20]);

impl From<bitcoincore_rpc_async::bitcoin::ScriptHash> for ScriptHash {
    fn from(value: bitcoincore_rpc_async::bitcoin::ScriptHash) -> Self {
        Self(value.to_vec().try_into().unwrap())
    }
}

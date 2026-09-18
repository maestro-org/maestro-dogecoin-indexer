use std::time::Duration;

use bitcoin::block::{Header, Version};
use bitcoin::consensus::Decodable;
use bitcoin::hash_types::TxMerkleNode;
use bitcoin::hashes::Hash;
use bitcoin::network::Magic;
use bitcoin::{Block, BlockHash, CompactTarget, Network, Transaction};
use bitcoincore_rpc::Auth;
use gasket::messaging::{RecvPort, SendPort};
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::prelude::Error;
use crate::storage::ChainDB;

mod health;
mod model;
mod pull;
pub mod roll;

#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum BitcoinCompatibleNetwork {
    Bitcoin,
    BitcoinTestnet,
    Dogecoin,
    DogecoinTestnet,
}

impl BitcoinCompatibleNetwork {
    pub fn genesis_block(&self) -> Block {
        match self {
            Self::Bitcoin => bitcoin::constants::genesis_block(Network::Bitcoin),
            Self::BitcoinTestnet => bitcoin::constants::genesis_block(Network::Testnet),
            // https://github.com/sandshrewmetaprotocols/metashrew/blob/master/src/chain.rs
            Self::Dogecoin => {
                Block {
                    header: Header {
                        version: Version::ONE,
                        prev_blockhash: Hash::all_zeros(),
                        merkle_root: TxMerkleNode::from_slice(&hex::decode("696ad20e2dd4365c7459b4a4a5af743d5e92c6da3229e6532cd605f6533f2a5b").unwrap()).unwrap(),
                        time: 1386325540,
                        bits: CompactTarget::from_consensus(0x1e0ffff0),
                        nonce: 99943,
                    },
                    txdata: vec![<Transaction as Decodable>::consensus_decode(&mut hex::decode("01000000010000000000000000000000000000000000000000000000000000000000000000ffffffff1004ffff001d0104084e696e746f6e646fffffffff010058850c020000004341040184710fa689ad5023690c80f3a49c8f13f8d45b8c857fbcbc8bc4a8e4d3eb4b10f4d4604fa08dce601aaf0f470216fe1b51850b4acf21b179c45070ac7b03a9ac00000000").unwrap().as_slice()).unwrap()]
                }
            },
            // Same genesis coinbase as mainnet; only time and nonce differ.
            Self::DogecoinTestnet => {
                Block {
                    header: Header {
                        version: Version::ONE,
                        prev_blockhash: Hash::all_zeros(),
                        merkle_root: TxMerkleNode::from_slice(&hex::decode("696ad20e2dd4365c7459b4a4a5af743d5e92c6da3229e6532cd605f6533f2a5b").unwrap()).unwrap(),
                        time: 1391503289,
                        bits: CompactTarget::from_consensus(0x1e0ffff0),
                        nonce: 997879,
                    },
                    txdata: vec![<Transaction as Decodable>::consensus_decode(&mut hex::decode("01000000010000000000000000000000000000000000000000000000000000000000000000ffffffff1004ffff001d0104084e696e746f6e646fffffffff010058850c020000004341040184710fa689ad5023690c80f3a49c8f13f8d45b8c857fbcbc8bc4a8e4d3eb4b10f4d4604fa08dce601aaf0f470216fe1b51850b4acf21b179c45070ac7b03a9ac00000000").unwrap().as_slice()).unwrap()]
                }
            },
        }
    }

    pub fn genesis_block_hash(&self) -> BlockHash {
        match self {
            Self::Bitcoin => bitcoin::constants::genesis_block(Network::Bitcoin).block_hash(),
            Self::BitcoinTestnet => bitcoin::constants::genesis_block(Network::Testnet).block_hash(),
            // https://github.com/sandshrewmetaprotocols/metashrew/blob/master/src/chain.rs
            Self::Dogecoin => {
                Block {
                    header: Header {
                        version: Version::ONE,
                        prev_blockhash: Hash::all_zeros(),
                        merkle_root: TxMerkleNode::from_slice(&hex::decode("696ad20e2dd4365c7459b4a4a5af743d5e92c6da3229e6532cd605f6533f2a5b").unwrap()).unwrap(),
                        time: 1386325540,
                        bits: CompactTarget::from_consensus(0x1e0ffff0),
                        nonce: 99943,
                    },
                    txdata: vec![<Transaction as Decodable>::consensus_decode(&mut hex::decode("01000000010000000000000000000000000000000000000000000000000000000000000000ffffffff1004ffff001d0104084e696e746f6e646fffffffff010058850c020000004341040184710fa689ad5023690c80f3a49c8f13f8d45b8c857fbcbc8bc4a8e4d3eb4b10f4d4604fa08dce601aaf0f470216fe1b51850b4acf21b179c45070ac7b03a9ac00000000").unwrap().as_slice()).unwrap()]
                }.block_hash()
            },
            Self::DogecoinTestnet => {
                BlockHash::from_slice(
                    hex::decode(
                        "9e555073d0c4f36456db8951f449704d544d2826d9aa60636b40374626780abb",
                    )
                    .unwrap()
                    .as_slice(),
                ).unwrap()
            },
        }
    }

    pub fn magic(&self) -> Magic {
        match self {
            Self::Bitcoin => Network::Bitcoin.magic(),
            Self::BitcoinTestnet => Network::Testnet.magic(),
            Self::Dogecoin => Magic::from_bytes([0xc0, 0xc0, 0xc0, 0xc0]),
            Self::DogecoinTestnet => Magic::from_bytes([0xfc, 0xc1, 0xb7, 0xdc]),
        }
    }
}

impl Into<bitcoin::Network> for BitcoinCompatibleNetwork {
    fn into(self) -> bitcoin::Network {
        match self {
            Self::Bitcoin => bitcoin::Network::Bitcoin,
            Self::BitcoinTestnet => bitcoin::Network::Testnet,
            Self::Dogecoin => bitcoin::Network::Regtest, // regtest has start height 0 for runes indexing
            Self::DogecoinTestnet => bitcoin::Network::Regtest, // regtest has start height 0 for runes indexing
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct Config {
    pub node_address: String,
    pub node_rpc: String,
    pub node_rpc_user: String,
    pub node_rpc_pass: String,
    pub network: BitcoinCompatibleNetwork,
    pub health_endpoint: String,
    pub intersect: Option<(u64, String)>,
    pub first_rune_height: Option<u64>,
    pub first_inscription_height: Option<u64>,
    /// Unused on Dogecoin (Bitcoin ordinals concept); defaults to 0
    #[serde(default)]
    pub jubilee_height: u64,
    // amount of blocks to fetch from node per request
    pub block_page_size: Option<usize>,
    // max messages to queue between sync stages
    pub sync_channel_queue: Option<usize>,
    // store all UTxOs in memory as well as database
    pub utxos_in_memory: Option<bool>,
}

fn define_gasket_policy(config: &Option<gasket::retries::Policy>) -> gasket::runtime::Policy {
    let default_retries = gasket::retries::Policy {
        max_retries: 20,
        backoff_unit: Duration::from_secs(1),
        backoff_factor: 2,
        max_backoff: Duration::from_secs(60),
        dismissible: false,
    };

    let retries = config.clone().unwrap_or(default_retries);

    gasket::runtime::Policy {
        tick_timeout: std::time::Duration::from_secs(600).into(),
        bootstrap_retry: retries.clone(),
        work_retry: retries.clone(),
        teardown_retry: retries.clone(),
    }
}

pub fn pipeline(
    config: &Config,
    chain_db: ChainDB,
    retries: &Option<gasket::retries::Policy>,
) -> Result<gasket::daemon::Daemon, Error> {
    let rpc_auth = Auth::UserPass(config.node_rpc_user.clone(), config.node_rpc_pass.clone());

    let mut pull = pull::Stage::new(
        config.node_address.clone(),
        config.node_rpc.clone(),
        rpc_auth.clone(),
        config.network,
        chain_db.clone(),
        config.block_page_size.unwrap_or(50),
        config.intersect.clone(),
    );

    let chain_cursor = chain_db.cursor().map_err(Error::storage)?;
    info!(?chain_cursor, "chain cursor");

    let mut health = health::Stage::new(
        config.health_endpoint.clone(),
        config.node_rpc.clone(),
        rpc_auth.clone(),
    );

    let mut roll = roll::Stage::new(chain_db, config.utxos_in_memory);

    let queue_size = config.sync_channel_queue.unwrap_or(250);

    let (to_roll, from_pull) = gasket::messaging::tokio::mpsc_channel(queue_size);
    pull.downstream.connect(to_roll);
    roll.upstream.connect(from_pull);

    let (roll_to_health, roll_from_health) = gasket::messaging::tokio::mpsc_channel(queue_size);

    health.roll_upstream.connect(roll_from_health);
    roll.health_downstream.connect(roll_to_health);

    let policy = define_gasket_policy(retries);

    let pull = gasket::runtime::spawn_stage(pull, policy.clone());
    let roll = gasket::runtime::spawn_stage(roll, policy.clone());
    let health = gasket::runtime::spawn_stage(health, policy);

    Ok(gasket::daemon::Daemon(vec![pull, roll, health]))
}

#[cfg(test)]
mod genesis_tests {
    use super::*;

    #[test]
    fn dogecoin_genesis_blocks_hash_correctly() {
        for network in [
            BitcoinCompatibleNetwork::Dogecoin,
            BitcoinCompatibleNetwork::DogecoinTestnet,
        ] {
            let block = network.genesis_block();
            assert_eq!(block.block_hash(), network.genesis_block_hash());
            assert_eq!(
                block.header.merkle_root,
                block.compute_merkle_root().unwrap()
            );
        }
    }
}

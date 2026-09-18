use std::sync::Arc;

use async_trait::async_trait;

pub mod dogecoin;

use bitcoincore_rpc_async::bitcoin::{BlockHash, Txid};
pub use dogecoin::*;
use serde_json::Value;

use crate::{
    error::Error,
    options::{Mode, Options},
    types::BlockchainInfo,
};

pub type ChainClient = Arc<dyn Client + Send + Sync + 'static>;

#[async_trait]
pub trait Client {
    async fn get_chain_tip(&self) -> Result<(u64, BlockHash), Error>;
    async fn get_blockchain_info(&self) -> Result<BlockchainInfo, Error>;
    async fn get_latest_block(&self) -> Result<Value, Error>;
    async fn get_block(&self, block_hash: BlockHash) -> Result<Value, Error>;
    async fn get_transaction(&self, tx_hash: Txid) -> Result<Value, Error>;
    async fn decode_psbt(&self, psbt: &str) -> Result<Value, Error>;
    async fn raw_rpc_call(&self, method: &str, params: &Value) -> Result<Value, Error>;
    // Mempool RPC:
    async fn get_mempool_info(&self) -> Result<Value, Error>;
    async fn get_mempool_transactions(&self) -> Result<Value, Error>;
    async fn get_mempool_transaction_details(&self, tx_hash: Txid) -> Result<Value, Error>;
    async fn get_mempool_transaction_ancestors(&self, tx_hash: Txid) -> Result<Value, Error>;
    async fn get_mempool_transaction_descendants(&self, tx_hash: Txid) -> Result<Value, Error>;
    // Transaction RPC:
    async fn get_transaction_details(&self, tx_hash: Txid) -> Result<Value, Error>;
}

pub async fn client(options: &Options) -> Result<Arc<dyn Client + Send + Sync + 'static>, Error> {
    match options.mode {
        Mode::Dogecoin | Mode::DogecoinTestnet => {
            dogecoin(options).await.map(|client| Arc::new(client) as _)
        }
        Mode::GenerateOpenApi => {
            return Err(Error::GenerateOpenApiIsUnexpected());
        }
    }
}

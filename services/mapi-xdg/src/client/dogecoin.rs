use std::str::FromStr;

use async_trait::async_trait;

use bitcoincore_rpc_async::bitcoin::hashes::hex::ToHex;
use bitcoincore_rpc_async::bitcoin::{BlockHash, Txid};

use serde_json::{json, Value};

use super::Client;
use crate::{
    error::Error,
    options::Options,
    types::{
        mempool_info::MempoolInfo, mempool_transaction_details::MempoolTransactionDetails,
        mempool_transactions::MempoolTransactions, rpc::RpcCall, BlockchainInfo,
    },
};

#[derive(Debug, Clone)]
pub struct Dogecoin {
    client: reqwest::Client,
    rpc_url: String,
    rpc_username: String,
    rpc_password: String,
}

#[async_trait]
impl Client for Dogecoin {
    async fn get_chain_tip(&self) -> Result<(u64, BlockHash), Error> {
        let height = self.raw_rpc_call("getblockcount", &json!([])).await?;

        let block = self.raw_rpc_call("getbestblockhash", &json!([])).await?;

        let height = height
            .as_u64()
            .ok_or(Error::Internal("Height is not a u64".to_string()))?;

        let block = BlockHash::from_str(
            block
                .as_str()
                .ok_or(Error::Internal("Block hash is not a string".to_string()))?,
        )
        .map_err(|e| Error::InvalidHex(e.to_string()))?;

        Ok((height, block))
    }

    async fn get_blockchain_info(&self) -> Result<BlockchainInfo, Error> {
        Ok(serde_json::from_value(
            self.raw_rpc_call("getblockchaininfo", &json!([])).await?,
        )?)
    }

    async fn get_latest_block(&self) -> Result<Value, Error> {
        let blockhash: String =
            serde_json::from_value(self.raw_rpc_call("getbestblockhash", &json!([])).await?)?;
        let response = self.raw_rpc_call("getblock", &json!([blockhash])).await?;

        Ok(serde_json::to_value(response)?)
    }

    async fn get_block(&self, block_hash: BlockHash) -> Result<Value, Error> {
        Ok(serde_json::from_value(
            self.raw_rpc_call("getblock", &json!([block_hash.to_hex(), true]))
                .await?,
        )?)
    }

    async fn get_transaction(&self, tx_hash: Txid) -> Result<Value, Error> {
        Ok(serde_json::from_value(
            self.raw_rpc_call(
                "getrawtransaction",
                &json!([
                    tx_hash.to_hex().to_string(),
                    1 // 1 for Verbose Output
                ]),
            )
            .await?,
        )?)
    }

    async fn get_mempool_info(&self) -> Result<Value, Error> {
        let raw_json = self.raw_rpc_call("getmempoolinfo", &json!([])).await?;
        let mempool_info: MempoolInfo = serde_json::from_value(raw_json)?;
        let serialized_json = serde_json::to_value(mempool_info)?;
        Ok(serialized_json)
    }

    async fn get_mempool_transactions(&self) -> Result<Value, Error> {
        let raw_json = self
            .raw_rpc_call(
                "getrawmempool",
                &json!([
                    false, // -> verbose
                    true,  // -> mempool_sequence
                ]),
            )
            .await?;
        let mempool_transactions: MempoolTransactions = serde_json::from_value(raw_json)?;
        let serialized_json: Value = serde_json::to_value(mempool_transactions)?;
        Ok(serialized_json)
    }

    async fn get_mempool_transaction_details(&self, tx_hash: Txid) -> Result<Value, Error> {
        let raw_json = self
            .raw_rpc_call(
                "getmempoolentry",
                &json!([
                    tx_hash.to_string(), // -> txid: The transaction id (must be in mempool)
                ]),
            )
            .await?;
        let mempool_transactions: MempoolTransactionDetails = serde_json::from_value(raw_json)?;
        let serialized_json: Value = serde_json::to_value(mempool_transactions)?;
        Ok(serialized_json)
    }

    async fn get_mempool_transaction_descendants(&self, tx_hash: Txid) -> Result<Value, Error> {
        Ok(serde_json::from_value(
            self.raw_rpc_call(
                "getmempooldescendants",
                &json!([
                    tx_hash.to_string(), // -> txid: The transaction id (must be in mempool)
                    false,               // -> verbose
                ]),
            )
            .await?,
        )?)
    }

    async fn get_mempool_transaction_ancestors(&self, tx_hash: Txid) -> Result<Value, Error> {
        Ok(serde_json::from_value(
            self.raw_rpc_call(
                "getmempoolancestors",
                &json!([
                    tx_hash.to_string(), // -> txid: The transaction id (must be in mempool)
                    false,               // -> verbose
                ]),
            )
            .await?,
        )?)
    }

    async fn get_transaction_details(&self, tx_hash: Txid) -> Result<Value, Error> {
        Ok(serde_json::from_value(
            self.raw_rpc_call(
                "getrawtransaction",
                &json!([
                    tx_hash.to_string(),
                    1 // -> Verbose Output
                ]),
            )
            .await?,
        )?)
    }

    async fn decode_psbt(&self, psbt: &str) -> Result<Value, Error> {
        Ok(serde_json::from_value(
            self.raw_rpc_call("decodepsbt", &json!([psbt])).await?,
        )?)
    }

    async fn raw_rpc_call(&self, method: &str, params: &Value) -> Result<Value, Error> {
        let request_body = RpcCall::new(method, params);

        let response = self
            .client
            .post(&self.rpc_url)
            .json(&request_body)
            .basic_auth(self.rpc_username.clone(), Some(self.rpc_password.clone()))
            .send()
            .await?;

        let response_body: Value = response.json().await?;

        if let Some(error) = response_body.get("error").filter(|e| !e.is_null()) {
            return Err(Error::Rpc(error.to_string()));
        }

        match response_body.get("result") {
            Some(result) => Ok(result.clone()),
            None => Err(Error::Internal("No result in response".to_string())),
        }
    }
}

pub async fn dogecoin(options: &Options) -> Result<Dogecoin, Error> {
    let rpc_url = if options.node_address.starts_with("http") {
        options.node_address.clone()
    } else {
        format!("http://{}", options.node_address)
    };

    Ok(Dogecoin {
        client: reqwest::Client::new(),
        rpc_url,
        rpc_username: options.node_user.clone(),
        rpc_password: options.node_password.clone(),
    })
}

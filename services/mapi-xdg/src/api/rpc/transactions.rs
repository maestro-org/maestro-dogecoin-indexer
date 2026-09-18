use crate::{
    client::ChainClient,
    error::Error,
    types::{LastUpdated, TimestampedResponse},
};
use axum::{
    extract::Path, http::StatusCode, response::IntoResponse, routing::get, Extension, Json, Router,
};
use bitcoincore_rpc_async::bitcoin::{
    hashes::hex::{FromHex, ToHex},
    Txid,
};
use serde::Deserialize;

/**
 *  Router for /rpc/transactions/... endpoints
 */
pub fn router() -> Router {
    Router::new().route("/:tx_hash", get(transaction_details))
}

#[derive(Deserialize)]
pub struct GetTransactionsParams {
    pub tx_hash: String,
}

#[derive(Deserialize)]
pub struct GetTransactionsOutputParams {
    pub tx_hash: String,
    pub tx_output_index: u64,
}

/// Transaction Details
pub async fn transaction_details(
    Path(params): Path<GetTransactionsParams>,
    client: Extension<ChainClient>,
) -> Result<impl IntoResponse, Error> {
    let tx = Txid::from_hex(&params.tx_hash).map_err(|_| Error::InvalidHex(params.tx_hash))?;
    let (x, y) = client.get_chain_tip().await?;

    let out = TimestampedResponse {
        data: client.get_transaction_details(tx).await?,
        last_updated: LastUpdated {
            block_height: x,
            block_hash: y.to_hex(),
        },
    };

    Ok((StatusCode::OK, Json(out)))
}

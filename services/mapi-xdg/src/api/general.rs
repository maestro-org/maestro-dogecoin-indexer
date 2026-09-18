use axum::{
    http::StatusCode, response::IntoResponse, routing::get as _get, Extension, Json, Router,
};
use bitcoincore_rpc_async::bitcoin::hashes::hex::ToHex;

use crate::{
    client::ChainClient,
    error::Error,
    types::{LastUpdated, TimestampedResponse},
};

pub fn router() -> Router {
    Router::new().route("/info", _get(chain_info))
}

/// Chain Info
///
/// Information about the state of the blockchain.
pub async fn chain_info(client: Extension<ChainClient>) -> Result<impl IntoResponse, Error> {
    let (x, y) = client.get_chain_tip().await?;

    let out = TimestampedResponse {
        data: client.get_blockchain_info().await?,
        last_updated: LastUpdated {
            block_height: x,
            block_hash: y.to_hex(),
        },
    };

    Ok((StatusCode::OK, Json(out)))
}

use crate::{
    client::ChainClient,
    error::Error,
    types::{LastUpdated, TimestampedResponse},
};
use axum::{
    extract::Path, http::StatusCode, response::IntoResponse, routing::get as _get, Extension, Json,
    Router,
};
use bitcoincore_rpc_async::bitcoin::{hashes::hex::FromHex, BlockHash as BH};
use dogecoin::hashes::hex::ToHex;

pub fn router() -> Router {
    Router::new()
        .route("/latest", _get(latest_block))
        .route("/:block_hash", _get(block_info))
}

/// Latest Block Info
///
/// Information about the latest block on the chain.
async fn latest_block(client: Extension<ChainClient>) -> Result<impl IntoResponse, Error> {
    let (x, y) = client.get_chain_tip().await?;

    let out = TimestampedResponse {
        data: client.get_latest_block().await?,
        last_updated: LastUpdated {
            block_height: x,
            block_hash: y.to_hex(),
        },
    };

    Ok((StatusCode::OK, Json(out)))
}

/// Block Info
///
/// Provides detailed information about a specific block by hash.
async fn block_info(
    Path(block_hash): Path<String>,
    client: Extension<ChainClient>,
) -> Result<impl IntoResponse, Error> {
    let block = BH::from_hex(&block_hash).map_err(|_| Error::InvalidHex(block_hash))?;

    let (x, y) = client.get_chain_tip().await?;

    let out = TimestampedResponse {
        data: client.get_block(block).await?,
        last_updated: LastUpdated {
            block_height: x,
            block_hash: y.to_hex(),
        },
    };

    Ok((StatusCode::OK, Json(out)))
}

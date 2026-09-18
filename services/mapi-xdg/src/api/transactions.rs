use crate::{
    client::ChainClient,
    error::Error,
    types::{LastUpdated, TimestampedResponse},
};
use axum::{
    extract::Path,
    http::StatusCode,
    response::IntoResponse,
    routing::{get as _get, post},
    Extension, Json, Router,
};
use bitcoincore_rpc_async::bitcoin::{
    hashes::hex::{FromHex, ToHex},
    Txid,
};
use serde::Deserialize;

pub fn router() -> Router {
    Router::new()
        .route("/:tx_hash", _get(transaction_info))
        .route("/psbt/decode", post(decode_psbt))
}

#[derive(Deserialize)]
pub struct GetTransactionsParams {
    pub tx_hash: String,
}

/// Transaction Info
///
/// Information about the specified transaction.
pub async fn transaction_info(
    Path(params): Path<GetTransactionsParams>,
    client: Extension<ChainClient>,
) -> Result<impl IntoResponse, Error> {
    let transaction =
        Txid::from_hex(&params.tx_hash).map_err(|_| Error::InvalidHex(params.tx_hash))?;

    let (x, y) = client.get_chain_tip().await?;

    let out = TimestampedResponse {
        data: client.get_transaction(transaction).await?,
        last_updated: LastUpdated {
            block_height: x,
            block_hash: y.to_hex(),
        },
    };

    Ok((StatusCode::OK, Json(out)))
}

#[derive(Deserialize)]
pub struct DecodePsbtParams {
    pub psbt: String,
}

/// Decode PSBT
///
/// Deliberately not instrumented: a tracing span here would capture the raw
/// PSBT body.
pub async fn decode_psbt(
    client: Extension<ChainClient>,
    psbt: String,
) -> Result<impl IntoResponse, Error> {
    let txid = client.decode_psbt(&psbt).await?;

    Ok((StatusCode::OK, Json(txid)))
}

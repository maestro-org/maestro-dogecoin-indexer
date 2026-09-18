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

pub fn router() -> Router {
    Router::new()
        .route("/info", get(mempool_info))
        .route("/transactions", get(mempool_transactions))
        .route("/transactions/:tx_hash", get(mempool_transaction_details))
        .route(
            "/transactions/:tx_hash/ancestors",
            get(mempool_transaction_ancestors),
        )
        .route(
            "/transactions/:tx_hash/descendants",
            get(mempool_transaction_descendants),
        )
}

#[derive(Deserialize)]
pub struct GetTransactionsParams {
    pub tx_hash: String,
}

/// Mempool Info
pub async fn mempool_info(client: Extension<ChainClient>) -> Result<impl IntoResponse, Error> {
    let (x, y) = client.get_chain_tip().await?;

    let out = TimestampedResponse {
        data: client.get_mempool_info().await?,
        last_updated: LastUpdated {
            block_height: x,
            block_hash: y.to_hex(),
        },
    };

    Ok((StatusCode::OK, Json(out)))
}

/// List Mempool Transactions
pub async fn mempool_transactions(
    client: Extension<ChainClient>,
) -> Result<impl IntoResponse, Error> {
    let (x, y) = client.get_chain_tip().await?;

    let out = TimestampedResponse {
        data: client.get_mempool_transactions().await?,
        last_updated: LastUpdated {
            block_height: x,
            block_hash: y.to_hex(),
        },
    };

    Ok((StatusCode::OK, Json(out)))
}

/// Mempool Transaction Details
pub async fn mempool_transaction_details(
    Path(params): Path<GetTransactionsParams>,
    client: Extension<ChainClient>,
) -> Result<impl IntoResponse, Error> {
    let tx = Txid::from_hex(&params.tx_hash).map_err(|_| Error::InvalidHex(params.tx_hash))?;
    let (x, y) = client.get_chain_tip().await?;

    let out = TimestampedResponse {
        data: client.get_mempool_transaction_details(tx).await?,
        last_updated: LastUpdated {
            block_height: x,
            block_hash: y.to_hex(),
        },
    };

    Ok((StatusCode::OK, Json(out)))
}

/// List Mempool Transaction Ancestors
pub async fn mempool_transaction_ancestors(
    Path(params): Path<GetTransactionsParams>,
    client: Extension<ChainClient>,
) -> Result<impl IntoResponse, Error> {
    let tx = Txid::from_hex(&params.tx_hash).map_err(|_| Error::InvalidHex(params.tx_hash))?;
    let (x, y) = client.get_chain_tip().await?;

    let out = TimestampedResponse {
        data: client.get_mempool_transaction_ancestors(tx).await?,
        last_updated: LastUpdated {
            block_height: x,
            block_hash: y.to_hex(),
        },
    };

    Ok((StatusCode::OK, Json(out)))
}

/// List Mempool Transaction Descendants
pub async fn mempool_transaction_descendants(
    Path(params): Path<GetTransactionsParams>,
    client: Extension<ChainClient>,
) -> Result<impl IntoResponse, Error> {
    let tx = Txid::from_hex(&params.tx_hash).map_err(|_| Error::InvalidHex(params.tx_hash))?;
    let (x, y) = client.get_chain_tip().await?;

    let out = TimestampedResponse {
        data: client.get_mempool_transaction_descendants(tx).await?,
        last_updated: LastUpdated {
            block_height: x,
            block_hash: y.to_hex(),
        },
    };

    Ok((StatusCode::OK, Json(out)))
}

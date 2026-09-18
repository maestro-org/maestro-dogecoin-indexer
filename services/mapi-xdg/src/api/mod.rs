use crate::{
    client::client,
    error::Error,
    options::{Mode, Options},
    polyphony::PolyphonyWrapper,
};
use axum::{http::StatusCode, response::IntoResponse, routing::get, Extension, Router};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

mod addresses;
mod blocks;
mod dunes;
mod general;
mod inscriptions;
mod rpc;
mod transactions;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Dogecoin - Blockchain Indexer API",
        version = "0.0.1",
        description = "Dogecoin indexer endpoints: addresses, dunes, inscriptions and DRC-20. The same binary also proxies a set of node RPC endpoints (blocks, mempool, chain info) documented in the repository README.",
        license(
            name = "Apache 2.0",
            url = "https://www.apache.org/licenses/LICENSE-2.0.txt"
        )
    ),
    servers(
        (url = "http://localhost:3000", description = "Local instance")
    ),
    paths(
        healthcheck,
        addresses::total_balance_by_address::total_balance_by_address,
        addresses::utxos_by_address::utxos_by_address,
        addresses::txs_by_address::txs_by_address,
        inscriptions::content_by_inscription_id::content_by_inscription_id,
        inscriptions::drc20_by_address::drc20_by_address,
        inscriptions::drc20_holders_by_ticker::drc20_holders_by_ticker,
        inscriptions::drc20_info::drc20_info,
        inscriptions::inscription_info::inscription_info,
        inscriptions::list_drc20s::list_drc20s,
        inscriptions::inscriptions_by_address::inscriptions_by_address,
        inscriptions::transfer_inscriptions_by_address::transfer_inscriptions_by_address,
        dunes::dunes_by_address::dunes_by_address,
        dunes::dune_utxos_by_address::dune_utxos_by_address,
        dunes::info_by_dune::info_by_dune,
        dunes::utxos_by_dune::utxos_by_dune,
        dunes::holders_by_dune::holders_by_dune,
        dunes::list_dunes::list_dunes,
    ),
    components(schemas(
        crate::types::Utxo,
        crate::types::PaginatedUtxo,
        crate::types::inscriptions::Drc20Holder,
        crate::types::PaginatedDrc20Holder,
        crate::types::dunes::DuneUtxo,
        crate::types::TimestampedDuneQuantities,
        crate::types::TimestampedDuneInfo,
        crate::types::dunes::DuneIdAndName,
        crate::types::DuneAndAmount,
        crate::types::PaginatedDuneIdAndName,
        crate::types::PaginatedDuneUtxo,
        crate::types::PaginatedAddressDuneUtxo,
        crate::types::dunes::AddressDuneUtxo,
        crate::types::dunes::DuneInfo,
        crate::types::dunes::Terms,
        crate::types::dunes::DuneHolder,
        crate::types::PaginatedDuneHolder,
        crate::types::inscriptions::Drc20Ticker,
        crate::types::PaginatedDrc20Ticker,
        crate::types::InscriptionAndOffset,
        crate::types::TimestampedDrc20Quantities,
        crate::types::inscriptions::Drc20Terms,
        crate::types::inscriptions::Drc20Info,
        crate::types::TimestampedDrc20Info,
        crate::types::inscriptions::ContentBody,
        crate::types::PaginatedContentBody,
        crate::types::inscriptions::InscriptionByAddress,
        crate::types::PaginatedInscriptionByAddress,
        crate::types::inscriptions::InscriptionInfo,
        crate::types::TimestampedInscriptionInfo,
        crate::types::inscriptions::TransferInscriptionByAddress,
        crate::types::PaginatedTransferInscriptionByAddress,
        crate::types::InvolvedTransaction,
        crate::types::PaginatedInvolvedTransaction,
        crate::types::LastUpdated,
    )),
)]
pub struct APIDoc;

#[utoipa::path(
    get,
    path = "/healthcheck",
    responses(
        (status = 200, description= "Service is working"),
        (status = 500, description= "Internal Server Error")
    )
)]
async fn healthcheck(
    polyphony: Extension<PolyphonyWrapper>,
    _: Extension<Mode>,
) -> Result<impl IntoResponse, Error> {
    let utxos_encoder = &polyphony.utxos_by_script_hash_encoder()?;

    // --- start db snapshot at most recent timestamp
    let mut snapshot = polyphony.begin_snapshot_latest().await?;

    // --- fetch cursor key  for last updated
    let _ = snapshot.get_last_updated(utxos_encoder).await?;

    Ok((StatusCode::OK, "OK"))
}

pub async fn router(
    options: &Options,
    polyphony: PolyphonyWrapper,
    mode: Mode,
) -> Result<Router, Error> {
    let router = Router::new()
        .route("/healthcheck", get(healthcheck))
        .nest("/blocks", blocks::router())
        .nest("/addresses", addresses::router())
        .nest("/general", general::router())
        .nest("/transactions", transactions::router())
        .nest("/rpc/mempool", rpc::mempool::router())
        .nest("/rpc/transactions", rpc::transactions::router())
        .merge(dunes::router())
        .merge(inscriptions::router())
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", APIDoc::openapi()))
        .layer(Extension(client(options).await?))
        .layer(Extension(polyphony))
        .layer(Extension(mode));

    Ok(router)
}

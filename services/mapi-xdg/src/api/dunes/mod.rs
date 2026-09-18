use axum::{routing::get, Router};

pub mod dune_utxos_by_address;
pub mod dunes_by_address;
pub mod holders_by_dune;
pub mod info_by_dune;
pub mod list_dunes;
pub mod utxos_by_dune;

pub fn router() -> Router {
    Router::new()
        .route(
            "/addresses/:address/dunes",
            get(dunes_by_address::dunes_by_address),
        )
        .route(
            "/addresses/:address/dunes/:id",
            get(dune_utxos_by_address::dune_utxos_by_address),
        )
        .route("/assets/dunes", get(list_dunes::list_dunes))
        .route("/assets/dunes/:id", get(info_by_dune::info_by_dune))
        .route("/assets/dunes/:id/utxos", get(utxos_by_dune::utxos_by_dune))
        .route(
            "/assets/dunes/:id/holders",
            get(holders_by_dune::holders_by_dune),
        )
}

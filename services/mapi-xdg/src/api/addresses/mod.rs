use axum::{routing::get, Router};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

pub mod total_balance_by_address;
pub mod txs_by_address;
pub mod utxos_by_address;

pub fn router() -> Router {
    Router::new()
        .route(
            "/:address/balance",
            get(total_balance_by_address::total_balance_by_address),
        )
        .route("/:address/txs", get(txs_by_address::txs_by_address))
        .route("/:address/utxos", get(utxos_by_address::utxos_by_address))
}

#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ToSchema, IntoParams,
)]
pub struct QueryParams {
    #[serde(default)]
    /// Filters for UTXOs that contains inscriptions
    pub inscriptions: bool,
    /// Key for pagination
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ToSchema)]
pub struct PostUtxosBody {
    pub addresses: Vec<String>,
}

use axum::{routing::get, Router};

pub mod content_by_inscription_id;
pub mod drc20_by_address;
pub mod drc20_holders_by_ticker;
pub mod drc20_info;
pub mod inscription_info;
pub mod inscriptions_by_address;
pub mod list_drc20s;
pub mod transfer_inscriptions_by_address;

pub fn router() -> Router {
    Router::new()
        .route(
            "/addresses/:address/drc20",
            get(drc20_by_address::drc20_by_address),
        )
        .route(
            "/assets/drc20/:id/holders",
            get(drc20_holders_by_ticker::drc20_holders_by_ticker),
        )
        .route("/assets/drc20/:id", get(drc20_info::drc20_info))
        .route("/assets/drc20", get(list_drc20s::list_drc20s))
        .route(
            "/addresses/:address/transfer_inscriptions",
            get(transfer_inscriptions_by_address::transfer_inscriptions_by_address),
        )
        .route(
            "/addresses/:address/inscriptions",
            get(inscriptions_by_address::inscriptions_by_address),
        )
        .route(
            "/assets/inscriptions/:inscription_id",
            get(inscription_info::inscription_info),
        )
        .route(
            "/assets/inscriptions/:inscription_id/content_body",
            get(content_by_inscription_id::content_by_inscription_id),
        )
}

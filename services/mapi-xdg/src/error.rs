use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use timbre_xbt::TimbreError;
use tracing::error;

pub type MapiResult<T> = Result<T, Error>;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Node RPC error: {0}")]
    Bitcoin(#[from] bitcoincore_rpc_async::Error),

    #[error("Invalid hex value: {0}")]
    InvalidHex(String),

    #[error("Hex Error: {0}")]
    Hex(#[from] hex::FromHexError),

    #[error("Encode error: {0}")]
    Encode(#[from] bitcoincore_rpc_async::bitcoin::consensus::encode::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Unable to find user requested data")]
    NotFound,

    #[error("Users request/query was malformed: {0}")]
    MalformedRequest(String),

    #[error("TiKV error: {0}")]
    TiKV(#[from] tikv_client::Error),

    #[error("Missing expected data in storage: {0:?}")]
    MissingData(Vec<u8>),

    #[error("Unable to decode data from storage: {0:?} {1:?}")]
    MalformedData(Vec<u8>, Option<TimbreError>),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("RPC error: {0}")]
    Rpc(String),

    #[error("Unexpected mode: GenerateOpenApi")]
    GenerateOpenApiIsUnexpected(),
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        match self {
            Error::NotFound => (
                StatusCode::NOT_FOUND,
                format!("Unable to find requested data on-chain"),
            )
                .into_response(),
            Error::InvalidHex(e) => {
                (StatusCode::BAD_REQUEST, format!("Invalid Hash: {e}")).into_response()
            }
            Error::MalformedRequest(e) => (
                StatusCode::BAD_REQUEST,
                format!("Unable to parse request parameters: {e}"),
            )
                .into_response(),

            Error::Rpc(e) => {
                (StatusCode::BAD_REQUEST, format!("Node RPC error: {e}")).into_response()
            }
            _ => {
                error!("Internal server error: {}", self);

                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Internal server error"),
                )
                    .into_response()
            }
        }
    }
}

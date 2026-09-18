use axum::{extract::Path, response::IntoResponse, Extension, Json};
use reqwest::StatusCode;
use std::str::FromStr;
use timbre_xbt::{
    reducers::content_by_inscription_id::{
        Key as InscriptionInfoKey, Value as InscriptionInfoValue,
    },
    Reducer,
};

use crate::{
    client::ChainClient,
    error::Error,
    options::Mode,
    polyphony::PolyphonyWrapper,
    types::{inscriptions::InscriptionInfo, TimestampedResponse},
    util::{parse_inscription_id, MAX_CONTENT_PREVIEW},
};

#[utoipa::path(
    tag = "Inscriptions",
    get,
    path = "/assets/inscriptions/{inscription_id}",
    params(
        ("inscription_id" = String, Path, description = "Inscription ID", example="7d0a2dd897222913d58fc957b0429526117a0a61c964642fe93b077f328ccec1i0"),
    ),
    responses(
        (
            status = 200,
            description = "Requested data",
            body = TimestampedInscriptionInfo,
            example = json!(serde_json::Value::from_str(EXAMPLE_RESPONSE).unwrap())
        ),
        (status = 400, description = "Malformed query parameters"),
        (status = 404, description = "Requested entity not found on-chain"),
        (status = 500, description = "Internal server error"),
    )
)]
#[tracing::instrument(name = "INSCRIPTION_INFO", level = "info", skip(polyphony))]
/// Inscription Info
///
/// Information about an inscription. A preview of the content body is given only if its type is
/// "text/plain". For the whole content, use the complementary endpoint, namely
/// `/assets/inscriptions/{inscription_id}/content_body`.
pub async fn inscription_info(
    Path(inscription_id): Path<String>,
    _: Extension<ChainClient>,
    polyphony: Extension<PolyphonyWrapper>,
    mode: Extension<Mode>,
) -> Result<impl IntoResponse, Error> {
    let content_by_inscription_encoder = &polyphony.content_by_inscription_id_encoder()?;

    // --- start db snapshot at most recent timestamp
    let mut snapshot = polyphony.begin_snapshot_latest().await?;

    let last_updated = snapshot
        .get_last_updated(content_by_inscription_encoder)
        .await?;

    let parsed_key = parse_inscription_id(&inscription_id)?;

    let info = snapshot
        .get_reducer_key_maybe::<InscriptionInfoKey, InscriptionInfoValue>(
            content_by_inscription_encoder,
            &Reducer::ContentByInscriptionId,
            &InscriptionInfoKey {
                inscription_id: parsed_key,
            },
        )
        .await?
        .ok_or_else(|| Error::NotFound)?;

    let content_type: Option<String> = String::from_utf8(info.content_type.clone()).ok();
    let (content_body_preview, content_length) = if content_type == Some(String::from("text/plain"))
    {
        let mut content_body = info.content_body.clone();
        let length = content_body.len();
        // truncate content body preview
        content_body.truncate(MAX_CONTENT_PREVIEW);
        (
            Some(String::from_utf8_lossy(&content_body).to_string()),
            length as u64,
        )
    } else {
        (None, info.content_body.len() as u64)
    };

    let out = TimestampedResponse {
        data: InscriptionInfo {
            inscription_id,
            created_at: info.created_at,
            inscription_num: info.inscription_num,
            content_type,
            content_body_preview,
            content_length,
        },
        last_updated,
    };

    Ok((StatusCode::OK, Json(out)))
}

static EXAMPLE_RESPONSE: &str = r##"{
    "data": {
        "inscription_id": "7d0a2dd897222913d58fc957b0429526117a0a61c964642fe93b077f328ccec1i0",
        "created_at": 842166,
        "inscription_num": 234567,
        "content_type": "text/html;charset=utf-8",
        "content_body_preview": null,
        "content_length": 3035
    },
    "last_updated": {
        "block_hash": "1c3e7cd9e46bd6d0adb1ae0b52ec1a1ddfaa0cb61a41a1b1c26f0d1b0f42a06c",
        "block_height": 5146710
    }
}"##;

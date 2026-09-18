use axum::{
    extract::{Path, Query},
    response::IntoResponse,
    Extension, Json,
};
use base64::{engine::general_purpose, Engine};
use reqwest::StatusCode;
use std::cmp::min;
use std::str::FromStr;
use timbre_xbt::{
    reducers::content_by_inscription_id::{
        Cursor as ContentByInscriptionIdCursor, Key as ContentByInscriptionIdKey,
        Value as ContentByInscriptionIdValue,
    },
    Decode, Encode, Reducer,
};

use crate::{
    client::ChainClient,
    error::Error,
    options::Mode,
    polyphony::PolyphonyWrapper,
    types::{inscriptions::ContentBody, CountParam, CursorPaginationParams, PaginatedContent},
    util::{parse_inscription_id, DEFAULT_CONTENT_BODY_SIZE, MAX_CONTENT_BODY_SIZE},
};

#[utoipa::path(
    tag = "Inscriptions",
    get,
    path = "/assets/inscriptions/{inscription_id}/content_body",
    params(
        ("inscription_id" = String, Path, description = "Inscription ID", example="7d0a2dd897222913d58fc957b0429526117a0a61c964642fe93b077f328ccec1i0"),

        ("count" = inline(Option<CountParam>), Query, description = "The max number of bytes per page"),
        ("cursor" = inline(Option<String>), Query, description = "Pagination cursor string: the offset in the content body. Use the cursor to query for the next page"),
    ),
    responses(
        (
            status = 200,
            description = "Requested data",
            body = PaginatedContentBody,
            example = json!(serde_json::Value::from_str(EXAMPLE_RESPONSE).unwrap())
        ),
        (status = 400, description = "Malformed query parameters"),
        (status = 404, description = "Requested entity not found on-chain"),
        (status = 500, description = "Internal server error"),
    )
)]
#[tracing::instrument(name = "CONTENT_BY_INSCRIPTION_ID", level = "info", skip(polyphony))]
/// Content by Inscription ID
///
/// Paginated response of the content body byte array of an inscription. This endpoint is
/// complementary to the "Inscription Information" endpoint.
pub async fn content_by_inscription_id(
    Path(inscription_id): Path<String>,
    Query(page_params): Query<CursorPaginationParams>,
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

    // --- parse inscription ID
    let parsed_key: ([u8; 32], u32) = parse_inscription_id(&inscription_id)?;

    // --- fetch inscription info
    let info: ContentByInscriptionIdValue = snapshot
        .get_reducer_key_maybe::<ContentByInscriptionIdKey, ContentByInscriptionIdValue>(
            content_by_inscription_encoder,
            &Reducer::ContentByInscriptionId,
            &ContentByInscriptionIdKey {
                inscription_id: parsed_key,
            },
        )
        .await?
        .ok_or_else(|| Error::NotFound)?;

    let total_length: u64 = info.content_body.len() as u64;

    // --- parse pagination params
    let cursor: u64 = match page_params.cursor {
        Some(c) => match ContentByInscriptionIdCursor::decode_base64(&c) {
            Ok((res, _)) => res.offset,
            Err(_) => {
                return Err(Error::MalformedRequest(
                    "Error while decoding cursor".into(),
                ))
            }
        },
        None => 0,
    };
    if cursor > total_length {
        return Err(Error::MalformedRequest(
            "Cursor exceeds content length".into(),
        ));
    }

    let count: u64 = page_params
        .count
        .map(|c| c.0 as u64)
        .unwrap_or(DEFAULT_CONTENT_BODY_SIZE);
    if count == 0 {
        return Err(Error::MalformedRequest(
            "count must be greater than 0".into(),
        ));
    }
    if count > MAX_CONTENT_BODY_SIZE {
        return Err(Error::MalformedRequest("Max response size exceeded".into()));
    }

    let page_start = cursor as usize;
    let page_end = min(cursor + count, total_length) as usize;
    let content_body_page =
        general_purpose::STANDARD.encode(&info.content_body[page_start..page_end]);

    let remaining_bytes: u64 = total_length.saturating_sub(cursor + count);

    let next_cursor = if remaining_bytes != 0 {
        Some(
            ContentByInscriptionIdCursor {
                offset: cursor + count,
            }
            .encode_base64(),
        )
    } else {
        None
    };

    let out = PaginatedContent {
        data: ContentBody {
            content_body_page,
            total_length,
            remaining_bytes,
        },
        last_updated,
        next_cursor,
    };

    Ok((StatusCode::OK, Json(out)))
}

static EXAMPLE_RESPONSE: &str = r##"{
    "data": {
        "content_body_page": "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8gISIjJCUmJygpKissLS4vMDEyMzQ1Njc4OTo7PD0+P0BBQkNERUZHSElKS0xNTk9QUVJTVFVWV1hZWltcXV5fYGFiYw==",
        "total_length": 3035,
        "remaining_bytes": 2935
    },
    "last_updated": {
        "block_hash": "1c3e7cd9e46bd6d0adb1ae0b52ec1a1ddfaa0cb61a41a1b1c26f0d1b0f42a06c",
        "block_height": 5146710
    },
    "next_cursor": "AAAAAAAAAGQ"
}"##;

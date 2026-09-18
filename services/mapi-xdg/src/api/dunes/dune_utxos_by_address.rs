use axum::{
    extract::{Path, Query},
    response::IntoResponse,
    Extension, Json,
};
use bitcoin::{hashes::Hash, Script, Txid};
use reqwest::StatusCode;
use std::str::FromStr;
use tikv_client::KvPair;
use timbre_xbt::{
    reducers::{
        etching_by_rune_id,
        utxos_by_rune_id::{
            Cursor as UtxosByRuneIdCursor, Key as UtxosByRuneIdKey, Value as UtxosByRuneIdValue,
        },
    },
    Decode, Encode, Reducer,
};

use crate::{
    client::ChainClient,
    error::Error,
    options::Mode,
    polyphony::{PolyphonyWrapper, Scanner},
    types::{
        dunes::AddressDuneUtxo, CountParam, HeightPaginationParams, OrderParam, PaginatedResponse,
    },
    util::{decimal, parse_address_or_script_bytes, ParsedPaginationParams, RuneIdentifier},
};

#[utoipa::path(
    tag = "Addresses",
    get,
    path = "/addresses/{address}/dunes/{dune}",
    params(
        ("address" = String, Path, description = "Dogecoin address or hex encoded script pubkey", example="DNG3G7pKc1DgciyWs36GFUnVyvJieDhKdb"),
        ("dune" = String, Path, description = "Dune, specified either by the Dune ID (etching block number and transaction index) or name (spaced or un-spaced)", example="2519999:31"),

        ("count" = inline(Option<CountParam>), Query, description = "The max number of results per page"),

        ("order" = inline(Option<OrderParam>), Query, description = "The order in which the results are sorted (by height at which UTxO was produced)"),
        ("from" = inline(Option<u64>), Query, description = "Return only UTxOs created on or after a specific height"),
        ("to" = inline(Option<u64>), Query, description = "Return only UTxOs created on or before a specific height"),

        ("cursor" = inline(Option<String>), Query, description = "Pagination cursor string, use the cursor included in a page of results to fetch the next page"),
    ),
    responses(
        (
            status = 200,
            description = "Requested data",
            body = PaginatedAddressDuneUtxo,
            example = json!(serde_json::Value::from_str(EXAMPLE_RESPONSE).unwrap())
        ),
        (status = 400, description = "Malformed query parameters"),
        (status = 404, description = "Requested entity not found on-chain"),
        (status = 500, description = "Internal server error"),
    )
)]
#[tracing::instrument(name = "DUNE_UTXOS_BY_ADDRESS", level = "info", skip(polyphony))]
/// Dune UTxOs by Address and Dune
///
/// Return all UTxOs controlled by the specified address or script pubkey which
/// contain some of a specific kind of dune.
pub async fn dune_utxos_by_address(
    page_params: Query<HeightPaginationParams>,
    Path((addr_or_pk, rune_id)): Path<(String, String)>,
    _: Extension<ChainClient>,
    polyphony: Extension<PolyphonyWrapper>,
    mode: Extension<Mode>,
) -> Result<impl IntoResponse, Error> {
    let utxos_encoder = &polyphony.utxos_by_rune_id_encoder()?;
    let etching_encoder = &polyphony.etching_by_rune_id_encoder()?;

    // --- start db snapshot at most recent timestamp

    let mut snapshot = polyphony.begin_snapshot_latest().await?;

    // -- parse and try decode user params

    let (_, script_bytes) =
        parse_address_or_script_bytes(&polyphony, &mut snapshot, addr_or_pk, mode.0).await?;

    let script = Script::from_bytes(&script_bytes);
    let script_hash = script.script_hash().to_byte_array();

    let rune_id = match RuneIdentifier::parse(rune_id)? {
        RuneIdentifier::Id(id) => id,
        RuneIdentifier::Name(n) => polyphony
            .resolve_rune_name(&mut snapshot, n)
            .await?
            .unwrap_or_default(), // return empty vec instead of 404
    };

    let page_params = ParsedPaginationParams::parse::<_, UtxosByRuneIdCursor>(
        page_params.0,
        utxos_encoder,
        &Reducer::UtxosByRuneId,
        Some(rune_id),
    )?;

    // --- fetch cursor key for last updated

    let last_updated = snapshot.get_last_updated(utxos_encoder).await?;

    // --- initialise `next_cursor`

    let mut next_cursor = None;

    // --- get divisibility (or return empty vec if rune not found)

    let dec = if let Some(etch) = snapshot
        .get_reducer_key_maybe::<_, etching_by_rune_id::Value>(
            etching_encoder,
            &Reducer::EtchingByRuneId,
            &etching_by_rune_id::Key { rune_id },
        )
        .await?
    {
        etch.divisibility.unwrap_or(0) as usize
    } else {
        return Ok((
            StatusCode::OK,
            Json(PaginatedResponse {
                data: vec![],
                last_updated,
                next_cursor: None,
            }),
        ));
    };

    // --- scan keys for this page

    // filter for utxos controlled by the specified address
    let filter = |kv: &KvPair| {
        let Ok((value, _)) = UtxosByRuneIdValue::decode(&kv.1) else {
            return false;
        };

        value.script_hash == script_hash
    };

    let kvs = Scanner::new(page_params.key_range())
        .count(page_params.count() + 1)
        .order(page_params.order())
        .execute_with_filter::<UtxosByRuneIdKey, UtxosByRuneIdValue, _>(&mut snapshot, Some(filter))
        .await?;

    // --- process fetched kvs

    let mut kvs = kvs.into_iter().enumerate();

    let mut utxos: Vec<AddressDuneUtxo> = Vec::new();

    while let Some((i, (key, value))) = kvs.next() {
        // if this is the last result of the page, check if there is a subsequent
        // result (and therefore we need to return a cursor for next page)
        if i == (page_params.count() - 1) && kvs.next().is_some() {
            next_cursor = Some(
                UtxosByRuneIdCursor {
                    height: key.height,
                    utxo_hash: key.utxo_hash,
                    utxo_index: key.utxo_index,
                }
                .encode_base64(),
            );
        };

        utxos.push(AddressDuneUtxo {
            txid: Txid::from_byte_array(key.utxo_hash).to_string(),
            vout: key.utxo_index,
            satoshis: value.satoshis.to_string(),
            confirmations: last_updated.block_height.saturating_sub(key.height),
            height: key.height,
            dune_amount: decimal(value.rune_quantity, dec),
        })
    }

    let out = PaginatedResponse {
        data: utxos,
        last_updated,
        next_cursor,
    };

    Ok((StatusCode::OK, Json(out)))
}

static EXAMPLE_RESPONSE: &str = r##"{
    "data": [{
        "txid": "cd990aa533ffb733934344fafbc07350ba5945a5a50fca3adb9845573a99e545",
        "vout": 1,
        "satoshis": "100000",
        "confirmations": 3597,
        "height": 5280972,
        "dune_amount": "0.00001000"
    }],
    "last_updated": {
        "block_hash": "e45be2dac59c8b6a2090a6cee4c5c035c6d285b9ca4690fe7d454d9d6ad67a42",
        "block_height": 5284569
    },
    "next_cursor": null
}"##;

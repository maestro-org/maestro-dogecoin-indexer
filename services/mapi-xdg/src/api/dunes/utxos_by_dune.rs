use axum::{
    extract::{Path, Query},
    response::IntoResponse,
    Extension, Json,
};
use bitcoin::{hashes::Hash, Txid};
use reqwest::StatusCode;
use std::str::FromStr;
use timbre_xbt::{
    reducers::{
        etching_by_rune_id,
        utxos_by_rune_id::{
            Cursor as UtxosByRuneIdCursor, Key as UtxosByRuneIdKey, Value as UtxosByRuneIdValue,
        },
    },
    Encode, Reducer,
};

use crate::{
    client::ChainClient,
    error::Error,
    options::Mode,
    polyphony::{PolyphonyWrapper, Scanner},
    types::{dunes::DuneUtxo, CountParam, HeightPaginationParams, OrderParam, PaginatedResponse},
    util::{decimal, ParsedHeightPaginationParams, RuneIdentifier},
};

#[utoipa::path(
    tag = "Dunes",
    get,
    path = "/assets/dunes/{dune}/utxos",
    params(
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
            body = PaginatedDuneUtxo,
            example = json!(serde_json::Value::from_str(EXAMPLE_RESPONSE).unwrap())
        ),
        (status = 400, description = "Malformed query parameters"),
        (status = 404, description = "Requested entity not found on-chain"),
        (status = 500, description = "Internal server error"),
    )
)]
#[tracing::instrument(name = "UTXOS_BY_DUNE", level = "info", skip(polyphony))]
/// UTxOs by Dunes
///
/// Returns a list of UTxOs which contain some of the specified Dune.
pub async fn utxos_by_dune(
    page_params: Query<HeightPaginationParams>,
    Path(rune_id): Path<String>,
    _: Extension<ChainClient>,
    polyphony: Extension<PolyphonyWrapper>,
    mode: Extension<Mode>,
) -> Result<impl IntoResponse, Error> {
    let utxos_encoder = &polyphony.utxos_by_rune_id_encoder()?;
    let etching_encoder = &polyphony.utxos_by_rune_id_encoder()?;
    let script_by_script_hash_encoder = &polyphony.script_by_script_hash_encoder()?;

    // --- start db snapshot at most recent timestamp

    let mut snapshot = polyphony.begin_snapshot_latest().await?;

    // -- parse and try decode user params

    let dune_id = match RuneIdentifier::parse(rune_id)? {
        RuneIdentifier::Id(id) => id,
        RuneIdentifier::Name(n) => polyphony
            .resolve_rune_name(&mut snapshot, n)
            .await?
            .unwrap_or_default(), // return empty vec instead of 404
    };

    let page_params = ParsedHeightPaginationParams::parse::<_, UtxosByRuneIdCursor>(
        page_params.0,
        utxos_encoder,
        &Reducer::UtxosByRuneId,
        Some(dune_id),
    )?;

    // --- fetch cursor key  for last updated

    let last_updated = snapshot.get_last_updated(utxos_encoder).await?;

    // --- initialise `next_cursor`

    let mut next_cursor = None;

    // --- scan keys for this page (max count plus one to see if there is another page)

    let kvs = Scanner::new(page_params.key_range())
        .count(page_params.count() + 1)
        .order(page_params.order())
        .execute::<UtxosByRuneIdKey, UtxosByRuneIdValue>(&mut snapshot)
        .await?;

    let dec = if let Some(etch) = snapshot
        .get_reducer_key_maybe::<_, etching_by_rune_id::Value>(
            etching_encoder,
            &Reducer::EtchingByRuneId,
            &etching_by_rune_id::Key { rune_id: dune_id },
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

    // --- process fetched kvs

    let mut kvs = kvs.into_iter().enumerate();

    let mut utxos: Vec<DuneUtxo> = Vec::new();

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

        let (address, script) = polyphony
            .resolve_script_hash(
                &mut snapshot,
                mode.0,
                value.script_hash,
                script_by_script_hash_encoder,
            )
            .await?;

        utxos.push(DuneUtxo {
            txid: Txid::from_byte_array(key.utxo_hash).to_string(),
            vout: key.utxo_index,
            address: address.map(|x| x.to_string()),
            script_pubkey: hex::encode(script),
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
        "address": "D8UC42uehZdBYAfMBJumQKuBNs9vvarxtt",
        "script_pubkey": "76a9142484e20925fa4d816f04b313b07162a0f5f0961a88ac",
        "satoshis": "100000",
        "confirmations": 1336,
        "height": 5280972,
        "dune_amount": "0.00001000"
    }, {
        "txid": "55e0351b8904a44a337491da372d53138991f642a4eafb5d4a86c66cd30eb1fd",
        "vout": 1,
        "address": "D8vgB8Viu2ZGMKBUHh4khtS9FzR6B1D4cK",
        "script_pubkey": "76a91429872c5e5a0a611372348ae5afd278a446dd155f88ac",
        "satoshis": "100000",
        "confirmations": 1323,
        "height": 5280985,
        "dune_amount": "1000.00000000"
    }],
    "last_updated": {
        "block_hash": "caf5db9cfd306cccaec5f4fc92a3f47d9aa8e4007b3b74b6bdf97aab5f68808f",
        "block_height": 5282308
    },
    "next_cursor": null
}"##;

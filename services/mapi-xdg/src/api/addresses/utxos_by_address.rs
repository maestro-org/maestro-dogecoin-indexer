use crate::{
    types::DuneAndAmount,
    util::{decimal, parse_address_or_script_bytes},
};
use axum::{
    extract::{Path, Query},
    response::IntoResponse,
    Extension, Json,
};
use bitcoin::{hashes::Hash, Script, Txid};
use reqwest::StatusCode;
use serde::Deserialize;
use std::str::FromStr;
use tikv_client::KvPair;
use timbre_xbt::{
    reducers::{
        etching_by_rune_id, inscriptions_by_utxo, runes_by_utxo,
        utxos_by_script_hash::{
            Cursor as UtxosByScriptHashCursor, Key as UtxosByScriptHashKey,
            Value as UtxosByScriptHashValue,
        },
    },
    Encode, Reducer,
};

use crate::{
    client::ChainClient,
    error::Error,
    options::Mode,
    polyphony::{PolyphonyWrapper, Scanner},
    types::{
        CountParam, HeightPaginationParams, InscriptionAndOffset, OrderParam, PaginatedResponse,
        Utxo,
    },
    util::ParsedHeightPaginationParams,
};

#[derive(Debug, Deserialize)]
pub struct Params {
    pub filter_dust: Option<bool>,
    pub filter_dust_threshold: Option<u64>,
    pub exclude_metaprotocols: Option<bool>,
}

#[utoipa::path(
    tag = "Addresses",
    get,
    path = "/addresses/{address}/utxos",
    params(
        ("address" = String, Path, description = "Dogecoin address or hex encoded script pubkey", example="DNG3G7pKc1DgciyWs36GFUnVyvJieDhKdb"),
        ("filter_dust" = Option<bool>, Query, description = "Ignore UTxOs containing less than 100000 shibes"),
        ("filter_dust_threshold" = Option<u64>, Query, description = "Ignore UTxOs containing less than specified shibes"),
        ("exclude_metaprotocols" = Option<bool>, Query, description = "Exclude metaprotocol UTxOs (UTxOs involved in metaprotocols like dunes or inscriptions)"),

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
            body = PaginatedUtxo,
            example = json!(serde_json::Value::from_str(EXAMPLE_RESPONSE).unwrap())
        ),
        (status = 400, description = "Malformed query parameters"),
        (status = 404, description = "Requested entity not found on-chain"),
        (status = 500, description = "Internal server error"),
    )
)]
#[tracing::instrument(name = "UTXOS_BY_ADDRESS", level = "info", skip(polyphony))]
/// UTxOs by Address
///
/// List of all UTxOs which reside at the specified address or script pubkey.
pub async fn utxos_by_address(
    page_params: Query<HeightPaginationParams>,
    Path(addr_or_pk): Path<String>,
    params: Query<Params>,
    _: Extension<ChainClient>,
    polyphony: Extension<PolyphonyWrapper>,
    mode: Extension<Mode>,
) -> Result<impl IntoResponse, Error> {
    let utxos_encoder = &polyphony.utxos_by_script_hash_encoder()?;
    let runes_by_utxo_encoder = &polyphony.runes_by_utxo_encoder()?;
    let etching_by_rune_id_encoder = &polyphony.etching_by_rune_id_encoder()?;
    let inscriptions_by_utxo_encoder = &polyphony.inscriptions_by_utxo_encoder()?;

    // --- start db snapshot at most recent timestamp
    let mut snapshot = polyphony.begin_snapshot_latest().await?;

    // --- fetch cursor key  for last updated
    let last_updated = snapshot.get_last_updated(utxos_encoder).await?;

    // --- initialise `next_cursor`
    let mut next_cursor = None;

    // -- parse and try decode address
    let address;
    let script_bytes;

    match parse_address_or_script_bytes(&polyphony, &mut snapshot, addr_or_pk, mode.0).await {
        Ok((addr, bytes)) => {
            address = addr;
            script_bytes = bytes;
        }
        Err(Error::NotFound) => {
            // --- user param is a Dogecoin address, but the corresponding script pub key could not
            // --- be found in store because this address has no transaction history
            let out = PaginatedResponse {
                data: vec![],
                last_updated,
                next_cursor,
            };
            return Ok((StatusCode::OK, Json(out)));
        }
        Err(e) => return Err(e),
    }

    let script = Script::from_bytes(&script_bytes);
    let script_hash = script.script_hash();

    let page_params = ParsedHeightPaginationParams::parse::<_, UtxosByScriptHashCursor>(
        page_params.0,
        utxos_encoder,
        &Reducer::UtxosByScriptHash,
        Some(script_hash.to_byte_array()),
    )?;

    let filter_dust = params.filter_dust.unwrap_or(false);

    let threshold = params
        .filter_dust_threshold
        .unwrap_or(if filter_dust { 100_000 } else { 0 });

    let filter = if threshold > 0 {
        Some(|kv: &KvPair| {
            kv.1.get(0..8)
                .and_then(|b| b.try_into().ok())
                .map(u64::from_be_bytes)
                .is_some_and(|v| v >= threshold)
        })
    } else {
        None
    };

    let excl_metaprotocols: bool = params.exclude_metaprotocols.unwrap_or(false);

    // --- scan keys

    let scanner = Scanner::new(page_params.key_range())
        .count(page_params.count() + 1)
        .order(page_params.order());

    let kvs: Vec<(UtxosByScriptHashKey, UtxosByScriptHashValue)> = if excl_metaprotocols {
        scanner
            .get_non_metaprotocol_utxos(
                &mut snapshot,
                filter,
                runes_by_utxo_encoder,
                inscriptions_by_utxo_encoder,
            )
            .await?
    } else {
        scanner.execute_with_filter(&mut snapshot, filter).await?
    };

    // --- process fetched kvs
    let mut kvs = kvs.into_iter().enumerate();

    let mut utxos: Vec<Utxo> = Vec::new();

    while let Some((i, (key, value))) = kvs.next() {
        // if this is the last result of the page, check if there is a subsequent
        // result (and therefore we need to return a cursor for next page)
        if i == (page_params.count() - 1) && kvs.next().is_some() {
            next_cursor = Some(
                UtxosByScriptHashCursor {
                    height: key.height,
                    utxo_hash: key.utxo_hash,
                    utxo_index: key.utxo_index,
                }
                .encode_base64(),
            );
        };

        let dunes: Vec<DuneAndAmount> = if excl_metaprotocols {
            // if metaprotocol UTxOs are excluded, it's certain this UTxO has no Dunes
            vec![]
        } else {
            // build Dunes information for this UTxO
            let fetched_dunes = snapshot
                .get_reducer_key_maybe::<_, runes_by_utxo::Value>(
                    runes_by_utxo_encoder,
                    &Reducer::RunesByUtxo,
                    &runes_by_utxo::Key {
                        utxo_hash: key.utxo_hash,
                        utxo_index: key.utxo_index,
                    },
                )
                .await?
                .map(|x| x.runes)
                .unwrap_or_default();

            let mut out_dunes = Vec::new();

            for (dune_id, amount) in fetched_dunes {
                let dec = snapshot
                    .get_reducer_key::<_, etching_by_rune_id::Value>(
                        etching_by_rune_id_encoder,
                        &Reducer::EtchingByRuneId,
                        &etching_by_rune_id::Key { rune_id: dune_id },
                    )
                    .await?
                    .divisibility
                    .unwrap_or(0) as usize;

                out_dunes.push(DuneAndAmount {
                    dune_id: format!("{}:{}", dune_id.0, dune_id.1),
                    amount: decimal(amount, dec),
                })
            }
            out_dunes
        };

        let inscriptions: Vec<InscriptionAndOffset> = if excl_metaprotocols {
            // if metaprotocol UTxOs are excluded, it's certain this UTxO has no inscriptions
            vec![]
        } else {
            // build inscriptions for this UTxO
            let mut inscriptions = snapshot
                .get_reducer_key_maybe::<_, inscriptions_by_utxo::Value>(
                    inscriptions_by_utxo_encoder,
                    &Reducer::InscriptionsByUtxo,
                    &inscriptions_by_utxo::Key {
                        utxo_hash: key.utxo_hash,
                        utxo_index: key.utxo_index,
                    },
                )
                .await?
                .map(|x| x.inscriptions)
                .unwrap_or_default();
            inscriptions.sort_by_key(|(offset, _)| *offset);
            inscriptions
                .into_iter()
                .map(|(offset, inscription)| InscriptionAndOffset {
                    offset,
                    inscription_id: format!(
                        "{}i{}",
                        Txid::from_byte_array(inscription.0),
                        inscription.1
                    ),
                })
                .collect()
        };

        utxos.push(Utxo {
            txid: Txid::from_byte_array(key.utxo_hash).to_string(),
            vout: key.utxo_index,
            address: address.as_ref().map(|x| x.to_string()),
            script_pubkey: script.to_hex_string(),
            satoshis: value.satoshis.to_string(),
            confirmations: last_updated.block_height.saturating_sub(key.height),
            height: key.height,
            dunes,
            inscriptions,
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
        "txid": "ee4b62b6e221eb92ea821ecb7932eb325647d9fdce5632747905146a561b8a4b",
        "vout": 1,
        "address": "DEvbE2KyPKAzDsj511wijyF1NSBN9qMcix",
        "script_pubkey": "76a9146b53cbbe67beef7079e6ea3e39bdfdc34bf2707b88ac",
        "satoshis": "100000",
        "confirmations": 121,
        "height": 5095478,
        "dunes": [{
            "dune_id": "5095478:8",
            "amount": "5.00000000"
        }],
        "inscriptions": []
    }, {
        "txid": "ee4b62b6e221eb92ea821ecb7932eb325647d9fdce5632747905146a561b8a4b",
        "vout": 2,
        "address": "DEvbE2KyPKAzDsj511wijyF1NSBN9qMcix",
        "script_pubkey": "76a9146b53cbbe67beef7079e6ea3e39bdfdc34bf2707b88ac",
        "satoshis": "1999441400000",
        "confirmations": 121,
        "height": 5095478,
        "dunes": [],
        "inscriptions": []
    }],
    "last_updated": {
        "block_hash": "cedd9ed0572e357af2e383382053ee40cb0ab38a07f58bae74314117c878fbd5",
        "block_height": 5095599
    },
    "next_cursor": null
}"##;

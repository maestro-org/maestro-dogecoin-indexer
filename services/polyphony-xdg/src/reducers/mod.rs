use bitcoin::{Block, OutPoint, Transaction, Txid};
use gasket::runtime::spawn_stage;
use serde::Deserialize;
use std::time::Duration;

use crate::{
    bootstrap,
    crosscut::{self, Point},
    model::{self, StorageActionPayload},
};

type InputPort = gasket::messaging::tokio::InputPort<model::EnrichedBlockPayload>;
type OutputPort = gasket::messaging::tokio::OutputPort<StorageActionPayload>;

mod worker;

pub mod balances_by_brc20;
pub mod balances_by_rune_id;
pub mod brc20_balances_by_script_hash;
pub mod brc20_terms_by_ticker;
pub mod content_by_inscription_id;
pub mod etching_by_rune_id;
pub mod inscription_utxos_by_script_hash;
pub mod inscriptions_by_script_hash;
pub mod inscriptions_by_utxo;
pub mod mints_by_rune_id;
pub mod rune_balances_by_script_hash;
pub mod rune_id_by_rune_name;
pub mod runes_by_utxo;
pub mod sat_balance_by_script_hash;
pub mod script_by_script_hash;
pub mod script_hash_by_address_payload_hash;
pub mod transfer_inscriptions_by_script_hash;
pub mod txs_by_script_hash;
pub mod utxos_by_rune_id;
pub mod utxos_by_script_hash;

#[derive(Deserialize)]
#[serde(tag = "type")]
pub enum Config {
    BalancesByBrc20(balances_by_brc20::Config),
    BalancesByRuneId(balances_by_rune_id::Config),
    Brc20BalancesByScriptHash(brc20_balances_by_script_hash::Config),
    Brc20TermsByTicker(brc20_terms_by_ticker::Config),
    EtchingByRuneId(etching_by_rune_id::Config),
    InscriptionUtxosByScriptHash(inscription_utxos_by_script_hash::Config),
    ContentByInscriptionId(content_by_inscription_id::Config),
    InscriptionsByScriptHash(inscriptions_by_script_hash::Config),
    InscriptionsByUtxo(inscriptions_by_utxo::Config),
    MintsByRuneId(mints_by_rune_id::Config),
    RuneBalancesByScriptHash(rune_balances_by_script_hash::Config),
    RuneIdByRuneName(rune_id_by_rune_name::Config),
    RunesByUtxo(runes_by_utxo::Config),
    SatBalanceByScriptHash(sat_balance_by_script_hash::Config),
    ScriptByScriptHash(script_by_script_hash::Config),
    ScriptHashByAddressPayloadHash(script_hash_by_address_payload_hash::Config),
    TransferInscriptionsByScriptHash(transfer_inscriptions_by_script_hash::Config),
    TxsByScriptHash(txs_by_script_hash::Config),
    UtxosByRuneId(utxos_by_rune_id::Config),
    UtxosByScriptHash(utxos_by_script_hash::Config),
}

impl Config {
    fn plugin(self, policy: &crosscut::policies::RuntimePolicy) -> Reducer {
        match self {
            Config::BalancesByBrc20(c) => c.plugin(policy),
            Config::BalancesByRuneId(c) => c.plugin(policy),
            Config::Brc20BalancesByScriptHash(c) => c.plugin(policy),
            Config::Brc20TermsByTicker(c) => c.plugin(),
            Config::EtchingByRuneId(c) => c.plugin(),
            Config::InscriptionUtxosByScriptHash(c) => c.plugin(policy),
            Config::ContentByInscriptionId(c) => c.plugin(),
            Config::InscriptionsByScriptHash(c) => c.plugin(policy),
            Config::InscriptionsByUtxo(c) => c.plugin(),
            Config::MintsByRuneId(c) => c.plugin(),
            Config::RuneBalancesByScriptHash(c) => c.plugin(policy),
            Config::RuneIdByRuneName(c) => c.plugin(),
            Config::RunesByUtxo(c) => c.plugin(),
            Config::SatBalanceByScriptHash(c) => c.plugin(policy),
            Config::ScriptByScriptHash(c) => c.plugin(),
            Config::ScriptHashByAddressPayloadHash(c) => c.plugin(),
            Config::TransferInscriptionsByScriptHash(c) => c.plugin(policy),
            Config::TxsByScriptHash(c) => c.plugin(policy),
            Config::UtxosByRuneId(c) => c.plugin(policy),
            Config::UtxosByScriptHash(c) => c.plugin(policy),
        }
    }
}

pub struct Bootstrapper {
    input: InputPort,
    output: OutputPort,
    reducers: Vec<Reducer>,
}

impl Bootstrapper {
    pub fn new(configs: Vec<Config>, policy: &crosscut::policies::RuntimePolicy) -> Self {
        Self {
            reducers: configs.into_iter().map(|x| x.plugin(policy)).collect(),
            input: Default::default(),
            output: Default::default(),
        }
    }

    pub fn borrow_input_port(&mut self) -> &'_ mut InputPort {
        &mut self.input
    }

    pub fn borrow_output_port(&mut self) -> &'_ mut OutputPort {
        &mut self.output
    }

    pub fn spawn_stages(self, pipeline: &mut bootstrap::Pipeline, timeout: u64) {
        let worker = worker::Worker::new(self.reducers, self.input, self.output);
        pipeline.register_stage(spawn_stage(
            worker,
            gasket::runtime::Policy {
                tick_timeout: Some(Duration::from_secs(timeout)),
                ..Default::default()
            },
            Some("reducers"),
        ));
    }
}

pub enum Reducer {
    BalancesByBrc20(balances_by_brc20::Reducer),
    BalancesByRuneId(balances_by_rune_id::Reducer),
    Brc20BalancesByScriptHash(brc20_balances_by_script_hash::Reducer),
    Brc20TermsByTicker(brc20_terms_by_ticker::Reducer),
    EtchingByRuneId(etching_by_rune_id::Reducer),
    InscriptionUtxosByScriptHash(inscription_utxos_by_script_hash::Reducer),
    ContentByInscriptionId(content_by_inscription_id::Reducer),
    InscriptionsByScriptHash(inscriptions_by_script_hash::Reducer),
    InscriptionsByUtxo(inscriptions_by_utxo::Reducer),
    MintsByRuneId(mints_by_rune_id::Reducer),
    RuneBalancesByScriptHash(rune_balances_by_script_hash::Reducer),
    RuneIdByRuneName(rune_id_by_rune_name::Reducer),
    RunesByUtxo(runes_by_utxo::Reducer),
    SatBalanceByScriptHash(sat_balance_by_script_hash::Reducer),
    ScriptByScriptHash(script_by_script_hash::Reducer),
    ScriptHashByAddressPayloadHash(script_hash_by_address_payload_hash::Reducer),
    TransferInscriptionsByScriptHash(transfer_inscriptions_by_script_hash::Reducer),
    TxsByScriptHash(txs_by_script_hash::Reducer),
    UtxosByRuneId(utxos_by_rune_id::Reducer),
    UtxosByScriptHash(utxos_by_script_hash::Reducer),
}

impl Config {
    /// The instance-registry name this reducer's data is advertised under.
    /// Must match the names the API layer resolves
    /// (`dogecoin:<network>:<name>:scores`).
    pub fn instance_name(&self) -> &'static str {
        match self {
            Config::BalancesByBrc20(_)
            | Config::Brc20BalancesByScriptHash(_)
            | Config::Brc20TermsByTicker(_)
            | Config::InscriptionsByScriptHash(_)
            | Config::InscriptionsByUtxo(_)
            | Config::TransferInscriptionsByScriptHash(_) => "inscriptions",
            Config::InscriptionUtxosByScriptHash(_) => "inscription-utxos-by-script-hash",
            Config::ContentByInscriptionId(_) => "content-by-inscription-id",
            Config::EtchingByRuneId(_)
            | Config::MintsByRuneId(_)
            | Config::RuneBalancesByScriptHash(_)
            | Config::RuneIdByRuneName(_)
            | Config::RunesByUtxo(_)
            | Config::UtxosByRuneId(_) => "dunes",
            Config::SatBalanceByScriptHash(_) => "sat-balance-by-script-hash",
            Config::ScriptByScriptHash(_) => "scripts",
            Config::ScriptHashByAddressPayloadHash(_) => "scripts",
            Config::TxsByScriptHash(_) => "transactions",
            Config::UtxosByScriptHash(_) => "utxos",
            Config::BalancesByRuneId(_) => "balances-by-rune-id",
        }
    }
}

#[derive(Clone, Debug)]
pub enum ReducerOutput {
    BalancesByBrc20(balances_by_brc20::Output),
    BalancesByRuneId(balances_by_rune_id::Output),
    Brc20BalancesByScriptHash(brc20_balances_by_script_hash::Output),
    Brc20TermsByTicker(brc20_terms_by_ticker::Output),
    EtchingByRuneId(etching_by_rune_id::Output),
    InscriptionUtxosByScriptHash(inscription_utxos_by_script_hash::Output),
    ContentByInscriptionId(content_by_inscription_id::Output),
    InscriptionsByScriptHash(inscriptions_by_script_hash::Output),
    InscriptionsByUtxo(inscriptions_by_utxo::Output),
    MintsByRuneId(mints_by_rune_id::Output),
    RuneBalancesByScriptHash(rune_balances_by_script_hash::Output),
    RuneIdByRuneName(rune_id_by_rune_name::Output),
    RunesByUtxo(runes_by_utxo::Output),
    SatBalanceByScriptHash(sat_balance_by_script_hash::Output),
    ScriptByScriptHash(script_by_script_hash::Output),
    ScriptHashByAddressPayloadHash(script_hash_by_address_payload_hash::Output),
    TransferInscriptionsByScriptHash(transfer_inscriptions_by_script_hash::Output),
    TxsByScriptHash(txs_by_script_hash::Output),
    UtxosByRuneId(utxos_by_rune_id::Output),
    UtxosByScriptHash(utxos_by_script_hash::Output),

    /// Point, was mempool, timestamp
    Cursor(Point, bool, u64),
}

impl Reducer {
    pub fn reduce_block(
        &mut self,
        height: u64,
        block: &Block,
        ctx: &model::BlockContext,
        outputs: &mut Vec<ReducerOutput>,
    ) -> Result<(), gasket::error::Error> {
        match self {
            Reducer::BalancesByBrc20(x) => x.reduce_block(height, block, ctx, outputs),
            Reducer::BalancesByRuneId(x) => x.reduce_block(height, block, ctx, outputs),
            Reducer::Brc20BalancesByScriptHash(x) => x.reduce_block(height, block, ctx, outputs),
            Reducer::Brc20TermsByTicker(x) => x.reduce_block(height, block, ctx, outputs),
            Reducer::EtchingByRuneId(x) => x.reduce_block(height, block, ctx, outputs),
            Reducer::InscriptionUtxosByScriptHash(x) => x.reduce_block(height, block, ctx, outputs),
            Reducer::ContentByInscriptionId(x) => x.reduce_block(height, block, ctx, outputs),
            Reducer::InscriptionsByScriptHash(x) => x.reduce_block(height, block, ctx, outputs),
            Reducer::InscriptionsByUtxo(x) => x.reduce_block(height, block, ctx, outputs),
            Reducer::MintsByRuneId(x) => x.reduce_block(height, block, ctx, outputs),
            Reducer::RuneBalancesByScriptHash(x) => x.reduce_block(height, block, ctx, outputs),
            Reducer::RuneIdByRuneName(x) => x.reduce_block(height, block, ctx, outputs),
            Reducer::RunesByUtxo(x) => x.reduce_block(height, block, ctx, outputs),
            Reducer::SatBalanceByScriptHash(x) => x.reduce_block(block, ctx, outputs),
            Reducer::ScriptByScriptHash(x) => x.reduce_block(height, block, ctx, outputs),
            Reducer::ScriptHashByAddressPayloadHash(x) => {
                x.reduce_block(height, block, ctx, outputs)
            }
            Reducer::TransferInscriptionsByScriptHash(x) => {
                x.reduce_block(height, block, ctx, outputs)
            }
            Reducer::TxsByScriptHash(x) => x.reduce_block(height, block, ctx, outputs),
            Reducer::UtxosByRuneId(x) => x.reduce_block(height, block, ctx, outputs),
            Reducer::UtxosByScriptHash(x) => x.reduce_block(height, block, ctx, outputs),
        }
    }
}

#[derive(Clone, Debug)]
pub enum UtxoAction<T> {
    Consumed,
    Produced(T),
}

#[derive(Clone, Debug)]
pub enum IncrOrDecr<T> {
    Increment(T),
    Decrement(T),
}

pub fn chained_txos(txs: &Vec<Transaction>) -> (Vec<Txid>, Vec<OutPoint>) {
    let mut txids = vec![];
    let mut consumed = vec![];
    let mut produced = vec![];

    for tx in txs {
        consumed.extend(tx.input.iter().map(|x| x.previous_output));

        let txid = tx.compute_txid();

        for i in 0..tx.output.len() {
            produced.push(OutPoint::new(txid, i as u32));
        }

        txids.push(txid);
    }

    let chained_txos = produced
        .into_iter()
        .filter(|x| consumed.contains(&x))
        .collect();

    (txids, chained_txos)
}

use compressor_xdg::storage::ChainDB;
use miette::{Context, IntoDiagnostic};
use tracing::info;

#[derive(Debug, clap::Args)]
pub struct Args {}

pub async fn run(config: &super::Config, _args: &Args) -> miette::Result<()> {
    super::common::setup_tracing(&config.logging)?;
    super::common::setup_os_signal_hooks()?;

    info!(
        "running daemon (network {:?}, node {}, rolldb {})",
        config.sync.network,
        config.sync.node_address,
        config.chain_db.path.display()
    );

    let chain_db = ChainDB::open(
        &config.chain_db.path,
        config.chain_db.immutable_after_confs,
        config.sync.network,
        config.sync.first_rune_height.unwrap_or_default(),
        config.sync.first_inscription_height.unwrap_or_default(),
        config.sync.jubilee_height,
        config.sync.utxos_in_memory.unwrap_or(false),
    )
    .into_diagnostic()
    .context("opening chaindb")?;

    let _sync = compressor_xdg::sync::pipeline(&config.sync, chain_db.clone(), &None)
        .into_diagnostic()
        .context("initialising sync pipeline")?;

    compressor_xdg::serve::serve(&config.serve, chain_db.clone())
        .await
        .into_diagnostic()
        .context("initialising serve")?;

    info!("compressor is stopping...");

    Ok(())
}

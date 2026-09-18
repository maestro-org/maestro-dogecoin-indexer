use std::fs;

use bb8::Pool;
use bb8_tikv::TiKVTransactionalConnectionManager;
use options::Mode;
use tokio::net::TcpListener;
use tracing::info;
use tracing_subscriber::fmt;
use utoipa::OpenApi;

use crate::{api::APIDoc, error::Error, options::Options, polyphony::PolyphonyWrapper};

mod api;
mod client;
mod error;
pub mod key_resolver;
mod options;
pub mod polyphony;
pub mod types;
pub mod util;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let format = fmt::format()
        .with_level(true)
        .with_target(false)
        .with_thread_ids(false)
        .with_thread_names(false)
        .with_ansi(false) // Remove colors and styling
        .without_time(); // Remove timestamps

    fmt().event_format(format).init();

    let options = Options::parse();

    if options.mode == Mode::GenerateOpenApi {
        info!("Generating Open API specification...");
        fs::write(
            "docs/indexer/swagger.json",
            APIDoc::openapi().to_pretty_json()?,
        )?;
        info!("[OK] Done. See docs/indexer/swagger.json for the result.");
        return Ok(()); // Exit the program after writing the docs
    }

    let tikv_conn_manager =
        TiKVTransactionalConnectionManager::new(vec![options.tikv_address.clone()], None).unwrap();

    let tikv_pool = Pool::builder()
        .max_size(5)
        .build(tikv_conn_manager)
        .await
        .unwrap();

    let result = match options.mode {
        Mode::GenerateOpenApi => {
            panic!("Should not happen. Application in GenerateOpenApi Mode should have been terminated before.");
        }
        Mode::Dogecoin => {
            key_resolver::initialize_config(key_resolver::Network::Mainnet, options.redis.clone())
        }
        Mode::DogecoinTestnet => {
            key_resolver::initialize_config(key_resolver::Network::Testnet, options.redis.clone())
        }
    };

    if let Err(e) = result {
        tracing::error!("Critical error initializing configuration: {}", e);
        panic!(
            "Failed to initialize the application due to configuration error: {}",
            e
        );
    }

    let polyphony = PolyphonyWrapper::new(tikv_pool);

    let app = api::router(&options, polyphony, options.mode).await?;

    info!(address = %options.listen_address, "Starting HTTP Server");
    let listener = TcpListener::bind(options.listen_address).await?;

    axum::serve(listener, app).await?;

    Ok(())
}

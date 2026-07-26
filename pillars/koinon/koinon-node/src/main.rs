use std::sync::{Arc, Mutex};

use anyhow::Result;
use clap::Parser;

use koinon_node::api;
use koinon_node::block::BlockProducer;
use koinon_node::config::NodeConfig;
use koinon_node::store::StateStore;

#[derive(Parser)]
#[command(name = "koinon-node")]
#[command(about = "TPT Oikos Koinon node — block production and RPC API")]
struct Cli {
    /// Path to config file (TOML). Uses defaults if not provided.
    #[arg(short, long)]
    config: Option<String>,

    /// Override RPC port
    #[arg(long)]
    rpc_port: Option<u16>,

    /// Override data directory
    #[arg(long)]
    data_dir: Option<String>,
}

fn main() -> Result<()> {
    env_logger::init();

    let cli = Cli::parse();

    let mut config = match &cli.config {
        Some(path) => NodeConfig::load(path)?,
        None => NodeConfig::default(),
    };

    if let Some(port) = cli.rpc_port {
        config.rpc_port = port;
    }
    if let Some(dir) = cli.data_dir {
        config.data_dir = dir;
    }

    log::info!("koinon-node starting with data_dir={}", config.data_dir);
    log::info!("chain_id={}", config.genesis.chain_id);
    log::info!("block_time_ms={}", config.block_time_ms);

    let store = StateStore::new(&config.database_path())?;
    let producer = BlockProducer::new(config.clone(), store)?;
    let producer = Arc::new(Mutex::new(producer));

    // Start RPC server in background thread
    let rpc_producer = Arc::clone(&producer);
    let rpc_port = config.rpc_port;
    std::thread::spawn(move || {
        if let Err(e) = api::start_rpc_server(rpc_port, rpc_producer) {
            log::error!("RPC server failed: {e}");
        }
    });

    log::info!("Node initialized. Starting block production loop.");

    // Block production loop
    loop {
        let block_time = std::time::Duration::from_millis(config.block_time_ms);
        std::thread::sleep(block_time);

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut prod = producer.lock().unwrap();
        match prod.produce_block(now) {
            Ok(block) => {
                log::info!(
                    "Block #{} produced | hash={} | txs={}",
                    block.height,
                    format!("{:02x}{:02x}{:02x}{:02x}...", block.hash[0], block.hash[1], block.hash[2], block.hash[3]),
                    block.transactions.len(),
                );
            }
            Err(e) => {
                log::error!("Failed to produce block: {e}");
            }
        }
    }
}

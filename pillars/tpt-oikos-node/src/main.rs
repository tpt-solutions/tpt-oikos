//! TPT Oikos node binary.
//!
//! The unified entry point that boots all settlement-layer subsystems: the
//! parallel-settlement DAG, dual-token ledger, validator staking pool,
//! disinflationary block rewards, on-chain treasury, fee splitting, gas
//! metering, and Telos contract verification.
//!
//! # Quick start
//!
//! ```text
//! cargo run -p tpt-oikos-node -- start      # boot the node
//! cargo run -p tpt-oikos-node -- status     # inspect current state
//! cargo run -p tpt-oikos-node -- tokenomics # emission schedule
//! ```

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use koinon_dag::{Dag, ParallelSettler, Transaction, TxHash, TxKind};
use koinon_fee::FeeSplit;
use koinon_gas::calculate_gas;
use koinon_ledger::{
    emission_at_year, KoinAmount, OikosAmount, TotalValueConservation,
};
use koinon_rewards::{BlockReward, BlockRewardConfig, BlockRewardProcessor};
use koinon_staking::StakingPool;
use koinon_treasury::TreasuryPool;

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

/// Top-level CLI parsed by [`clap`].
///
/// Every subcommand maps to a dedicated handler function (`cmd_start`,
/// `cmd_status`, etc.). The binary entry point delegates to those handlers
/// after initialising `env_logger`.
#[derive(Parser)]
#[command(name = "oikos", version, about = "TPT Oikos node — boots the settlement layer")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

/// Available CLI subcommands.
#[derive(Subcommand)]
enum Commands {
    /// Start the node (initializes all subsystems and simulates block processing)
    Start,
    /// Show node status (block height, validators, staked amount, DAG stats)
    Status,
    /// Show OIKOS emission schedule and fee info
    Tokenomics,
    /// Verify a .telos contract file
    Verify {
        /// Path to the .telos file to verify
        file: PathBuf,
    },
    /// Estimate gas for a transaction
    Gas {
        /// Number of compute steps
        #[arg(long, default_value = "1")]
        steps: u64,
        /// Number of storage bytes
        #[arg(long, default_value = "0")]
        storage: u64,
    },
    /// Show version information
    Version,
}

// ---------------------------------------------------------------------------
// NodeState
// ---------------------------------------------------------------------------

/// Complete in-memory state of a running Oikos node.
///
/// Holds every subsystem required to process blocks, settle transactions,
/// enforce conservation, distribute validator rewards, and track treasury
/// funds. Created once at boot and mutated in place as blocks are processed.
///
/// # Invariants
///
/// - `block_number` starts at 0 and increments by exactly 1 per
///   [`process_block`](NodeState::process_block) call.
/// - The staking pool, conservation tracker, and treasury each maintain their
///   own internal invariants which are checked via `check_invariant()`.
pub struct NodeState {
    /// The parallel-settlement DAG storing all known transactions.
    pub dag: Dag,
    /// Dual-token (OIKOS + KOIN) conservation invariant tracker.
    pub conservation: TotalValueConservation,
    /// Validator staking pool — tracks registrations, stakes, and rewards.
    pub staking_pool: StakingPool,
    /// On-chain governance treasury.
    pub treasury: TreasuryPool,
    /// Current block height (starts at 0, incremented per block).
    pub block_number: u64,
    /// Processes block rewards according to the disinflationary emission schedule.
    pub block_reward_processor: BlockRewardProcessor,
}

impl NodeState {
    /// Create a new `NodeState` with all subsystems at their default (empty) state.
    ///
    /// # Examples
    ///
    /// ```text
    /// let state = NodeState::new();
    /// assert_eq!(state.block_number, 0);
    /// assert!(state.dag.is_empty());
    /// ```
    pub fn new() -> Self {
        log::info!("Initializing node state");
        let state = Self {
            dag: Dag::new(),
            conservation: TotalValueConservation::new(),
            staking_pool: StakingPool::new(),
            treasury: TreasuryPool::new(OikosAmount::ZERO),
            block_number: 0,
            block_reward_processor: BlockRewardProcessor::new(BlockRewardConfig::default()),
        };
        log::debug!("Node state created with default values");
        state
    }

    /// Process a single block, advancing the chain by one.
    ///
    /// Increments `block_number`, calculates the block reward via the
    /// disinflationary emission schedule, splits the supplied fees using the
    /// default 70/20/10 ratio, and checks the conservation invariant.
    ///
    /// # Arguments
    ///
    /// * `block_fees` — total KOIN fees collected during this block.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying reward processor rejects the block
    /// (e.g. duplicate block number).
    pub fn process_block(&mut self, block_fees: KoinAmount) -> Result<BlockReward, anyhow::Error> {
        self.block_number += 1;
        log::info!("Processing block {}", self.block_number);

        let reward = self
            .block_reward_processor
            .process_block(self.block_number, block_fees)
            .map_err(|e| anyhow::anyhow!("Reward processing failed: {e}"))?;

        let split = FeeSplit::from_total(block_fees);
        log::debug!(
            "Block {} reward: base={}, fees(burn={}, validator={}, treasury={})",
            self.block_number,
            reward.base_reward.0,
            split.burn.0,
            split.validator.0,
            split.treasury.0,
        );

        if !self.block_reward_processor.check_conservation() {
            log::warn!(
                "Conservation invariant violated at block {}",
                self.block_number
            );
        }

        Ok(reward)
    }

    /// Insert a transaction into the DAG.
    ///
    /// The transaction is added as a tip until [`settle_pending`](NodeState::settle_pending)
    /// resolves it against its parent transactions.
    ///
    /// # Errors
    ///
    /// Returns an error if the DAG rejects the transaction (e.g. invalid parents,
    /// duplicate hash).
    pub fn insert_transaction(&mut self, tx: Transaction) -> anyhow::Result<()> {
        let hash = tx.hash;
        self.dag.insert(tx).map_err(|e| anyhow::anyhow!("DAG insert failed: {:?}", e))?;
        log::debug!("Transaction {:02x?} inserted into DAG", hash);
        Ok(())
    }

    /// Settle all pending (tip) transactions in the DAG.
    ///
    /// Collects every current tip, then invokes the [`ParallelSettler`] to
    /// resolve dependencies and move transactions to their final status.
    /// Logs a summary of settled, failed, and conflicted transactions.
    pub fn settle_pending(&mut self) {
        let pending: Vec<TxHash> = self.dag.tips().copied().collect();
        if pending.is_empty() {
            return;
        }
        let result = ParallelSettler::settle_batch(&mut self.dag, &pending);
        log::info!(
            "Settlement: {} settled, {} failed, {} conflicted",
            result.settled.len(),
            result.failed.len(),
            result.conflicted.len(),
        );
    }

    /// Register a new validator in the staking pool.
    ///
    /// # Arguments
    ///
    /// * `did` — decentralized identifier for the validator (e.g. `"did:example:validator-alpha"`).
    ///
    /// # Returns
    ///
    /// The auto-incremented numeric validator ID.
    ///
    /// # Errors
    ///
    /// Returns [`StakingError`](koinon_staking::StakingError) if the DID is
    /// already registered.
    pub fn register_validator(&mut self, did: &str) -> Result<u64, koinon_staking::StakingError> {
        let id = self.staking_pool.register_validator(did)?;
        log::info!("Registered validator #{id} ({did})");
        Ok(id)
    }

    /// Stake OIKOS tokens for a registered validator.
    ///
    /// # Arguments
    ///
    /// * `id` — validator ID returned by [`register_validator`](NodeState::register_validator).
    /// * `amount` — OIKOS amount to stake.
    ///
    /// # Errors
    ///
    /// Returns [`StakingError`](koinon_staking::StakingError) if the validator
    /// does not exist or the staking invariant is violated.
    pub fn stake_validator(
        &mut self,
        id: u64,
        amount: OikosAmount,
    ) -> Result<(), koinon_staking::StakingError> {
        self.staking_pool.stake(id, amount)?;
        log::info!("Validator #{id} staked {}", amount);
        Ok(())
    }
}

impl Default for NodeState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a synthetic [`Transaction`] for testing and simulation.
///
/// The hash is deterministically derived from `nonce`, `sender`, and `recipient`
/// so that parent references remain stable within the simulation loop.
fn make_tx(nonce: u64, sender: u64, recipient: u64, parents: Vec<TxHash>) -> Transaction {
    let mut hash_bytes = [0u8; 32];
    hash_bytes[0] = (nonce + sender * 1000) as u8;
    hash_bytes[1] = (recipient) as u8;
    hash_bytes[2] = (nonce >> 8) as u8;
    Transaction {
        hash: hash_bytes,
        kind: TxKind::TransferKoin,
        sender,
        recipient,
        oikos_amount: OikosAmount::ZERO,
        koin_amount: KoinAmount(100),
        gas_limit: 21000,
        nonce,
        parent_hashes: parents,
        timestamp: nonce,
    }
}

/// Format a raw OIKOS amount (in base units) as a human-readable string
/// with 18 decimal places (e.g. `"1.234000000000000000"`).
fn format_oikos(amount: u128) -> String {
    let tokens = amount / 10_u128.pow(18);
    let frac = amount % 10_u128.pow(18);
    format!("{}.{:018}", tokens, frac)
}

/// Format an emission amount for display in the tokenomics table.
///
/// Values >= 1 billion are shown as `"X.XXXB"`, smaller values as `"X.XXXM"`.
fn format_emission(amount: u128) -> String {
    let billions = amount / 1_000_000_000;
    let millions = (amount % 1_000_000_000) / 1_000_000;
    if billions > 0 {
        format!("{}.{:03}B", billions, millions)
    } else {
        format!("{}.{:03}M", millions, (amount % 1_000_000) / 1_000)
    }
}

// ---------------------------------------------------------------------------
// Subcommand handlers
// ---------------------------------------------------------------------------

/// Handle the `start` subcommand.
///
/// Boots every subsystem, registers three genesis validators with OIKOS stakes,
/// inserts a genesis transaction, simulates 10 blocks with proportional fees,
/// distributes accumulated rewards, and prints a final status report.
fn cmd_start() -> anyhow::Result<()> {
    log::info!("=== TPT Oikos Node Starting ===");
    log::info!("Initializing subsystems...");

    let mut state = NodeState::new();

    // Register some validators
    log::info!("Registering genesis validators");
    let v1 = state.register_validator("did:example:validator-alpha")?;
    let v2 = state.register_validator("did:example:validator-beta")?;
    let v3 = state.register_validator("did:example:validator-gamma")?;

    let stake_amount = OikosAmount(200_000 * 10_u128.pow(18));
    state.stake_validator(v1, stake_amount)?;
    state.stake_validator(v2, stake_amount)?;
    state.stake_validator(v3, OikosAmount(150_000 * 10_u128.pow(18)))?;
    log::info!(
        "Total staked: {} OIKOS across {} validators",
        state.staking_pool.total_staked(),
        state.staking_pool.active_validators().len(),
    );

    // Genesis transaction (no parents)
    let genesis_tx = make_tx(0, 1, 2, vec![]);
    let genesis_hash = genesis_tx.hash;
    state.insert_transaction(genesis_tx)?;
    state.settle_pending();

    // Simulate 10 blocks with random transactions
    log::info!("Simulating 10 blocks...");
    let mut parent_hash = genesis_hash;
    for block in 1..=10u64 {
        let fees = KoinAmount((block * 500) as i128);
        let reward = state.process_block(fees)?;

        log::info!(
            "Block {}: year={}, base_reward={}, fees_burn={}, fees_validator={}, fees_treasury={}",
            block,
            reward.year,
            format_oikos(reward.base_reward.0),
            reward.fee_burn.0,
            reward.fee_validator.0,
            reward.fee_treasury.0,
        );

        // Add a transaction referencing the previous tip
        let tx = make_tx(block, (block % 5) + 1, (block % 3) + 10, vec![parent_hash]);
        let tx_hash = tx.hash;
        state.insert_transaction(tx)?;
        state.settle_pending();
        parent_hash = tx_hash;
    }

    // Distribute block rewards to validators
    let total_reward = OikosAmount(10 * 1_582_278); // 10 blocks worth of base rewards
    state.staking_pool.distribute_rewards(total_reward);
    log::info!(
        "Distributed {} OIKOS in validator rewards",
        total_reward
    );

    log::info!("=== Node started successfully ===");
    cmd_status_inner(&state);
    Ok(())
}

/// Handle the `status` subcommand.
///
/// Creates a fresh [`NodeState`] (empty DAG, no validators) and prints a
/// snapshot of its fields. This is useful for verifying the binary compiles
/// and boots without side-effects.
fn cmd_status() -> anyhow::Result<()> {
    let state = NodeState::new();
    cmd_status_inner(&state);
    Ok(())
}

/// Print a human-readable status summary for the given node state.
///
/// Output includes block height, active validator count, total staked OIKOS,
/// DAG transaction count, tip count, treasury balance, conservation status,
/// current emission year, and total OIKOS minted.
fn cmd_status_inner(state: &NodeState) {
    println!("=== TPT Oikos Node Status ===");
    println!("Block height:      {}", state.block_number);
    println!(
        "Active validators: {}",
        state.staking_pool.active_validators().len()
    );
    println!(
        "Total staked:      {} OIKOS",
        format_oikos(state.staking_pool.total_staked().0)
    );
    println!("DAG transactions:  {}", state.dag.len());
    println!("DAG tips:          {}", state.dag.tips().count());
    println!(
        "Treasury balance:  {} OIKOS",
        format_oikos(state.treasury.balance.0)
    );
    println!(
        "Conservation:      {}",
        if state.conservation.check_invariant() {
            "OK"
        } else {
            "VIOLATED"
        }
    );
    println!(
        "Current year:      {}",
        state.block_reward_processor.current_year()
    );
    println!(
        "Supply minted:     {} OIKOS",
        format_oikos(state.block_reward_processor.conservation().minted.0)
    );
}

/// Handle the `tokenomics` subcommand.
///
/// Prints the full 20-year OIKOS emission schedule (80% annual decay,
/// halts at year 20), max supply, fee split breakdown, and gas formula.
fn cmd_tokenomics() {
    println!("=== OIKOS Tokenomics ===");
    println!();
    println!("Emission Schedule (disinflationary, 80% decay per year, halts at year 20):");
    println!("{:<6} {:>15} {:>15}", "Year", "Annual", "Cumulative");
    println!("{}", "-".repeat(38));
    let mut cumulative = 0u128;
    for year in 1..=20 {
        let emission = emission_at_year(year);
        cumulative += emission;
        println!(
            "{:<6} {:>15} {:>15}",
            year,
            format_emission(emission),
            format_emission(cumulative)
        );
    }
    println!();
    println!("Max supply: 1,000,000,000 OIKOS");
    println!();
    println!("Fee Split (default 70/20/10):");
    println!("  Burn:        70% — deflationary pressure");
    println!("  Validator:   20% — block production incentive");
    println!("  Treasury:    10% — protocol development fund");
    println!();
    println!("Gas Formula: base(1000) + steps * 10 + storage_bytes * 100");
}

/// Handle the `verify` subcommand.
///
/// Reads the given `.telos` file, parses it into modules, extracts the
/// intermediate representation, and runs the formal verifier on each
/// constraint problem. Prints PASS/FAIL per function and exits with code 1
/// on any failure.
fn cmd_verify(file: &PathBuf) -> anyhow::Result<()> {
    let source = std::fs::read_to_string(file)
        .map_err(|e| anyhow::anyhow!("Failed to read {}: {}", file.display(), e))?;

    let modules = tpt_telos_parser::parse(&source)
        .map_err(|e| anyhow::anyhow!("Parse error in {}: {}", file.display(), e))?;

    let problems = tpt_telos_ir::extract(&modules)
        .map_err(|e| anyhow::anyhow!("IR extraction error in {}: {}", file.display(), e))?;

    let mut all_passed = true;
    println!("Verifying {}", file.display());
    for problem in &problems {
        let result = tpt_telos_verifier::verify(problem);
        if !result.all_passed {
            all_passed = false;
        }
        let status = if result.all_passed { "PASS" } else { "FAIL" };
        println!("  [{status}] {}", result.func_name);
        for check in &result.checks {
            let cp = if check.passed { "PASS" } else { "FAIL" };
            println!("    [{cp}] {}", check.description);
        }
    }
    println!();
    if all_passed {
        println!("RESULT: all constraints satisfied");
    } else {
        println!("RESULT: verification FAILED");
        std::process::exit(1);
    }
    Ok(())
}

/// Handle the `gas` subcommand.
///
/// Calculates and prints the gas cost breakdown: base (1000) + compute
/// (steps × 10) + storage (bytes × 100).
fn cmd_gas(steps: u64, storage: u64) {
    let cost = calculate_gas(steps, storage);
    println!("Gas Estimate:");
    println!("  Total:     {cost}");
    println!("  Base:      1000");
    println!("  Compute:   {steps} steps x 10 = {}", steps * 10);
    println!("  Storage:   {storage} bytes x 100 = {}", storage * 100);
}

/// Handle the `version` subcommand.
///
/// Prints the crate version, Rust edition, license, and an inventory of every
/// integrated subsystem.
fn cmd_version() {
    println!("TPT Oikos Node v{}", env!("CARGO_PKG_VERSION"));
    println!("Rust edition: {}", env!("CARGO_PKG_RUST_VERSION"));
    println!("License: {}", env!("CARGO_PKG_LICENSE"));
    println!();
    println!("Subsystems:");
    println!("  koinon-ledger    — dual-token accounting (OIKOS + KOIN)");
    println!("  koinon-dag       — parallel-settlement DAG");
    println!("  koinon-staking   — validator staking pool");
    println!("  koinon-rewards   — disinflationary block rewards");
    println!("  koinon-treasury  — on-chain governance treasury");
    println!("  koinon-fee       — 70/20/10 fee split");
    println!("  koinon-gas       — deterministic gas metering");
    println!("  koinon-mandates  — AI agent mandate enforcement");
    println!("  koinon-escrow    — escrow & streaming payments");
    println!("  telos            — contract verification (Telos language)");
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

/// Binary entry point.
///
/// Initialises `env_logger` with millisecond timestamps and a default log
/// level of `info` (override via `RUST_LOG`), parses the CLI arguments, and
/// dispatches to the appropriate subcommand handler.
fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Start => cmd_start()?,
        Commands::Status => cmd_status()?,
        Commands::Tokenomics => cmd_tokenomics(),
        Commands::Verify { file } => cmd_verify(&file)?,
        Commands::Gas { steps, storage } => cmd_gas(steps, storage),
        Commands::Version => cmd_version(),
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_state_initializes_with_defaults() {
        let state = NodeState::new();
        assert_eq!(state.block_number, 0);
        assert!(state.dag.is_empty());
        assert!(state.staking_pool.check_invariant());
        assert!(state.conservation.check_invariant());
        assert!(state.treasury.check_invariant());
    }

    #[test]
    fn block_processing_produces_rewards_and_fee_split() {
        let mut state = NodeState::new();
        let fees = KoinAmount(10000);
        let reward = state.process_block(fees).unwrap();

        assert_eq!(reward.block_number, 1);
        assert_eq!(reward.year, 1);
        assert!(reward.base_reward.0 > 0);

        let total_fees = reward.fee_burn.0 + reward.fee_validator.0 + reward.fee_treasury.0;
        assert_eq!(total_fees, 10000);
    }

    #[test]
    fn multiple_blocks_increment_height() {
        let mut state = NodeState::new();
        for i in 1..=5 {
            let reward = state.process_block(KoinAmount(500)).unwrap();
            assert_eq!(reward.block_number, i);
        }
        assert_eq!(state.block_number, 5);
    }

    #[test]
    fn status_output_includes_all_fields() {
        let state = NodeState::new();
        let status = format!(
            "height={} validators={} staked={} dag_len={} tips={} treasury={}",
            state.block_number,
            state.staking_pool.active_validators().len(),
            format_oikos(state.staking_pool.total_staked().0),
            state.dag.len(),
            state.dag.tips().count(),
            format_oikos(state.treasury.balance.0),
        );
        assert!(status.contains("height=0"));
        assert!(status.contains("validators=0"));
        assert!(status.contains("dag_len=0"));
        assert!(status.contains("tips=0"));
    }

    #[test]
    fn transaction_insertion_into_dag() {
        let mut state = NodeState::new();
        let tx = make_tx(0, 1, 2, vec![]);
        let hash = tx.hash;
        state.insert_transaction(tx).unwrap();
        assert_eq!(state.dag.len(), 1);
        assert!(state.dag.get(&hash).is_some());
    }

    #[test]
    fn genesis_transaction_settles_without_parents() {
        let mut state = NodeState::new();
        let tx = make_tx(0, 1, 2, vec![]);
        let hash = tx.hash;
        state.insert_transaction(tx).unwrap();
        state.settle_pending();
        let node = state.dag.get(&hash).unwrap();
        assert_eq!(node.status, koinon_dag::TxStatus::Settled);
    }

    #[test]
    fn conservation_holds_after_processing() {
        let mut state = NodeState::new();
        for _ in 1..=20 {
            state.process_block(KoinAmount(1000)).unwrap();
        }
        assert!(state.block_reward_processor.check_conservation());
        assert!(state.conservation.check_invariant());
    }
}

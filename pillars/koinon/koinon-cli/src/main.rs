use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "koinon", version, about = "tpt-koinon Settlement & AI Economy Layer")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Show ledger balances for an account
    Balance {
        /// Account ID
        #[arg(short, long)]
        account: u64,
    },
    /// Submit a transaction to the DAG
    Send {
        #[arg(short, long)]
        from: u64,
        #[arg(short, long)]
        to: u64,
        #[arg(short, long)]
        amount: String,
        #[arg(short, long, default_value = "koin")]
        token: String,
    },
    /// Show DAG status
    Dag {
        #[command(subcommand)]
        action: DagCommands,
    },
    /// Estimate gas for a transaction
    Gas {
        #[arg(short, long, default_value = "1")]
        steps: u64,
        #[arg(short, long, default_value = "0")]
        storage: u64,
    },
    /// Verify a .telos contract file before deployment
    Verify {
        /// Path to the .telos file to verify
        file: PathBuf,
        /// Output machine-readable JSON instead of human text
        #[arg(long)]
        json: bool,
    },
    /// Show tokenomics summary
    Tokenomics {
        #[command(subcommand)]
        action: TokenomicsCommands,
    },
}

#[derive(Subcommand)]
enum DagCommands {
    /// Show DAG statistics
    Stats,
    /// List pending transactions
    Pending,
}

#[derive(Subcommand)]
enum TokenomicsCommands {
    /// Show the OIKOS emission schedule
    Emission,
    /// Show fee split for a given fee amount
    FeeSplit {
        /// Total fee amount in Koin base units
        #[arg(short, long)]
        amount: i128,
    },
    /// Show gas cost estimate
    GasCost {
        #[arg(short, long, default_value = "1")]
        steps: u64,
        #[arg(short, long, default_value = "0")]
        storage: u64,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Balance { account } => {
            println!("Balance for account {account}: (not yet connected to state)");
        }
        Commands::Send { from, to, amount, token } => {
            println!("Sending {amount} {token} from {from} to {to} (not yet wired)");
        }
        Commands::Dag { action } => match action {
            DagCommands::Stats => {
                println!("DAG stats: (not yet connected to state)");
            }
            DagCommands::Pending => {
                println!("Pending transactions: (not yet connected to state)");
            }
        },
        Commands::Gas { steps, storage } => {
            let cost = koinon_gas::calculate_gas(steps, storage);
            println!("Estimated gas: {cost}");
        }
        Commands::Verify { file, json } => {
            cmd_verify(&file, json)?;
        }
        Commands::Tokenomics { action } => match action {
            TokenomicsCommands::Emission => {
                cmd_emission_schedule();
            }
            TokenomicsCommands::FeeSplit { amount } => {
                cmd_fee_split(amount);
            }
            TokenomicsCommands::GasCost { steps, storage } => {
                let cost = koinon_gas::calculate_gas(steps, storage);
                println!("Gas cost: {cost}");
                println!("  Base: 1000");
                println!("  Compute: {steps} steps x 10 = {}", steps * 10);
                println!("  Storage: {storage} bytes x 100 = {}", storage * 100);
            }
        },
    }

    Ok(())
}

/// Verify a .telos contract file by invoking the tpt-telos verifier.
///
/// This is the contract deployment gate: any contract deployed to koinon
/// must first pass telos verification to ensure invariants hold.
fn cmd_verify(file: &PathBuf, json: bool) -> anyhow::Result<()> {
    let source = std::fs::read_to_string(file)
        .map_err(|e| anyhow::anyhow!("Failed to read {}: {}", file.display(), e))?;

    let modules = tpt_telos_parser::parse(&source)
        .map_err(|e| anyhow::anyhow!("Parse error in {}: {}", file.display(), e))?;

    let problems = tpt_telos_ir::extract(&modules)
        .map_err(|e| anyhow::anyhow!("IR extraction error in {}: {}", file.display(), e))?;

    let mut all_passed = true;
    let mut results: Vec<serde_json::Value> = Vec::new();

    for problem in &problems {
        let result = tpt_telos_verifier::verify(problem);
        if !result.all_passed {
            all_passed = false;
        }
        results.push(serde_json::json!({
            "func_name": result.func_name,
            "all_passed": result.all_passed,
            "checks": result.checks.iter().map(|c| {
                serde_json::json!({
                    "description": c.description,
                    "passed": c.passed,
                })
            }).collect::<Vec<_>>(),
        }));
    }

    if json {
        let output = serde_json::json!({
            "file": file.display().to_string(),
            "passed": all_passed,
            "functions": results,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!("Verifying {}", file.display());
        for r in &results {
            let func_name = r["func_name"].as_str().unwrap_or("?");
            let passed = r["all_passed"].as_bool().unwrap_or(false);
            let status = if passed { "PASS" } else { "FAIL" };
            println!("  [{status}] {func_name}");
            for check in r["checks"].as_array().unwrap_or(&vec![]) {
                let desc = check["description"].as_str().unwrap_or("?");
                let cp = if check["passed"].as_bool().unwrap_or(false) { "PASS" } else { "FAIL" };
                println!("    [{cp}] {desc}");
            }
        }
        println!();
        if all_passed {
            println!("RESULT: all constraints satisfied");
        } else {
            println!("RESULT: verification FAILED");
        }
    }

    if !all_passed {
        std::process::exit(1);
    }

    Ok(())
}

/// Print the OIKOS emission schedule.
fn cmd_emission_schedule() {
    use koinon_ledger::emission_at_year;
    println!("OIKOS Emission Schedule (disinflationary, halts at year 20)");
    println!("{:<6} {:>15} {:>15}", "Year", "Annual", "Cumulative");
    println!("{}", "-".repeat(38));
    let mut cumulative = 0u128;
    for year in 1..=20 {
        let emission = emission_at_year(year);
        cumulative += emission;
        println!("{:<6} {:>15} {:>15}", year, format_emission(emission), format_emission(cumulative));
    }
}

fn format_emission(amount: u128) -> String {
    let billions = amount / 1_000_000_000;
    let millions = (amount % 1_000_000_000) / 1_000_000;
    if billions > 0 {
        format!("{}.{:03}B", billions, millions)
    } else {
        format!("{}.{:03}M", millions, (amount % 1_000_000) / 1_000)
    }
}

/// Show fee split for a given amount.
fn cmd_fee_split(amount: i128) {
    use koinon_fee::FeeSplit;
    use koinon_ledger::KoinAmount;
    let split = FeeSplit::from_total(KoinAmount(amount));
    println!("Fee split for {amount} Koin:");
    println!("  Burn (70%):      {}", split.burn.0);
    println!("  Validator (20%): {}", split.validator.0);
    println!("  Treasury (10%):  {}", split.treasury.0);
    println!("  Total:           {}", split.total().0);
    println!("  Conservation:    {}", if split.check_conservation(KoinAmount(amount)) { "OK" } else { "VIOLATED" });
}

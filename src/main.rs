use anyhow::Result;
use clap::{Parser, Subcommand};

mod builder;
mod codec;
mod extract;
mod model;
mod record;

#[derive(Parser)]
#[command(name = "aeat720_generator", about = "Convert CSV ↔ AEAT Modelo 720, extract positions from broker PDFs")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Input file (CSV or .720 with --reverse)
    #[arg(global = false)]
    input: Option<String>,

    /// Output file
    #[arg(global = false)]
    output: Option<String>,

    /// Reverse: convert .720 back to CSV
    #[arg(long, short)]
    reverse: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Extract positions from a broker PDF statement into CSV
    Extract {
        /// Path to the broker PDF statement
        pdf: String,
        /// Spanish tax ID
        #[arg(long)]
        nif: String,
        /// Full name, surname first
        #[arg(long)]
        name: String,
        /// Contact phone number
        #[arg(long)]
        phone: String,
        /// Tax year
        #[arg(long)]
        year: usize,
        /// Ownership percentage
        #[arg(long)]
        percentage: f64,
        /// Contact person name (defaults to --name)
        #[arg(long)]
        contact_name: Option<String>,
        /// EUR/USD exchange rate at year end for USD→EUR conversion
        #[arg(long)]
        eur_usd_rate: Option<f64>,
        /// CSV file to append to (prints to stdout if omitted)
        #[arg(short, long)]
        output: Option<String>,
        /// Print rows without writing to file
        #[arg(long)]
        dry_run: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Extract { pdf, nif, name, phone, year, percentage, contact_name, eur_usd_rate, output, dry_run }) => {
            let ea = extract::ExtractArgs {
                nif, name: name.clone(), phone, year, percentage,
                contact_name: contact_name.unwrap_or(name),
                eur_usd_rate,
            };
            extract::extract(&pdf, &ea, output.as_deref(), dry_run)
        }
        None => {
            if cli.reverse {
                let input = cli.input.as_deref().ok_or_else(|| anyhow::anyhow!("Missing input .720 file"))?;
                codec::reverse(input, cli.output.as_deref())
            } else if let Some(input) = cli.input.as_deref() {
                codec::generate(input, cli.output.as_deref())
            } else {
                Cli::parse_from(["", "--help"]);
                Ok(())
            }
        }
    }
}

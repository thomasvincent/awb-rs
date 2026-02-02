use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use url::Url;

mod commands;

#[derive(Parser)]
#[command(name = "awb-rs")]
#[command(version, about = "AutoWikiBrowser in Rust - Wikipedia bot framework", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Login to a MediaWiki instance
    Login {
        /// Wiki API URL (e.g., https://en.wikipedia.org/w/api.php)
        #[arg(long)]
        wiki: Url,

        /// Bot username
        #[arg(long)]
        username: String,

        /// Profile ID to save credentials under
        #[arg(long, default_value = "default")]
        profile: String,
    },

    /// List pages from various sources
    List {
        /// Wiki API URL
        #[arg(long)]
        wiki: Url,

        /// Source type
        #[arg(long)]
        source: ListSource,

        /// Query value (category name, page title, search query, or file path)
        #[arg(long)]
        query: String,

        /// Maximum number of pages to fetch (0 = unlimited)
        #[arg(long, default_value = "100")]
        limit: usize,
    },

    /// Run editing workflow with a profile
    Run {
        /// Wiki API URL
        #[arg(long)]
        wiki: Url,

        /// Profile file path (TOML)
        #[arg(long)]
        profile: PathBuf,

        /// Batch mode (auto-save all changes)
        #[arg(long)]
        batch: bool,

        /// Dry-run mode (show diffs without saving)
        #[arg(long)]
        dry_run: bool,

        /// Profile ID for credentials
        #[arg(long, default_value = "default")]
        auth_profile: String,
    },

    /// Export telemetry log
    ExportLog {
        /// Output format
        #[arg(long)]
        format: ExportFormat,

        /// Output file path
        #[arg(long)]
        output: PathBuf,
    },
}

#[derive(Clone, Debug, clap::ValueEnum)]
enum ListSource {
    Category,
    WhatLinksHere,
    Search,
    File,
}

#[derive(Clone, Debug, clap::ValueEnum)]
enum ExportFormat {
    Json,
    Csv,
    Plain,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize telemetry
    awb_telemetry::init_telemetry(&awb_telemetry::TelemetryConfig {
        log_dir: "logs".into(),
        level: tracing::Level::INFO,
        json_output: true,
        human_output: true,
    })?;

    let cli = Cli::parse();

    match cli.command {
        Commands::Login { wiki, username, profile } => {
            commands::login::run(wiki, username, profile).await
        }
        Commands::List { wiki, source, query, limit } => {
            commands::list::run(wiki, source, query, limit).await
        }
        Commands::Run { wiki, profile, batch, dry_run, auth_profile } => {
            commands::run::run(wiki, profile, batch, dry_run, auth_profile).await
        }
        Commands::ExportLog { format, output } => {
            commands::export::run(format, output).await
        }
    }
}

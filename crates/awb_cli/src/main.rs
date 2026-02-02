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

    /// Run bot mode (unattended batch editing)
    Bot {
        /// Wiki API URL
        #[arg(long)]
        wiki: Url,

        /// Profile file path (TOML)
        #[arg(long)]
        profile: PathBuf,

        /// Maximum number of edits (default: unlimited)
        #[arg(long)]
        max_edits: Option<u32>,

        /// Dry-run mode (show diffs without saving)
        #[arg(long)]
        dry_run: bool,

        /// Checkpoint file path for resume capability
        #[arg(long)]
        checkpoint: Option<PathBuf>,

        /// Profile ID for credentials
        #[arg(long, default_value = "default")]
        auth_profile: String,

        /// Skip pages with no changes
        #[arg(long, default_value = "true")]
        skip_no_change: bool,

        /// Skip pages with warnings
        #[arg(long)]
        skip_on_warning: bool,

        /// Log progress every N pages
        #[arg(long, default_value = "10")]
        log_every_n: u32,
    },

    /// OAuth authentication management
    #[command(subcommand)]
    OAuth(OAuthCommands),
}

#[derive(Subcommand)]
enum OAuthCommands {
    /// Setup OAuth 1.0a credentials
    Setup {
        /// Wiki API URL
        #[arg(long)]
        wiki: Url,

        /// OAuth consumer key
        #[arg(long)]
        consumer_key: String,

        /// OAuth access token
        #[arg(long)]
        access_token: String,

        /// Profile ID to save credentials under
        #[arg(long, default_value = "default")]
        profile: String,
    },

    /// Authorize OAuth 2.0 (opens browser)
    Authorize {
        /// Wiki API URL
        #[arg(long)]
        wiki: Url,

        /// OAuth 2.0 client ID
        #[arg(long)]
        client_id: String,

        /// Profile ID to save credentials under
        #[arg(long, default_value = "default")]
        profile: String,
    },
}

#[derive(Clone, Debug, clap::ValueEnum)]
enum ListSource {
    Category,
    WhatLinksHere,
    Search,
    File,
    Watchlist,
    UserContribs,
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
        Commands::Login {
            wiki,
            username,
            profile,
        } => commands::login::run(wiki, username, profile).await,
        Commands::List {
            wiki,
            source,
            query,
            limit,
        } => commands::list::run(wiki, source, query, limit).await,
        Commands::Run {
            wiki,
            profile,
            batch,
            dry_run,
            auth_profile,
        } => commands::run::run(wiki, profile, batch, dry_run, auth_profile).await,
        Commands::ExportLog { format, output } => commands::export::run(format, output).await,
        Commands::Bot {
            wiki,
            profile,
            max_edits,
            dry_run,
            checkpoint,
            auth_profile,
            skip_no_change,
            skip_on_warning,
            log_every_n,
        } => {
            commands::bot::run(
                wiki,
                profile,
                max_edits,
                dry_run,
                checkpoint,
                auth_profile,
                skip_no_change,
                skip_on_warning,
                log_every_n,
            )
            .await
        }
        Commands::OAuth(oauth_cmd) => match oauth_cmd {
            OAuthCommands::Setup {
                wiki,
                consumer_key,
                access_token,
                profile,
            } => commands::oauth::setup(wiki, consumer_key, access_token, profile).await,
            OAuthCommands::Authorize {
                wiki,
                client_id,
                profile,
            } => commands::oauth::authorize(wiki, client_id, profile).await,
        },
    }
}

// SPDX-License-Identifier: AGPL-3.0-or-later

mod commands;
mod constants;
mod utils;

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use p2panda_rs::test_utils::memory_store::MemoryStore;

/// Command line arguments to configure fishy.
#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

/// Subcommands with extra arguments defining the features of fishy.
#[derive(Debug, Subcommand)]
enum Commands {
    /// Initialises all files for a new fishy project in a given folder.
    Init {
        /// Target folder where files will be created.
        #[arg(default_value = ".")]
        target_dir: PathBuf,

        /// Name of the schema which will be created.
        #[arg(short = 'n', default_value = None)]
        schema_name: Option<String>,
    },

    /// Automatically creates and signs p2panda data from a key pair and the defined schemas.
    Build {
        /// Path to the schema definition file.
        #[arg(short = 's', long = "schema", default_value = "schema.toml")]
        schema_path: PathBuf,

        /// Path to the lock file with signed and encoded p2panda data.
        #[arg(short = 'l', long = "lock", default_value = "schema.lock")]
        lock_path: PathBuf,

        /// Path to the key pair file, storing a hex-encoded ed25519 private key.
        #[arg(short = 'k', long = "key", default_value = "secret.txt")]
        private_key_path: PathBuf,

        /// Show current state without committing any changes.
        #[arg(short = 'i', long = "inspect", action=clap::ArgAction::SetTrue)]
        only_show_plan_and_exit: bool,
    },

    /// Deploy created schemas on a node.
    Deploy {
        /// GraphQL endpoint of p2panda node where schema gets deployed to.
        #[arg(short = 'e', long, default_value = "http://localhost:2020/graphql")]
        endpoint: String,

        /// Path to the lock file with signed and encoded p2panda data.
        #[arg(short = 'l', long = "lock", default_value = "schema.lock")]
        lock_path: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Cli::parse();
    let store = MemoryStore::default();

    match args.command {
        Commands::Init {
            target_dir,
            schema_name,
        } => {
            commands::init(target_dir, schema_name)
                .with_context(|| "Could not initialise new fishy project")?;
        }
        Commands::Build {
            schema_path,
            lock_path,
            private_key_path,
            only_show_plan_and_exit,
        } => {
            commands::build(
                store,
                schema_path,
                lock_path,
                private_key_path,
                only_show_plan_and_exit,
            )
            .await
            .with_context(|| "Could not create or update schema")?;
        }
        Commands::Deploy {
            lock_path,
            endpoint,
        } => commands::deploy(lock_path, &endpoint)
            .await
            .with_context(|| "Could not publish schemas to node")?,
    }

    Ok(())
}

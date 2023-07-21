// SPDX-License-Identifier: AGPL-3.0-or-later

mod commands;

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

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
        #[arg(default_value = None)]
        target_dir: Option<PathBuf>,

        /// Name of the schema which will be created.
        #[arg(short = 'n', default_value = None)]
        schema_name: Option<String>,
    },

    /// Automatically creates and signs p2panda data from a key pair and the defined schemas.
    Update {
        /// Path to the key pair file, storing a hex-encoded ed25519 private key.
        #[arg(short = 'k', long = "key", default_value = "secret.txt")]
        private_key_path: PathBuf,
    },

    /// Deploy created schemas on a node.
    Deploy {
        /// GraphQL endpoint of p2panda node where schema gets deployed to.
        #[arg(short = 'e', long, default_value = "http://localhost:2020/graphql")]
        endpoint: String,
    },
}

fn main() -> Result<()> {
    let args = Cli::parse();

    match args.command {
        Commands::Init {
            target_dir,
            schema_name,
        } => {
            commands::init(target_dir, schema_name)?;
        }
        Commands::Update { private_key_path } => todo!(),
        Commands::Deploy { endpoint } => todo!(),
    }

    Ok(())
}

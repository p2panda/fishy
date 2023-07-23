// SPDX-License-Identifier: AGPL-3.0-or-later

mod current;
mod diff;
mod executor;
mod previous;
mod print;
mod write;

use std::path::PathBuf;

use anyhow::{bail, Result};
use dialoguer::Confirm;
use p2panda_rs::test_utils::memory_store::MemoryStore;

use crate::commands::update::current::get_current_schemas;
use crate::commands::update::diff::get_diff;
use crate::commands::update::executor::execute_plan;
use crate::commands::update::previous::get_previous_schemas;
use crate::commands::update::print::print_plan;
use crate::commands::update::write::write_to_lock_file;
use crate::lock_file::LockFile;
use crate::schema_file::SchemaFile;
use crate::utils::files::absolute_path;
use crate::utils::key_pair;
use crate::utils::terminal::{print_title, print_variable};

/// Automatically creates and signs p2panda data from a key pair and the defined schemas.
pub async fn update(
    store: MemoryStore,
    schema_path: PathBuf,
    lock_path: PathBuf,
    private_key_path: PathBuf,
) -> Result<()> {
    print_title("Create operations and sign entries to update schema");
    print_variable("schema_path", absolute_path(&schema_path)?.display());
    print_variable("lock_path", absolute_path(&lock_path)?.display());
    print_variable(
        "private_key_path",
        absolute_path(&private_key_path)?.display(),
    );
    println!();

    // Load schema file
    let schema_file = SchemaFile::from_path(&schema_path)?;
    if schema_file.iter().len() == 0 {
        bail!("Schema file is empty");
    }

    // Load lock file or create new one when it does not exist yet
    let lock_file = if lock_path.exists() {
        LockFile::from_path(&lock_path)?
    } else {
        LockFile::new(&[])
    };

    // Load key pair
    let key_pair = key_pair::read_key_pair(&private_key_path)?;
    let public_key = key_pair.public_key();

    // Calculate diff between previous and current version
    let previous_schemas = get_previous_schemas(&store, &lock_file).await?;
    let current_schemas = get_current_schemas(&schema_file)?;
    let diff = get_diff(previous_schemas.clone(), current_schemas).await?;

    // Execute plan on the diff
    let (commits, plan) = execute_plan(store, key_pair, diff).await?;

    if commits.is_empty() {
        println!("No new changes to commit.");
    } else {
        // Show plan and ask user for confirmation of changes
        print_plan(plan, previous_schemas, public_key)?;

        if Confirm::new()
            .with_prompt(format!(
                "Do you want to commit these changes ({} total)?",
                commits.len()
            ))
            .interact()?
        {
            // Write commits to lock file
            write_to_lock_file(commits, lock_file, lock_path)?;
        } else {
            println!("Abort. No changes committed.")
        }
    }

    Ok(())
}

// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use dialoguer::Confirm;
use fishy::build::{
    execute_plan, get_current_schemas, get_diff, get_previous_schemas, write_to_lock_file,
};
use fishy::lock_file::LockFile;
use fishy::schema_file::SchemaFile;
use fishy::utils::files::absolute_path;
use fishy::utils::key_pair;
use p2panda_rs::test_utils::memory_store::MemoryStore;

use crate::utils::print::print_plan;
use crate::utils::terminal::{print_title, print_variable};

/// Automatically creates and signs p2panda data from a key pair and the defined schemas.
pub async fn build(
    store: MemoryStore,
    schema_path: PathBuf,
    lock_path: PathBuf,
    private_key_path: PathBuf,
    only_show_plan_and_exit: bool,
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
    let schema_file = SchemaFile::from_path(&schema_path).context(format!(
        "Try reading schema file from path '{}'",
        schema_path.display()
    ))?;
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
    let key_pair = key_pair::read_key_pair(&private_key_path).context(format!(
        "Try reading private key file from path '{}'",
        private_key_path.display()
    ))?;
    let public_key = key_pair.public_key();

    // Calculate diff between previous and current version
    let previous_schemas = get_previous_schemas(&store, &lock_file).await?;
    let current_schemas = get_current_schemas(&schema_file)?;
    let diff = get_diff(previous_schemas.clone(), current_schemas).await?;

    // Execute plan on the diff
    let (commits, plan) = execute_plan(store, key_pair, diff).await?;

    // We can also choose to only show the plan and exit directly, without committing any changes.
    // This is useful if we want to find out the schema id and state
    if only_show_plan_and_exit {
        print_plan(plan, previous_schemas, public_key, false)?;
        return Ok(());
    }

    if commits.is_empty() {
        println!("No new changes to commit.");
    } else {
        // Show plan to user and ask for confirmation
        print_plan(plan, previous_schemas, public_key, true)?;

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

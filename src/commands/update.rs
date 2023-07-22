// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::PathBuf;

use anyhow::Result;
use p2panda_rs::identity::KeyPair;

use crate::lock_file::{Commit, LockFile};
use crate::schema_file::SchemaFile;
use crate::utils::files::absolute_path;
use crate::utils::key_pair;
use crate::utils::terminal::{print_title, print_variable};

/// Automatically creates and signs p2panda data from a key pair and the defined schemas.
pub fn update(schema_path: PathBuf, lock_path: PathBuf, private_key_path: PathBuf) -> Result<()> {
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

    // Load lock file or create new one when it does not exist yet
    let lock_file = if lock_path.exists() {
        LockFile::from_path(&lock_path)?
    } else {
        LockFile::new(&[])
    };

    // Load key pair
    let key_pair = key_pair::read_key_pair(&private_key_path)?;

    // Plan update and generate required commits from it
    let planned_schemas = get_planned_schemas(&schema_file)?;
    let current_schemas = get_current_schemas(&lock_file)?;
    let commits = commit_updates(planned_schemas, current_schemas, &key_pair)?;

    // Show diff and ask user for confirmation of changes
    // @TODO

    // Write commits to lock file
    // @TODO

    Ok(())
}

struct PlannedSchemas;

struct CurrentSchemas;

fn get_planned_schemas(schema_file: &SchemaFile) -> Result<PlannedSchemas> {
    todo!();
}

fn get_current_schemas(lock_file: &LockFile) -> Result<CurrentSchemas> {
    todo!();
}

fn commit_updates(
    planned_schemas: PlannedSchemas,
    current_schemas: CurrentSchemas,
    key_pair: &KeyPair,
) -> Result<Vec<Commit>> {
    todo!();
}

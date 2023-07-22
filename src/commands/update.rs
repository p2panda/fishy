// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::PathBuf;

use anyhow::{bail, Result};
use p2panda_rs::identity::KeyPair;
use p2panda_rs::operation::decode::decode_operation;
use p2panda_rs::operation::traits::Schematic;
use p2panda_rs::schema::{Schema, SchemaDescription, SchemaId, SchemaName};

use crate::lock_file::{Commit, LockFile};
use crate::schema_file::{SchemaFields, SchemaFile};
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

/// Schema which was defined in the user's schema file.
#[derive(Debug)]
struct PlannedSchema {
    pub name: SchemaName,
    pub description: SchemaDescription,
    pub fields: SchemaFields,
}

impl PlannedSchema {
    pub fn new(name: &SchemaName, description: &SchemaDescription, fields: &SchemaFields) -> Self {
        Self {
            name: name.clone(),
            description: description.clone(),
            fields: fields.clone(),
        }
    }
}

/// Extracts all schema definitions from user file and returns them as planned schemas.
fn get_planned_schemas(schema_file: &SchemaFile) -> Result<Vec<PlannedSchema>> {
    schema_file
        .iter()
        .map(|(schema_name, schema_definition)| {
            if schema_definition.fields.len() == 0 {
                bail!("Schema {schema_name} does not contain any fields");
            }

            Ok(PlannedSchema::new(
                schema_name,
                &schema_definition.description,
                &schema_definition.fields,
            ))
        })
        .collect()
}

struct CurrentSchemas;

/// Reads currently committed operations from lock file, materializes schema documents from them
/// and returns these schemas.
fn get_current_schemas(lock_file: &LockFile) -> Result<CurrentSchemas> {
    // Sometimes `commits` is not defined in the .toml file, set an empty array as a fallback
    let commits = lock_file.commits.clone().unwrap_or(vec![]);

    // Publish every commit in our temporary, in-memory "node" to materialize schema documents
    for commit in commits {
        let plain_operation = decode_operation(&commit.operation)?;

        // Derive schema definitions from the operation's schema id. This fails if there's an
        // invalid id or unknown system schema version.
        let schema = match plain_operation.schema_id() {
            SchemaId::SchemaDefinition(version) => {
                Schema::get_system(SchemaId::SchemaDefinition(*version))?
            }
            SchemaId::SchemaFieldDefinition(version) => {
                Schema::get_system(SchemaId::SchemaFieldDefinition(*version))?
            }
            schema_id => {
                bail!("Detected commit with invalid schema id {schema_id} in lock file");
            }
        };

        // @TODO
        /* publish(
            &context.store,
            schema,
            &commit.entry,
            &plain_operation,
            &commit.operation,
        )
        .await?; */
    }

    let ret = CurrentSchemas {};
    Ok(ret)
}

fn commit_updates(
    planned_schemas: Vec<PlannedSchema>,
    current_schemas: CurrentSchemas,
    key_pair: &KeyPair,
) -> Result<Vec<Commit>> {
    todo!();
}

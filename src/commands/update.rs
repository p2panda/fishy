// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{anyhow, bail, Context, Result};
use p2panda_rs::api::publish;
use p2panda_rs::document::traits::AsDocument;
use p2panda_rs::entry::traits::AsEncodedEntry;
use p2panda_rs::identity::KeyPair;
use p2panda_rs::operation::decode::decode_operation;
use p2panda_rs::operation::traits::Schematic;
use p2panda_rs::schema::system::{SchemaFieldView, SchemaView};
use p2panda_rs::schema::{Schema, SchemaDescription, SchemaId, SchemaName};
use p2panda_rs::storage_provider::traits::DocumentStore;
use p2panda_rs::test_utils::memory_store::MemoryStore;

use crate::lock_file::{Commit, LockFile};
use crate::schema_file::{SchemaFields, SchemaFile};
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
    let current_schemas = get_current_schemas(&store, &lock_file).await?;
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

#[derive(Debug)]
struct CurrentSchema {
    pub schema: Schema,
    pub schema_view: SchemaView,
    pub schema_field_views: Vec<SchemaFieldView>,
}

impl CurrentSchema {
    pub fn new(
        schema: &Schema,
        schema_view: &SchemaView,
        schema_field_views: &[SchemaFieldView],
    ) -> Self {
        Self {
            schema: schema.clone(),
            schema_view: schema_view.clone(),
            schema_field_views: schema_field_views.to_vec(),
        }
    }
}

type CurrentSchemas = HashMap<SchemaName, CurrentSchema>;

/// Reads currently committed operations from lock file, materializes schema documents from them
/// and returns these schemas.
async fn get_current_schemas(store: &MemoryStore, lock_file: &LockFile) -> Result<CurrentSchemas> {
    // Sometimes `commits` is not defined in the .toml file, set an empty array as a fallback
    let commits = lock_file.commits.clone().unwrap_or(vec![]);

    // Publish every commit in our temporary, in-memory "node" to materialize schema documents
    for commit in commits {
        // Check entry hash integrity
        if commit.entry_hash != commit.entry.hash() {
            bail!(
                "Entry hash {} does not match it's content",
                commit.entry_hash
            );
        }

        // Decode operation
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

        // Publish commits to a in-memory node where they get materialized to documents. This fully
        // validates the given entries and operations.
        publish(
            store,
            schema,
            &commit.entry,
            &plain_operation,
            &commit.operation,
        )
        .await
        .with_context(|| "Invalid commits detected")?;
    }

    // Load materialized documents from node and assemble them
    let mut current_schemas = CurrentSchemas::new();

    let definitions = store
        .get_documents_by_schema(&SchemaId::SchemaDefinition(1))
        .await
        .with_context(|| "Critical storage failure")?;

    for definition in definitions {
        let document_view = definition.view();

        // Skip over deleted documents
        if document_view.is_none() {
            continue;
        }

        // Convert document view into more specialized schema view. Unwrap here, since we know the
        // document was not deleted at this point.
        let schema_view = SchemaView::try_from(document_view.unwrap())?;

        // Assemble all fields for this schema
        let mut schema_field_views: Vec<SchemaFieldView> = Vec::new();

        for view_id in schema_view.fields().iter() {
            let field_definition = store
                .get_document_by_view_id(view_id)
                .await
                .with_context(|| "Critical storage failure")?
                .ok_or_else(|| {
                    anyhow!(
                        "Missing field definition document {view_id} for schema {}",
                        schema_view.view_id()
                    )
                })?;

            // Convert document view into more specialized schema field view
            let document_view = field_definition
                .view()
                .ok_or_else(|| anyhow!("Can not assign a deleted schema field to a schema"))?;
            schema_field_views.push(SchemaFieldView::try_from(document_view)?);
        }

        // Finally assemble the schema from all its parts ..
        let schema = Schema::from_views(schema_view.clone(), schema_field_views.clone())
            .with_context(|| {
                format!(
                    "Could not assemble schema with view id {} from given documents",
                    definition.view_id()
                )
            })?;

        // .. and add it to the resulting hash map
        current_schemas.insert(
            schema.id().name(),
            CurrentSchema::new(&schema, &schema_view, &schema_field_views),
        );
    }

    Ok(current_schemas)
}

fn commit_updates(
    planned_schemas: Vec<PlannedSchema>,
    current_schemas: CurrentSchemas,
    key_pair: &KeyPair,
) -> Result<Vec<Commit>> {
    todo!();
}

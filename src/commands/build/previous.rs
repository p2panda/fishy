// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashMap;

use anyhow::{anyhow, bail, Context, Result};
use p2panda_rs::api::publish;
use p2panda_rs::document::traits::AsDocument;
use p2panda_rs::entry::traits::AsEncodedEntry;
use p2panda_rs::operation::decode::decode_operation;
use p2panda_rs::operation::traits::Schematic;
use p2panda_rs::schema::system::{SchemaFieldView, SchemaView};
use p2panda_rs::schema::{Schema, SchemaId, SchemaName};
use p2panda_rs::storage_provider::traits::DocumentStore;
use p2panda_rs::test_utils::memory_store::MemoryStore;

use crate::lock_file::LockFile;

/// Reads previously committed operations from lock file, materializes schema documents from them
/// and returns these schemas.
pub async fn get_previous_schemas(
    store: &MemoryStore,
    lock_file: &LockFile,
) -> Result<PreviousSchemas> {
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
    let mut previous_schemas = PreviousSchemas::new();

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
        previous_schemas.insert(
            schema.id().name(),
            PreviousSchema::new(&schema, &schema_view, &schema_field_views),
        );
    }

    Ok(previous_schemas)
}

/// Materialized schemas the user already committed.
#[derive(Clone, Debug)]
pub struct PreviousSchema {
    pub schema: Schema,
    pub schema_view: SchemaView,
    pub schema_field_views: Vec<SchemaFieldView>,
}

impl PreviousSchema {
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

pub type PreviousSchemas = HashMap<SchemaName, PreviousSchema>;

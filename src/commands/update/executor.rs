// SPDX-License-Identifier: AGPL-3.0-or-later

use anyhow::{anyhow, Result};
use async_trait::async_trait;

use p2panda_rs::document::DocumentViewId;
use p2panda_rs::entry::traits::AsEncodedEntry;
use p2panda_rs::hash::Hash;
use p2panda_rs::identity::KeyPair;
use p2panda_rs::operation::encode::encode_operation;
use p2panda_rs::operation::traits::Schematic;
use p2panda_rs::operation::{
    Operation, OperationAction, OperationBuilder, OperationValue, PinnedRelationList,
};
use p2panda_rs::schema::{FieldType as PandaFieldType, Schema, SchemaId};
use p2panda_rs::test_utils::memory_store::helpers::send_to_store;
use p2panda_rs::test_utils::memory_store::MemoryStore;

use crate::lock_file::Commit;
use crate::schema_file::{FieldType, RelationType};

use super::diff::{FieldDiff, FieldTypeDiff, SchemaDiff};

/// Execute the changes required to get from the previous version to the current.
///
/// Returns a list of signed commits and information about the steps which have been taken.
pub async fn execute_plan(
    store: MemoryStore,
    key_pair: KeyPair,
    diffs: Vec<SchemaDiff>,
) -> Result<(Vec<Commit>, Vec<Plan>)> {
    let mut executor = Executor::new(store, key_pair);

    for diff in diffs {
        diff.execute(&mut executor).await?;
    }

    Ok((executor.commits, executor.plans))
}

/// This executor accounts for the nested, recursive layout of schemas and their dependencies.
///
/// It iterates over the dependency graph in a depth-first order, calculates the required changes
/// and generates operations out of them.
#[derive(Debug)]
pub struct Executor {
    store: MemoryStore,
    key_pair: KeyPair,
    commits: Vec<Commit>,
    plans: Vec<Plan>,
}

impl Executor {
    /// Returns a new instance of `Executor`.
    fn new(store: MemoryStore, key_pair: KeyPair) -> Self {
        Self {
            store,
            key_pair,
            commits: Vec::new(),
            plans: Vec::new(),
        }
    }

    /// Signs and publishes an operation and keeps track of the resulting commit.
    async fn commit(&mut self, operation: &Operation) -> Result<Hash> {
        // Encode operation
        let schema = Schema::get_system(operation.schema_id().to_owned())?;
        let encoded_operation = encode_operation(operation)?;

        // Publish operation on node which might already contain data from previously published
        // schemas
        let (encoded_entry, _) = send_to_store(&self.store, operation, schema, &self.key_pair)
            .await
            .map_err(|err| anyhow!("Critical storage failure: {err}"))?;

        self.commits
            .push(Commit::new(&encoded_entry, &encoded_operation));

        Ok(encoded_entry.hash())
    }
}

#[async_trait]
pub trait Executable {
    /// Iterate over dependencies and commit required changes.
    async fn execute(&self, executor: &mut Executor) -> Result<DocumentViewId>;
}

/// After execution we know all changes and all resulting schema ids.
#[derive(Clone, Debug)]
pub struct Plan(SchemaId, SchemaDiff);

impl Plan {
    pub fn new(schema_id: SchemaId, diff: &SchemaDiff) -> Self {
        Self(schema_id, diff.clone())
    }

    pub fn schema_id(&self) -> SchemaId {
        self.0.clone()
    }

    pub fn schema_diff(&self) -> SchemaDiff {
        self.1.clone()
    }
}

#[async_trait]
impl Executable for SchemaDiff {
    async fn execute(&self, executor: &mut Executor) -> Result<DocumentViewId> {
        // Execute all fields first, they are direct dependencies of a schema
        let mut field_view_ids: Vec<DocumentViewId> = Vec::new();

        for field in &self.current_fields {
            let field_view_id = field.execute(executor).await?;
            field_view_ids.push(field_view_id);
        }

        let operation: Option<Operation> = match &self.previous_schema_view {
            // A previous version of this schema existed already
            Some(previous_schema_view) => {
                let mut fields: Vec<(&str, OperationValue)> = Vec::new();

                if self.current_description.to_string() != previous_schema_view.description() {
                    fields.push(("description", self.current_description.to_string().into()));
                }

                if &PinnedRelationList::new(field_view_ids.clone()) != previous_schema_view.fields()
                {
                    fields.push(("fields", field_view_ids.into()));
                }

                if !fields.is_empty() {
                    let operation = OperationBuilder::new(&SchemaId::SchemaDefinition(1))
                        .previous(previous_schema_view.view_id())
                        .action(OperationAction::Update)
                        .fields(&fields)
                        .build()?;

                    Some(operation)
                } else {
                    // Nothing has changed ..
                    None
                }
            }

            // We can not safely determine a previous version, either it never existed or its name
            // changed. Let's create a new document!
            None => {
                let operation = OperationBuilder::new(&SchemaId::SchemaDefinition(1))
                    .action(OperationAction::Create)
                    .fields(&[
                        ("name", self.name.to_string().into()),
                        ("description", self.current_description.to_string().into()),
                        ("fields", field_view_ids.into()),
                    ])
                    .build()?;

                Some(operation)
            }
        };

        // Get the document view id of the created / updated document
        let view_id = match operation {
            Some(operation) => {
                let entry_hash = executor.commit(&operation).await?;
                entry_hash.into()
            }
            None => self
                .previous_schema_view
                .as_ref()
                .expect("Document to not be deleted")
                .view_id()
                .clone(),
        };

        // Derive the schema id and add it to our list of plans together with the diff
        let schema_id = SchemaId::new_application(&self.name, &view_id);
        executor.plans.push(Plan::new(schema_id, self));

        Ok(view_id)
    }
}

#[async_trait]
impl Executable for FieldDiff {
    async fn execute(&self, executor: &mut Executor) -> Result<DocumentViewId> {
        let current_field_type = match &self.current_field_type {
            // Convert all basic field types
            FieldTypeDiff::Field(FieldType::String) => PandaFieldType::String,
            FieldTypeDiff::Field(FieldType::Boolean) => PandaFieldType::Boolean,
            FieldTypeDiff::Field(FieldType::Float) => PandaFieldType::Float,
            FieldTypeDiff::Field(FieldType::Integer) => PandaFieldType::Integer,

            // Convert relation field types
            FieldTypeDiff::Relation(relation, schema_plan) => {
                // Get id of schema this field relates to
                let schema = executor
                    .plans
                    .iter()
                    .find(|plan| &plan.schema_diff() == schema_plan);

                let schema_id = match schema {
                    // Schema was already materialized
                    Some(schema) => schema.schema_id(),
                    // Materialize schema first
                    None => {
                        let view_id = schema_plan.execute(executor).await?;
                        SchemaId::new_application(&schema_plan.name, &view_id)
                    }
                };

                match relation {
                    RelationType::Relation => PandaFieldType::Relation(schema_id),
                    RelationType::RelationList => PandaFieldType::RelationList(schema_id),
                    RelationType::PinnedRelation => PandaFieldType::PinnedRelation(schema_id),
                    RelationType::PinnedRelationList => {
                        PandaFieldType::PinnedRelationList(schema_id)
                    }
                }
            }
        };

        let operation: Option<Operation> = match &self.previous_field_view {
            // A previous version of this field existed already
            Some(previous_field_view) => {
                if previous_field_view.field_type() != &current_field_type {
                    let operation = OperationBuilder::new(&SchemaId::SchemaFieldDefinition(1))
                        .action(OperationAction::Update)
                        .previous(previous_field_view.id()) // view_id
                        .fields(&[("type", current_field_type.into())])
                        .build()?;

                    Some(operation)
                } else {
                    // Nothing has changed ..
                    None
                }
            }

            // This field did not exist before, let's create a new document!
            None => {
                let operation = OperationBuilder::new(&SchemaId::SchemaFieldDefinition(1))
                    .action(OperationAction::Create)
                    .fields(&[
                        ("name", self.name.clone().into()),
                        ("type", current_field_type.into()),
                    ])
                    .build()?;

                Some(operation)
            }
        };

        match operation {
            Some(operation) => {
                let entry_hash = executor.commit(&operation).await?;
                Ok(entry_hash.into())
            }
            None => Ok(self
                .previous_field_view
                .as_ref()
                .expect("Document to not be deleted")
                .id() // view_id
                .clone()),
        }
    }
}

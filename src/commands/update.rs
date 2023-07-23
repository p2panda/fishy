// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{anyhow, bail, Context, Result};
use async_trait::async_trait;
use comfy_table::modifiers::UTF8_ROUND_CORNERS;
use comfy_table::presets::UTF8_FULL;
use comfy_table::{Cell, Color, Table};
use console::style;
use dialoguer::Confirm;
use p2panda_rs::api::publish;
use p2panda_rs::document::traits::AsDocument;
use p2panda_rs::document::DocumentViewId;
use p2panda_rs::entry::traits::AsEncodedEntry;
use p2panda_rs::hash::Hash;
use p2panda_rs::identity::{KeyPair, PublicKey};
use p2panda_rs::operation::decode::decode_operation;
use p2panda_rs::operation::encode::encode_operation;
use p2panda_rs::operation::traits::Schematic;
use p2panda_rs::operation::{
    Operation, OperationAction, OperationBuilder, OperationValue, PinnedRelationList,
};
use p2panda_rs::schema::system::{SchemaFieldView, SchemaView};
use p2panda_rs::schema::{
    FieldName, FieldType as PandaFieldType, Schema, SchemaDescription, SchemaId, SchemaName,
};
use p2panda_rs::storage_provider::traits::DocumentStore;
use p2panda_rs::test_utils::memory_store::helpers::send_to_store;
use p2panda_rs::test_utils::memory_store::MemoryStore;
use topological_sort::TopologicalSort;

use crate::lock_file::{Commit, LockFile};
use crate::schema_file::{
    FieldType, RelationId, RelationSchema, RelationType, SchemaField, SchemaFields, SchemaFile,
};
use crate::utils::files::{self, absolute_path};
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

/// Schema which was defined in the user's schema file.
#[derive(Clone, Debug)]
struct CurrentSchema {
    pub name: SchemaName,
    pub description: SchemaDescription,
    pub fields: SchemaFields,
}

impl CurrentSchema {
    pub fn new(name: &SchemaName, description: &SchemaDescription, fields: &SchemaFields) -> Self {
        Self {
            name: name.clone(),
            description: description.clone(),
            fields: fields.clone(),
        }
    }
}

/// Extracts all schema definitions from user file and returns them as current schemas.
fn get_current_schemas(schema_file: &SchemaFile) -> Result<Vec<CurrentSchema>> {
    schema_file
        .iter()
        .map(|(schema_name, schema_definition)| {
            if schema_definition.fields.len() == 0 {
                bail!("Schema {schema_name} does not contain any fields");
            }

            Ok(CurrentSchema::new(
                schema_name,
                &schema_definition.description,
                &schema_definition.fields,
            ))
        })
        .collect()
}

/// Materialized schemas the user already committed.
#[derive(Clone, Debug)]
struct PreviousSchema {
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

type PreviousSchemas = HashMap<SchemaName, PreviousSchema>;

/// Reads previously committed operations from lock file, materializes schema documents from them
/// and returns these schemas.
async fn get_previous_schemas(
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

/// This executor accounts for the nested, recursive layout of schemas and their dependencies.
///
/// It iterates over the dependency graph in a depth-first order, calculates the required changes
/// and generates operations out of them.
#[derive(Debug)]
struct Executor {
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
trait Executable {
    /// Iterate over dependencies and commit required changes.
    async fn execute(&self, executor: &mut Executor) -> Result<DocumentViewId>;
}

/// After execution we know all changes and all resulting schema ids.
#[derive(Clone, Debug)]
struct Plan(SchemaId, SchemaDiff);

impl Plan {
    pub fn new(schema_id: SchemaId, diff: &SchemaDiff) -> Self {
        Self(schema_id, diff.clone())
    }
}

/// Information about the previous and current version of a schema.
///
/// The contained field definition documents are direct dependencies of the schema definition
/// document.
#[derive(Clone, Debug, Eq, PartialEq)]
struct SchemaDiff {
    /// Name of the schema.
    name: SchemaName,

    /// Previous version of this schema (if it existed).
    previous_schema_view: Option<SchemaView>,

    /// Current version of the schema description.
    current_description: SchemaDescription,

    /// Current version of the schema fields.
    current_fields: Vec<FieldDiff>,
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

/// Information about the previous and current version of a field.
///
/// A field of relation type links to a schema which is a direct dependency.
#[derive(Clone, Debug, Eq, PartialEq)]
struct FieldDiff {
    /// Name of the schema field.
    name: FieldName,

    /// Previous version of this field (if it existed).
    previous_field_view: Option<SchemaFieldView>,

    /// Current version of the field type.
    current_field_type: FieldTypeDiff,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum FieldTypeDiff {
    /// Basic schema field type.
    Field(FieldType),

    /// Relation field type linked to a schema.
    Relation(RelationType, SchemaDiff),
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
                let schema = executor.plans.iter().find(|plan| &plan.1 == schema_plan);

                let schema_id = match schema {
                    // Schema was already materialized
                    Some(schema) => schema.0.clone(),
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

/// Gathers the differences between the current and the previous versions and organises them in
/// nested, topological order as some changes depend on each other.
async fn get_diff(
    previous_schemas: PreviousSchemas,
    current_schemas: Vec<CurrentSchema>,
) -> Result<Vec<SchemaDiff>> {
    // Create a linked dependency graph from all schemas and their relations to each other: Fields
    // are direct dependencies of schemas, relation fields are dependend on their linked schemas.
    //
    // We can apply topological ordering to determine which schemas need to be materialized first
    // before the others can relate to them.
    let mut graph = TopologicalSort::<SchemaName>::new();

    for current_schema in current_schemas.iter() {
        graph.insert(current_schema.name.clone());

        for (_, schema_field) in current_schema.fields.iter() {
            if let SchemaField::Relation { schema, .. } = schema_field {
                match &schema.id {
                    RelationId::Name(linked_schema) => {
                        graph.add_dependency(linked_schema.clone(), current_schema.name.clone());
                    }
                    RelationId::Id(_) => bail!("Relating to schemas via `id` is not supported yet"),
                }
            }
        }
    }

    let mut sorted_schemas: Vec<SchemaName> = Vec::new();
    loop {
        let mut next = graph.pop_all();

        if next.is_empty() && !graph.is_empty() {
            bail!("Cyclic dependency detected between relations");
        } else if next.is_empty() {
            break;
        } else {
            sorted_schemas.append(&mut next);
        }
    }

    // Based on this sorted list in topological order we can now extend it with information about
    // what was previously given and what the current state is. This will help us to determine the
    // concrete changes required to get to the current version
    let mut schema_diffs: Vec<SchemaDiff> = Vec::new();

    for current_schema_name in &sorted_schemas {
        // Get the previous (if it exists) and current schema versions
        let previous_schema = previous_schemas.get(current_schema_name);
        let current_schema = current_schemas
            .iter()
            .find(|item| &item.name == current_schema_name)
            // Since we sorted everything in topological order we can be sure that this exists
            .expect("Current schema needs to be given in array");

        // Get the regarding current or previously existing fields and derive plans from it
        let mut field_diffs: Vec<FieldDiff> = Vec::new();

        for (current_field_name, current_field) in current_schema.fields.iter() {
            // Get the current field version
            let current_field_type = match current_field {
                SchemaField::Field { field_type } => FieldTypeDiff::Field(field_type.clone()),
                SchemaField::Relation { field_type, schema } => match &schema.id {
                    RelationId::Name(linked_schema_name) => {
                        let schema_diff = schema_diffs
                            .iter()
                            .find(|plan| &plan.name == linked_schema_name)
                            // Since we sorted everything in topological order we can be sure that
                            // this exists when we look for it
                            .expect("Current schema needs to be given in array");

                        FieldTypeDiff::Relation(field_type.clone(), schema_diff.clone())
                    }
                    RelationId::Id(_) => bail!("Relating to schemas via `id` is not supported yet"),
                },
            };

            // Get the previous field version (if it existed)
            let previous_field_view = match previous_schema {
                Some(schema) => schema
                    .schema_field_views
                    .iter()
                    .find(|field| field.name() == current_field_name)
                    .cloned(),
                None => None,
            };

            let field_diff = FieldDiff {
                name: current_field_name.clone(),
                previous_field_view,
                current_field_type,
            };

            field_diffs.push(field_diff);
        }

        // Get the previous schema version (if it existed)
        let previous_schema_view = previous_schema.map(|schema| schema.schema_view.clone());

        let schema_diff = SchemaDiff {
            name: current_schema_name.clone(),
            previous_schema_view,
            current_description: current_schema.description.clone(),
            current_fields: field_diffs,
        };

        schema_diffs.push(schema_diff);
    }

    let result: Vec<SchemaDiff> = sorted_schemas
        .iter()
        .map(|group| {
            return schema_diffs
                .iter()
                .find(|diff| &diff.name == group)
                .cloned()
                .expect("Diff exists at this point");
        })
        .collect();

    Ok(result)
}

/// Execute the changes required to get from the previous version to the current.
///
/// Returns a list of signed commits and information about the steps which have been taken.
async fn execute_plan(
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

/// Shows the execution plan to the user.
fn print_plan(
    plans: Vec<Plan>,
    previous_schemas: PreviousSchemas,
    public_key: PublicKey,
) -> Result<()> {
    println!("The following changes will be applied:\n");

    for plan in plans {
        let schema_diff = plan.1;

        // Schema id
        let current_schema_id = plan.0;
        let previous_schema_id = match &schema_diff.previous_schema_view {
            Some(view) => {
                let schema_name = SchemaName::new(view.name())?;
                Some(SchemaId::new_application(&schema_name, view.view_id()))
            }
            None => None,
        };

        // Description
        let current_description = schema_diff.current_description;
        let previous_description = match &schema_diff.previous_schema_view {
            Some(view) => {
                let schema_description = SchemaDescription::new(view.description())?;
                Some(schema_description)
            }
            None => None,
        };

        // Fields
        let current_fields: SchemaFields = {
            let mut fields = SchemaFields::new();

            for field in schema_diff.current_fields {
                let schema_field: SchemaField = match field.current_field_type {
                    FieldTypeDiff::Field(field_type) => SchemaField::Field { field_type },
                    FieldTypeDiff::Relation(field_type, schema_diff) => SchemaField::Relation {
                        field_type,
                        schema: RelationSchema {
                            id: RelationId::Name(schema_diff.name.clone()),
                            external: None,
                        },
                    },
                };

                fields.insert(&field.name, &schema_field);
            }

            fields
        };

        let previous_fields = match &schema_diff.previous_schema_view {
            Some(previous) => {
                let mut fields = SchemaFields::new();

                let previous_schema = previous_schemas
                    .values()
                    .find(|item| {
                        return item.schema_view.view_id() == previous.view_id();
                    })
                    .expect("Needs to exist at this point");

                for (field_name, field_type) in previous_schema.schema.fields().iter() {
                    let schema_field = match field_type {
                        PandaFieldType::Boolean => SchemaField::Field {
                            field_type: FieldType::Boolean,
                        },
                        PandaFieldType::Integer => SchemaField::Field {
                            field_type: FieldType::Integer,
                        },
                        PandaFieldType::Float => SchemaField::Field {
                            field_type: FieldType::Float,
                        },
                        PandaFieldType::String => SchemaField::Field {
                            field_type: FieldType::String,
                        },
                        PandaFieldType::Relation(schema_id) => SchemaField::Relation {
                            field_type: RelationType::Relation,
                            schema: RelationSchema {
                                id: RelationId::Name(schema_id.name()),
                                external: None,
                            },
                        },
                        PandaFieldType::RelationList(schema_id) => SchemaField::Relation {
                            field_type: RelationType::RelationList,
                            schema: RelationSchema {
                                id: RelationId::Name(schema_id.name()),
                                external: None,
                            },
                        },
                        PandaFieldType::PinnedRelation(schema_id) => SchemaField::Relation {
                            field_type: RelationType::PinnedRelation,
                            schema: RelationSchema {
                                id: RelationId::Name(schema_id.name()),
                                external: None,
                            },
                        },
                        PandaFieldType::PinnedRelationList(schema_id) => SchemaField::Relation {
                            field_type: RelationType::PinnedRelationList,
                            schema: RelationSchema {
                                id: RelationId::Name(schema_id.name()),
                                external: None,
                            },
                        },
                    };

                    fields.insert(field_name, &schema_field);
                }

                Some(fields)
            }
            None => None,
        };

        let mut fields: HashMap<FieldName, (Option<SchemaField>, Option<SchemaField>)> =
            HashMap::new();

        for (field_name, field_type) in current_fields.iter() {
            fields.insert(field_name.clone(), (Some(field_type.clone()), None));
        }

        if let Some(previous_fields) = &previous_fields {
            for (field_name, field_type) in previous_fields.iter() {
                match fields.get(field_name) {
                    Some(entry) => {
                        fields.insert(
                            field_name.clone(),
                            (entry.0.clone(), Some(field_type.clone())),
                        );
                    }
                    None => {
                        fields.insert(field_name.clone(), (None, Some(field_type.clone())));
                    }
                }
            }
        }

        // Skip this schema if nothing has changed
        if let Some(previous_schema_id) = &previous_schema_id {
            if previous_schema_id.version() == current_schema_id.version() {
                continue;
            }
        }

        // Display schema id
        println!(
            "{}",
            style(format!("{current_schema_id}")).bold().underlined(),
        );

        if let Some(previous_schema_id) = &previous_schema_id {
            println!("Previously: {previous_schema_id}");
        }

        // Display name
        println!();
        println!(
            "Name: {}",
            style(current_schema_id.name()).fg(if previous_schema_id.is_some() {
                console::Color::White
            } else {
                console::Color::Green
            })
        );

        // Display description
        if let Some(previous_description) = &previous_description {
            if previous_description != &current_description {
                println!(
                    "Description: {}",
                    style(format!(
                        "\"{previous_description}\" -> \"{current_description}\""
                    ))
                    .yellow()
                );
            } else {
                println!("Description: \"{current_description}\"");
            }
        } else {
            println!(
                "Description: {}",
                style(format!("\"{current_description}\"")).green()
            );
        }

        // Display fields
        let mut table = Table::new();
        table
            .load_preset(UTF8_FULL)
            .apply_modifier(UTF8_ROUND_CORNERS)
            .set_header(vec!["#", "Field Name", "Field Type"]);

        for (index, (field_name, (current_field, previous_field))) in fields.iter().enumerate() {
            let color = match (current_field, previous_field) {
                (None, Some(_)) => Color::Red,
                (Some(_), None) => Color::Green,
                (Some(current), Some(previous)) => {
                    if current != previous {
                        Color::Yellow
                    } else {
                        Color::White
                    }
                }
                _ => unreachable!(),
            };

            let field_type = match (current_field, previous_field) {
                (None, Some(previous)) => format!("{previous}"),
                (Some(current), None) => format!("{current}"),
                (Some(current), Some(previous)) => {
                    if current != previous {
                        format!("{previous} -> {current}")
                    } else {
                        format!("{current}")
                    }
                }
                _ => unreachable!(),
            };

            table.add_row(vec![
                Cell::new((index + 1).to_string()).fg(color),
                Cell::new(field_name.to_owned()).fg(color),
                Cell::new(field_type).fg(color),
            ]);
        }

        println!("{table}\n");
    }

    println!(
        "Public key used for signing: {}\n",
        style(public_key).bold()
    );

    Ok(())
}

/// Write commits to lock file.
fn write_to_lock_file(
    mut new_commits: Vec<Commit>,
    mut lock_file: LockFile,
    lock_path: PathBuf,
) -> Result<()> {
    // Add new commits to the existing ones
    let applied_commits_count = new_commits.len();
    let mut commits: Vec<Commit> = Vec::new();

    if let Some(current_commits) = lock_file.commits.as_mut() {
        commits.append(current_commits);
    }

    commits.append(&mut new_commits);

    // Write everything to .toml file
    let lock_file = LockFile::new(&commits);

    let lock_file_str = format!(
        "{}\n\n{}",
        "# This file is automatically generated by fishy.\n# It is not intended for manual editing.",
        toml::to_string_pretty(&lock_file)?
    );

    files::write_file(lock_path, &lock_file_str)?;

    println!(
        "Successfully written {} new commits to schema.lock file",
        applied_commits_count
    );

    Ok(())
}

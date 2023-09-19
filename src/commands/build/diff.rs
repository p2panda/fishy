// SPDX-License-Identifier: AGPL-3.0-or-later

use anyhow::{bail, Result};
use p2panda_rs::schema::system::{SchemaFieldView, SchemaView};
use p2panda_rs::schema::{FieldName, SchemaDescription, SchemaId, SchemaName};
use topological_sort::TopologicalSort;

use crate::schema_file::{FieldType, RelationId, RelationType, SchemaField};

use super::current::CurrentSchema;
use super::previous::PreviousSchemas;

/// Gathers the differences between the current and the previous versions and organises them in
/// nested, topological order as some changes depend on each other.
pub async fn get_diff(
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
                    RelationId::Id(schema_id) => {
                        // @TODO: Is it fine to do nothing here?
                    }
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
                    RelationId::Id(schema_id) => {
                        FieldTypeDiff::ExternalRelation(field_type.clone(), schema_id.to_owned())
                    }
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

/// Information about the previous and current version of a schema.
///
/// The contained field definition documents are direct dependencies of the schema definition
/// document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchemaDiff {
    /// Name of the schema.
    pub name: SchemaName,

    /// Previous version of this schema (if it existed).
    pub previous_schema_view: Option<SchemaView>,

    /// Current version of the schema description.
    pub current_description: SchemaDescription,

    /// Current version of the schema fields.
    pub current_fields: Vec<FieldDiff>,
}

/// Information about the previous and current version of a field.
///
/// A field of relation type links to a schema which is a direct dependency.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldDiff {
    /// Name of the schema field.
    pub name: FieldName,

    /// Previous version of this field (if it existed).
    pub previous_field_view: Option<SchemaFieldView>,

    /// Current version of the field type.
    pub current_field_type: FieldTypeDiff,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FieldTypeDiff {
    /// Basic schema field type.
    Field(FieldType),

    /// Relation field type linked to a schema.
    Relation(RelationType, SchemaDiff),

    /// Relation field type linked to an external schema which is not defined in this context.
    ExternalRelation(RelationType, SchemaId),
}

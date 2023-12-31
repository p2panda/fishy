// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::BTreeMap;
use std::path::Path;
use std::{collections::btree_map::Iter, fmt::Display};

use anyhow::{Context, Result};
use p2panda_rs::schema::{FieldName, SchemaDescription, SchemaId, SchemaName};
use serde::{Deserialize, Serialize};

use crate::utils::files;

/// Serializable format for definitions of one to many p2panda schemas.
///
/// ```toml
/// [event]
/// description = "An example schema"
///
/// [event.fields]
/// date = { type = "int" }
/// title = { type = "str" }
/// venue = { type = "relation", schema = { name = "venue" } }
///
/// [venue]
/// description = "Another schema"
///
/// [venue.fields]
/// name = { type = "str" }
/// ```
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SchemaFile(BTreeMap<SchemaName, SchemaDefinition>);

impl SchemaFile {
    /// Loads a .toml file from the given path and serialises its content into a new `SchemaFile`
    /// instance.
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self> {
        let data = files::read_file(&path)?;
        let schema_file: Self =
            toml::from_str(&data).with_context(|| "Invalid TOML syntax in schema file")?;
        Ok(schema_file)
    }

    /// Returns an iterator over all defined schemas.
    pub fn iter(&self) -> Iter<SchemaName, SchemaDefinition> {
        self.0.iter()
    }
}

/// Single schema definition with description and its fields.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SchemaDefinition {
    pub description: SchemaDescription,
    pub fields: SchemaFields,
}

/// Holds one to many schema field definitions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SchemaFields(BTreeMap<FieldName, SchemaField>);

impl SchemaFields {
    /// Returns a new empty instance of `SchemaFields`.
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }

    /// Returns the number of given fields.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Inserts a new field.
    pub fn insert(&mut self, field_name: &FieldName, field: &SchemaField) {
        self.0.insert(field_name.clone(), field.clone());
    }

    /// Returns an iterator over all fields.
    pub fn iter(&self) -> Iter<FieldName, SchemaField> {
        self.0.iter()
    }
}

/// Definition of a single schema field.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum SchemaField {
    /// This field is either a string, integer, float or boolean.
    Field {
        #[serde(rename = "type")]
        field_type: FieldType,
    },
    /// This field is either a (pinned) relation or relation list.
    Relation {
        #[serde(rename = "type")]
        field_type: RelationType,
        schema: RelationSchema,
    },
}

impl Display for SchemaField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let type_str = match self {
            SchemaField::Field { field_type } => match field_type {
                FieldType::Boolean => "bool",
                FieldType::Float => "float",
                FieldType::Integer => "int",
                FieldType::String => "str",
                FieldType::Bytes => "bytes",
            }
            .to_string(),
            SchemaField::Relation { field_type, schema } => {
                let name = match &schema.id {
                    RelationId::Name(name) => name.to_owned(),
                    RelationId::Id(id) => id.name(),
                };

                match field_type {
                    RelationType::Relation => format!("relation({})", name),
                    RelationType::RelationList => format!("relation_list({})", name),
                    RelationType::PinnedRelation => format!("pinned_relation({})", name),
                    RelationType::PinnedRelationList => format!("pinned_relation_list({})", name),
                }
            }
        };

        write!(f, "{}", type_str)
    }
}

/// Definition of field type.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum FieldType {
    #[serde(rename = "bool")]
    Boolean,
    #[serde(rename = "float")]
    Float,
    #[serde(rename = "int")]
    Integer,
    #[serde(rename = "str")]
    String,
    #[serde(rename = "bytes")]
    Bytes,
}

/// Definition of relation type.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum RelationType {
    Relation,
    RelationList,
    PinnedRelation,
    PinnedRelationList,
}

/// Definition of the schema used by a relation.
///
/// A schema can be either identified by its name or schema id while the schema definition itself
/// can either be in the same file, in an external file or remote git repository.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelationSchema {
    #[serde(flatten)]
    pub id: RelationId,
    #[serde(flatten)]
    pub external: Option<RelationSource>,
}

/// Identifier of schema.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase", deny_unknown_fields)]
pub enum RelationId {
    /// Schema id from schema defined in the same document or externally.
    Id(SchemaId),

    /// Name from schema defined in the same document.
    Name(SchemaName),
}

/// Definition of schema source.
///
/// If no external schema was defined we can assume the schema was defined in the same file.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase", deny_unknown_fields)]
pub enum RelationSource {
    /// Cloneable git repository URL from external machine.
    Git(String),

    /// File system path on local machine.
    Path(String),
}

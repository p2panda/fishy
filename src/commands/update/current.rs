// SPDX-License-Identifier: AGPL-3.0-or-later

use anyhow::{bail, Result};
use p2panda_rs::schema::{SchemaDescription, SchemaName};

use crate::schema_file::{SchemaFields, SchemaFile};

/// Extracts all schema definitions from user file and returns them as current schemas.
pub fn get_current_schemas(schema_file: &SchemaFile) -> Result<Vec<CurrentSchema>> {
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

/// Schema which was defined in the user's schema file.
#[derive(Clone, Debug)]
pub struct CurrentSchema {
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

// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashMap;

use anyhow::Result;
use comfy_table::modifiers::UTF8_ROUND_CORNERS;
use comfy_table::presets::UTF8_FULL;
use comfy_table::{Cell, Color, Table};
use console::style;
use p2panda_rs::identity::PublicKey;
use p2panda_rs::schema::{
    FieldName, FieldType as PandaFieldType, SchemaDescription, SchemaId, SchemaName,
};

use crate::schema_file::{
    FieldType, RelationId, RelationSchema, RelationType, SchemaField, SchemaFields,
};

use super::diff::FieldTypeDiff;
use super::executor::Plan;
use super::previous::PreviousSchemas;

/// Shows the execution plan to the user.
pub fn print_plan(
    plans: Vec<Plan>,
    previous_schemas: PreviousSchemas,
    public_key: PublicKey,
    show_only_diff: bool,
) -> Result<()> {
    if show_only_diff {
        println!(
            "The following changes ({}, {}, {}) will be applied:\n",
            style("add").green(),
            style("change").yellow(),
            style("remove").red()
        );
    }

    for plan in &plans {
        let schema_diff = plan.schema_diff();

        // Schema id
        let current_schema_id = plan.schema_id();
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
                    FieldTypeDiff::ExternalRelation(field_type, schema_id) => {
                        SchemaField::Relation {
                            field_type,
                            schema: RelationSchema {
                                id: RelationId::Id(schema_id),
                                // Even though this is an external relation we set this to `None`
                                // here as this field indicates that the schema definition came
                                // from an (external) git or file system path.
                                external: None,
                            },
                        }
                    }
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
                        PandaFieldType::Bytes => SchemaField::Field {
                            field_type: FieldType::Bytes,
                        },
                        PandaFieldType::Relation(schema_id) => SchemaField::Relation {
                            field_type: RelationType::Relation,
                            schema: RelationSchema {
                                id: RelationId::Id(schema_id.to_owned()),
                                external: None,
                            },
                        },
                        PandaFieldType::RelationList(schema_id) => SchemaField::Relation {
                            field_type: RelationType::RelationList,
                            schema: RelationSchema {
                                id: RelationId::Id(schema_id.to_owned()),
                                external: None,
                            },
                        },
                        PandaFieldType::PinnedRelation(schema_id) => SchemaField::Relation {
                            field_type: RelationType::PinnedRelation,
                            schema: RelationSchema {
                                id: RelationId::Id(schema_id.to_owned()),
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

        // Skip printing this schema if nothing has changed
        if show_only_diff {
            if let Some(previous_schema_id) = &previous_schema_id {
                if previous_schema_id.version() == current_schema_id.version() {
                    continue;
                }
            }
        }

        // Display schema id
        let color = match &previous_schema_id {
            Some(previous_schema_id) => {
                if previous_schema_id != &current_schema_id {
                    console::Color::Yellow
                } else {
                    console::Color::White
                }
            }
            None => console::Color::Green,
        };

        println!(
            "{}",
            style(format!("{current_schema_id}"))
                .bold()
                .underlined()
                .fg(color),
        );

        if let Some(previous_schema_id) = &previous_schema_id {
            if previous_schema_id.version() != current_schema_id.version() {
                println!("Previously: {previous_schema_id}");
            }
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

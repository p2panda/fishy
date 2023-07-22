// SPDX-License-Identifier: AGPL-3.0-or-later

use std::env;
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use dialoguer::Input;
use p2panda_rs::identity::KeyPair;
use p2panda_rs::schema::validate::validate_name;

use crate::constants::{LOCK_FILE_NAME, PRIVATE_KEY_FILE_NAME, SCHEMA_FILE_NAME};
use crate::utils::files::write_file;
use crate::utils::key_pair::write_key_pair;
use crate::utils::terminal::{print_title, print_variable};

/// Initialises all files for a new fishy project in a given folder.
pub fn init(target_dir: Option<PathBuf>, schema_name: Option<String>) -> Result<()> {
    print_title("Initialise a new fishy project");

    // Use current directory when none was given
    let target_dir = target_dir.unwrap_or(env::current_dir()?);
    print_variable("target_dir", target_dir.display());

    // Make sure everything is okay
    sanity_check(&target_dir)?;

    // Ask user about the schema name when none was given
    let schema_name = match schema_name {
        Some(name) => {
            print_variable("schema_name", &name);

            if !validate_name(&name) {
                bail!("'{name}' is not a valid p2panda schema name");
            }

            name
        }
        None => Input::new()
            .with_prompt("? Name of your schema")
            .validate_with(|input: &String| -> Result<()> {
                if !validate_name(input) {
                    bail!("This is not a valid p2panda schema name");
                }

                Ok(())
            })
            .interact()?,
    };

    init_secret_file(&target_dir)?;
    init_schema_file(&target_dir, &schema_name)?;

    Ok(())
}

/// Checks if everything is okay.
fn sanity_check(target_dir: &Path) -> Result<()> {
    // Check if target directory exists
    if !target_dir.exists() {
        bail!(
            "Given target directory '{}' does not exist",
            target_dir.display()
        );
    }

    // Check if files already exist
    [PRIVATE_KEY_FILE_NAME, SCHEMA_FILE_NAME, LOCK_FILE_NAME]
        .iter()
        .try_for_each(|file_name| {
            if Path::new(file_name).exists() {
                bail!("Found an already existing '{file_name}' file")
            }

            Ok(())
        })?;

    Ok(())
}

/// Creates a file with a newly generated ed25519 private key inside.
fn init_secret_file(target_dir: &Path) -> Result<()> {
    let key_pair = KeyPair::new();
    write_key_pair(target_dir, &key_pair)?;
    Ok(())
}

/// Creates a new schema file from a small template.
fn init_schema_file(target_dir: &Path, schema_name: &str) -> Result<()> {
    let schema_path = {
        let mut path = target_dir.to_path_buf();
        path.push(SCHEMA_FILE_NAME);
        path
    };

    write_file(
        &schema_path,
        &format!(
            r#"[{schema_name}]
description = "Write about your schema here"

[{schema_name}.fields]
some_field = {{ type = "str" }}"#
        ),
    )?;

    Ok(())
}

// SPDX-License-Identifier: AGPL-3.0-or-later

use std::env;
use std::path::PathBuf;

use anyhow::Result;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Input;

/// Initialises all files for a new fishy project in a given folder.
pub fn init(target_dir: Option<PathBuf>, schema_name: Option<String>) -> Result<()> {
    // Use current directory when none was given
    let target_dir = target_dir.unwrap_or(env::current_dir()?);

    // Ask user about the schema name when none was given
    let schema_name = match schema_name {
        Some(name) => name,
        None => Input::<String>::with_theme(&ColorfulTheme::default())
            .with_prompt("What is the name of your first schema?")
            .interact_text()?,
    };

    Ok(())
}

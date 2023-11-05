// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::PathBuf;

use anyhow::Result;

use crate::lock_file::{Commit, LockFile};
use crate::utils::files::{self};

/// Write commits to lock file.
pub fn write_to_lock_file(
    mut new_commits: Vec<Commit>,
    lock_file: LockFile,
    lock_path: PathBuf,
) -> Result<()> {
    // Add new commits to the existing ones
    let applied_commits_count = new_commits.len();

    let mut commits: Vec<Commit> = lock_file.commits();
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

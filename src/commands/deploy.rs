// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use fishy::lock_file::LockFile;
use fishy::Client;
use indicatif::ProgressBar;

use crate::utils::terminal::{print_title, print_variable};
use fishy::utils::files::absolute_path;

/// Deploy created schemas on a node.
pub async fn deploy(lock_path: PathBuf, endpoint: &str) -> Result<()> {
    print_title("Deploy created schemas on a node");
    print_variable("lock_path", absolute_path(&lock_path)?.display());
    print_variable("endpoint", endpoint);
    println!();

    let lock_file = LockFile::from_path(&lock_path).context(format!(
        "Try reading lock file from path '{}'",
        lock_path.display()
    ))?;

    let commits = lock_file.commits.unwrap_or(Vec::new());
    if commits.is_empty() {
        bail!("No data given to deploy to node. Please run `update` command first.");
    }

    // Count how many commits we needed to deploy
    let mut skipped = 0;
    let total = commits.len();
    let progress = ProgressBar::new(total as u64);

    // Publish commits on external node via GraphQL
    let client = Client::new(endpoint);

    for commit in commits {
        let published = client.publish(commit).await?;

        if !published {
            skipped += 1;
        }

        progress.inc(1);
    }

    println!();

    if total == skipped {
        println!("Node is already up-to-date with latest schema version. No deployment required.")
    } else {
        println!(
            "Successfully deployed {} commits on node (ignored {}).",
            total - skipped,
            skipped,
        );
    }

    Ok(())
}

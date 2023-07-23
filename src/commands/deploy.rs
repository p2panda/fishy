// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::PathBuf;

use anyhow::{anyhow, bail, Result};
use gql_client::Client;
use p2panda_rs::entry::decode::decode_entry;
use p2panda_rs::entry::traits::AsEntry;
use p2panda_rs::entry::{LogId, SeqNum};
use p2panda_rs::hash::Hash;
use serde::Deserialize;

use crate::lock_file::LockFile;
use crate::utils::files::absolute_path;
use crate::utils::terminal::{print_title, print_variable};

/// Deploy created schemas on a node.
pub async fn deploy(lock_path: PathBuf, endpoint: &str) -> Result<()> {
    print_title("Deploy created schemas on a node");
    print_variable("lock_path", absolute_path(&lock_path)?.display());
    print_variable("endpoint", endpoint);
    println!();

    let lock_file = LockFile::from_path(&lock_path)?;

    let commits = lock_file.commits.unwrap_or(Vec::new());
    if commits.is_empty() {
        bail!("No data given to deploy to node. Please run `update` command first.");
    }

    // Count how many commits we needed to deploy
    let mut skipped = 0;
    let total = commits.len();

    // Publish commits on external node via GraphQL
    let client = Client::new(endpoint);

    for commit in commits {
        let entry = decode_entry(&commit.entry).unwrap();

        let query = format!(
            r#"
            {{
                nextArgs(publicKey: "{}", viewId: "{}") {{
                    logId
                    seqNum
                    skiplink
                    backlink
                }}
            }}
            "#,
            entry.public_key(),
            commit.entry_hash,
        );

        let response = client.query_unwrap::<NextArgsResponse>(&query).await;

        if let Ok(result) = response {
            let args = result.next_args;

            if entry.log_id() != &args.log_id || Some(commit.entry_hash) != args.backlink {
                bail!("Inconsistency between local commits and node detected");
            }

            // Check if node already knows about this commit
            if entry.seq_num() < &args.seq_num {
                skipped += 1;

                // Skip this one
                continue;
            }
        }

        let query = format!(
            r#"
            mutation Publish {{
                publish(entry: "{}", operation: "{}") {{
                    logId
                    seqNum
                    skiplink
                    backlink
                }}
            }}
            "#,
            commit.entry, commit.operation
        );

        client
            .query_unwrap::<PublishResponse>(&query)
            .await
            .map_err(|err| anyhow!("GraphQL request to node failed: {err}"))?;
    }

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

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct NextArguments {
    log_id: LogId,
    seq_num: SeqNum,
    skiplink: Option<Hash>,
    backlink: Option<Hash>,
}

/// GraphQL response for `nextArgs` query.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct NextArgsResponse {
    next_args: NextArguments,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct PublishResponse {
    publish: NextArguments,
}

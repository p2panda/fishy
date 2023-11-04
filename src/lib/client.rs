// SPDX-License-Identifier: AGPL-3.0-or-later

use anyhow::{anyhow, bail, Result};
use gql_client::Client as GQLClient;
use p2panda_rs::entry::decode::decode_entry;
use p2panda_rs::entry::traits::AsEntry;
use p2panda_rs::entry::{LogId, SeqNum};
use p2panda_rs::hash::Hash;
use serde::Deserialize;

use crate::lock_file::Commit;

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

pub struct Client(GQLClient);

impl Client {
    pub fn new(endpoint: &str) -> Self {
        Self(GQLClient::new(endpoint))
    }

    pub async fn publish(&self, commit: Commit) -> Result<bool> {
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

        let response = self.0.query_unwrap::<NextArgsResponse>(&query).await;

        if let Ok(result) = response {
            let args = result.next_args;

            if entry.log_id() != &args.log_id {
                bail!("Inconsistency between local commits and node detected");
            }

            // Check if node already knows about this commit
            if entry.seq_num() < &args.seq_num {
                // Skip this one
                return Ok(false);
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

        self.0
            .query_unwrap::<PublishResponse>(&query)
            .await
            .map_err(|err| anyhow!("GraphQL request to node failed: {err}"))?;

        Ok(true)
    }
}

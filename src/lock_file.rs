// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::Path;

use anyhow::{Context, Result};
use p2panda_rs::entry::traits::AsEncodedEntry;
use p2panda_rs::entry::EncodedEntry;
use p2panda_rs::hash::Hash;
use p2panda_rs::operation::EncodedOperation;
use serde::{Deserialize, Serialize};

use crate::utils::files;

/// Serializable format holding encoded and signed p2panda operations and entries.
///
/// ```toml
/// version = 1
///
/// [[commits]]
/// entry_hash = "..."
/// entry = "..."
/// operation = "..."
///
/// [[commits]]
/// entry_hash = "..."
/// entry = "..."
/// operation = "..."
///
/// # ...
/// ```
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LockFile {
    pub version: LockFileVersion,
    pub commits: Option<Vec<Commit>>,
}

impl LockFile {
    /// Returns a new, empty instance of `LockFile`.
    pub fn new(commits: &[Commit]) -> Self {
        Self {
            version: LockFileVersion::V1,
            commits: Some(commits.to_vec()),
        }
    }

    /// Loads a .toml file from the given path and serialises its content into a new `LockFile`
    /// instance.
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self> {
        let data = files::read_file(&path)?;
        let schema_file: Self =
            toml::from_str(&data).with_context(|| "Invalid TOML syntax in lock file")?;
        Ok(schema_file)
    }
}

/// Known versions of lock file format.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum LockFileVersion {
    V1 = 1,
}

/// Single commit with encoded entry and operation pair.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Commit {
    /// Hash of the entry.
    pub entry_hash: Hash,

    /// Encoded and signed p2panda entry.
    pub entry: EncodedEntry,

    /// Encoded p2panda operation.
    pub operation: EncodedOperation,
}

impl Commit {
    /// Returns a new instance of `Commit`.
    pub fn new(entry: &EncodedEntry, operation: &EncodedOperation) -> Self {
        Self {
            entry_hash: entry.hash(),
            entry: entry.clone(),
            operation: operation.clone(),
        }
    }
}

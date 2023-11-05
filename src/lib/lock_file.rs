// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::Path;

use anyhow::{Context, Result};
use p2panda_rs::entry::traits::AsEncodedEntry;
use p2panda_rs::entry::EncodedEntry;
use p2panda_rs::hash::Hash;
use p2panda_rs::operation::EncodedOperation;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

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
    version: LockFileVersion,
    commits: Option<Vec<Commit>>,
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

    /// Version of the lockfile.
    pub fn version(&self) -> &LockFileVersion {
        &self.version
    }

    /// Commits contained in the lockfile.
    ///
    /// Returns an empty vec if no `commits` were defined in the .toml file yet.
    pub fn commits(&self) -> Vec<Commit> {
        match &self.commits {
            Some(commits) => commits.clone(),
            None => Vec::new(),
        }
    }
}

/// Known versions of lock file format.
#[derive(Debug, Clone)]
pub enum LockFileVersion {
    V1,
}

impl LockFileVersion {
    /// Returns the operation version encoded as u64.
    pub fn as_u64(&self) -> u64 {
        match self {
            LockFileVersion::V1 => 1,
        }
    }
}

impl Serialize for LockFileVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(self.as_u64())
    }
}

impl<'de> Deserialize<'de> for LockFileVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let version = u64::deserialize(deserializer)?;

        match version {
            1 => Ok(LockFileVersion::V1),
            _ => Err(serde::de::Error::custom(format!(
                "unsupported lock file version {}",
                version
            ))),
        }
    }
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

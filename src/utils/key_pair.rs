// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fs::{File, Permissions};
use std::os::unix::prelude::PermissionsExt;
use std::path::Path;

use anyhow::Result;
use p2panda_rs::identity::KeyPair;

use crate::constants::PRIVATE_KEY_FILE_NAME;
use crate::utils::files;

/// Writes a hex-encoded ed25519 private key string into a file and sets permission to 0600.
pub fn write_key_pair(target_dir: &Path, key_pair: &KeyPair) -> Result<()> {
    let mut path = target_dir.to_path_buf();
    path.push(PRIVATE_KEY_FILE_NAME);

    let private_key_str = hex::encode(key_pair.private_key());
    files::write_file(&path, &private_key_str)?;

    let file = File::open(&path)?;
    file.set_permissions(Permissions::from_mode(0o600))?;

    Ok(())
}

/// Reads a hex-encoded ed25519 private key string from a file and derives key pair from it.
pub fn read_key_pair(target_dir: &Path) -> Result<KeyPair> {
    let mut path = target_dir.to_path_buf();
    path.push(PRIVATE_KEY_FILE_NAME);

    let private_key_str = files::read_file(&path)?;
    let key_pair = KeyPair::from_private_key_str(&private_key_str)?;

    Ok(key_pair)
}

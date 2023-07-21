// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;

use anyhow::Result;

pub fn write_file(path: &PathBuf, content: &str) -> Result<()> {
    let mut file = File::create(path)?;
    file.write_all(content.as_bytes())?;
    Ok(())
}

pub fn read_file(path: &PathBuf) -> Result<String> {
    let mut buf = String::new();
    let mut file = File::open(path)?;
    file.read_to_string(&mut buf)?;
    Ok(buf)
}

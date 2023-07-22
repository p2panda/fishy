// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fmt::Display;

use console::style;

pub fn print_title(title: &str) {
    println!("{} {title}", style("fishy:").bold().underlined());
}

pub fn print_variable(name: &str, value: impl Display) {
    println!("{name}: {value}");
}

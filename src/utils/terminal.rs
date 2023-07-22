// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fmt::Display;

use console::style;

/// Prints a nice looking main title into the terminal.
pub fn print_title(title: &str) {
    println!("{} {title}\n", style("fishy:").bold().underlined());
}

/// Prints a nice looking variable value into the terminal.
pub fn print_variable(name: &str, value: impl Display) {
    println!("- {name}: {}", style(value).dim());
}

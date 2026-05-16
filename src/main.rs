/* ipsort: versitile ip address sorting tool
 * Copyright 2026 Mario Finelli
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

use clap::{CommandFactory, Parser};
use clap_complete::{Shell, generate};
use ipsort::blocks::{deduplicate_blocks, sort_blocks, sort_inline};
use ipsort::classify::{ClassifiedLine, classify_line};
use ipsort::output::{IpsOnlyMode, OutputOptions, render_line};
use ipsort::sort::SortOptions;
use std::io::{self, BufRead, IsTerminal};

#[derive(Parser)]
#[command(
    name = "ipsort",
    about = "Sort IP addresses and CIDRs numerically",
    long_about = "Sort IP addresses and CIDRs by their actual numeric value.\n\
                  Accepts input from stdin or positional arguments.\n\
                  Use '-' to read explicitly from stdin.",
    version
)]
struct Cli {
    /// Input CIDRs. Pass one or more addresses, a comma/space-separated list,
    /// or '-' to read from stdin.
    addresses: Vec<String>,

    /// Reverse the sort order
    #[arg(short, long)]
    reverse: bool,

    /// Sort IPv6 addresses before IPv4
    #[arg(long)]
    ipv6_first: bool,

    /// Remove duplicate addresses (compared by normalized CIDR)
    #[arg(short, long)]
    unique: bool,

    /// Reorder all IP tokens freely across the entire input
    #[arg(long)]
    inline: bool,

    /// Emit canonical network strings (clears host bits, adds /32 or /128)
    #[arg(long)]
    normalize: bool,

    /// Strip all non-IP content and emit one bare address per line
    #[arg(long, conflicts_with = "ips_only_with_structure")]
    ips_only: bool,

    /// Strip decoration but preserve non-IP lines as block separators
    #[arg(long, conflicts_with = "ips_only")]
    ips_only_with_structure: bool,

    /// Generate shell completions for the given shell and print to stdout
    #[arg(long, value_name = "SHELL", hide = true)]
    generate_completions: Option<Shell>,
}

fn main() {
    let cli = Cli::parse();

    // Handle completion generation before anything else
    if let Some(shell) = cli.generate_completions {
        let mut cmd = Cli::command();
        generate(shell, &mut cmd, "ipsort", &mut io::stdout());
        return;
    }

    // Validate: stdin + positional args are mutually exclusive
    let has_args = !cli.addresses.is_empty()
        && !(cli.addresses.len() == 1 && cli.addresses[0] == "-");

    // Read input into lines
    let lines: Vec<String> = if has_args {
        cli.addresses.clone()
    } else {
        let stdin = io::stdin();
        let is_tty = stdin.is_terminal();
        let lines: Vec<String> = stdin
            .lock()
            .lines()
            .map(|l| l.expect("failed to read from stdin"))
            .collect();
        if !is_tty && lines.is_empty() {
            eprintln!("ipsort: no input provided");
            std::process::exit(1);
        }
        lines
    };

    let sort_opts = SortOptions {
        ipv6_first: cli.ipv6_first,
        reverse: cli.reverse,
    };

    let out_opts = OutputOptions {
        normalize: cli.normalize,
        ips_only: if cli.ips_only {
            IpsOnlyMode::Flat
        } else if cli.ips_only_with_structure {
            IpsOnlyMode::WithStructure
        } else {
            IpsOnlyMode::Off
        },
    };

    // Classify lines
    let classified: Vec<ClassifiedLine> = lines
        .iter()
        .map(|line| classify_line(line, &sort_opts))
        .collect();

    // Check that at least one IP was found
    let has_any_ip = classified
        .iter()
        .any(|l| matches!(l, ClassifiedLine::HasIp { .. }));
    if !has_any_ip {
        eprintln!("ipsort: no IP addresses found in input");
        std::process::exit(1);
    }

    // Sort and deduplicate
    let sorted = if cli.inline {
        // --inline: sort all IPs globally across the entire input,
        // dedup is folded into sort_inline when --unique is set
        sort_inline(classified, &sort_opts, cli.unique)
    } else {
        let sorted = sort_blocks(classified, &sort_opts);
        if cli.unique {
            match deduplicate_blocks(sorted, &out_opts.ips_only) {
                Ok(lines) => lines,
                Err(e) => {
                    eprintln!(
                        "ipsort: --unique: duplicate IP {} found on multi-IP line {:?}",
                        e.duplicate_ip, e.line
                    );
                    eprintln!(
                        "ipsort: --unique: cannot determine which IP to remove; clean up input and try again"
                    );
                    std::process::exit(1);
                }
            }
        } else {
            sorted
        }
    };

    // Render and print
    for line in &sorted {
        if let ClassifiedLine::HasIp { warnings, .. } = line {
            for w in warnings {
                eprintln!("{w}");
            }
        }
        for rendered in render_line(line, &out_opts, &sort_opts) {
            println!("{rendered}");
        }
    }
}

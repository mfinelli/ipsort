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
use ipsort::blocks::{
    aggregate_blocks, deduplicate_blocks, sort_blocks, sort_inline,
};
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
    #[arg(short, long)]
    inline: bool,

    /// Emit canonical network strings (clears host bits, adds /32 or /128)
    #[arg(short, long)]
    normalize: bool,

    /// Strip all non-IP content and emit one bare address per line
    #[arg(long, conflicts_with = "ips_only_with_structure")]
    ips_only: bool,

    /// Strip decoration but preserve non-IP lines as block separators
    #[arg(long, conflicts_with = "ips_only")]
    ips_only_with_structure: bool,

    /// Merge adjacent CIDRs into their minimal supernet representation
    #[arg(short, long)]
    aggregate: bool,

    /// Check whether input is already sorted; exit 0 if so, 1 if not. No
    /// output is printed. Reports the first out-of-order line to stderr on
    /// failure.
    #[arg(short, long)]
    check: bool,

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
    let has_args = !(cli.addresses.is_empty()
        || cli.addresses.len() == 1 && cli.addresses[0] == "-");

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

    // Save a clone of classified lines for --check comparison before sorting
    let classified_for_check: Vec<ClassifiedLine> = if cli.check {
        classified.clone()
    } else {
        vec![]
    };

    // Sort, aggregate, and deduplicate
    let sorted = if cli.inline {
        sort_inline(classified, &sort_opts, cli.unique)
    } else {
        let sorted = sort_blocks(classified, &sort_opts);

        let sorted = if cli.aggregate {
            match aggregate_blocks(sorted, &sort_opts) {
                Ok(lines) => lines,
                Err(e) => {
                    eprintln!(
                        "ipsort: --aggregate: multi-IP line {:?} would be involved in aggregation to {}",
                        e.line, e.aggregate
                    );
                    eprintln!(
                        "ipsort: --aggregate: cannot determine which IP to replace; clean up input and try again"
                    );
                    std::process::exit(1);
                }
            }
        } else {
            sorted
        };

        if cli.unique && !cli.aggregate {
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

    // Render sorted output
    let sorted_rendered: Vec<String> = sorted
        .iter()
        .flat_map(|line| render_line(line, &out_opts, &sort_opts))
        .collect();

    if cli.check {
        // Render the unsorted classified lines through the same output options
        // to get a comparable stream (same decoration stripping, normalization,
        // etc.)
        let unsorted_rendered: Vec<String> = classified_for_check
            .iter()
            .flat_map(|line| render_line(line, &out_opts, &sort_opts))
            .collect();

        // Compare the two streams
        for (i, (unsorted, sorted)) in unsorted_rendered
            .iter()
            .zip(sorted_rendered.iter())
            .enumerate()
        {
            if unsorted != sorted {
                eprintln!(
                    "ipsort: check failed: line {} is out of order: {:?}",
                    i + 1,
                    unsorted
                );
                std::process::exit(1);
            }
        }
        // Also check length difference (e.g. --unique or --aggregate removed
        // lines)
        if unsorted_rendered.len() != sorted_rendered.len() {
            eprintln!(
                "ipsort: check failed: input has {} lines but sorted output has {}",
                unsorted_rendered.len(),
                sorted_rendered.len()
            );
            std::process::exit(1);
        }
        // All good
        std::process::exit(0);
    }

    // Normal output
    for (line, rendered_lines) in sorted.iter().zip(
        sorted
            .iter()
            .map(|line| render_line(line, &out_opts, &sort_opts)),
    ) {
        if let ClassifiedLine::HasIp { warnings, .. } = line {
            for w in warnings {
                eprintln!("{w}");
            }
        }
        for rendered in rendered_lines {
            println!("{rendered}");
        }
    }
}

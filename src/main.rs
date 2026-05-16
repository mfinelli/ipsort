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

use blocks::sort_blocks;
use classify::{ClassifiedLine, Span, classify_line};
use ipnet::IpNet;
use ipsort::{blocks, classify, parse, sort};
use sort::SortOptions;
use std::str::FromStr;

fn main() {
    let opts = SortOptions::default();

    let tokens = vec![
        "192.168.1.50/24", // host bits set
        "10.0.0.0/8",
        "172.16.0.5",  // bare IP, promoted to /32
        "10.0.0.0/24", // more specific than 10.0.0.0/8
        "192.168.1.0/24",
        "not-an-ip",
        "\"172.16.10.0/24\",", // punctuation-wrapped
        "10.0.0.1",            // bare IP
    ];

    for token in &tokens {
        let parsed = parse::parse_token(token);
        match &parsed {
            parse::ParsedToken::ValidCidr {
                original,
                network,
                had_host_bits,
            } => {
                if *had_host_bits {
                    eprintln!(
                        "warning: host bits set in {original:?}, treating as {network}"
                    );
                }
                println!("ValidCidr   original={original:?} network={network}");
            }
            parse::ParsedToken::BareIp { original, network } => {
                println!(
                    "BareIp      original={original:?} promoted={network}"
                );
            }
            parse::ParsedToken::NotAnIp(s) => {
                println!("NotAnIp     {s:?}");
            }
        }
    }

    let mut networks: Vec<IpNet> = vec![
        "192.168.1.0/24",
        "10.0.0.0/24",
        "172.16.0.0/12",
        "10.0.0.0/8",
        "192.168.0.0/16",
        "10.0.0.1", // bare ip gets dropped
        "172.16.1.0/24",
        "2001:db8::/32",
        "::1/128",
        "2001:db8:1::/48",
    ]
    .iter()
    .filter_map(|s| IpNet::from_str(s).ok())
    .collect();

    networks.sort_by(|a, b| sort::compare(a, b, &opts));

    for net in &networks {
        println!("{net}");
    }

    let input = vec![
        "# plain CIDRs",
        "192.168.1.0/24",
        "10.0.0.0/8",
        "",
        "# yaml list",
        "- 192.168.2.0/24",
        "- 10.10.0.0/16",
        "",
        "# multi-ip lines",
        "network: 172.16.5.0/24 172.16.1.0/24",
        "\"10.0.1.0/24\", \"10.0.2.0/24\"",
        "",
        "# host bits set",
        "10.0.0.5/24",
        "",
        "# non-ip content",
        "# just a comment",
        "---",
    ];

    for line in &input {
        let classified = classify_line(line, &opts);
        match &classified {
            ClassifiedLine::HasIp {
                spans,
                sort_key,
                warnings,
            } => {
                for w in warnings {
                    eprintln!("{w}");
                }
                print!("HasIp sort_key={sort_key:20}  spans=[");
                for span in spans {
                    match span {
                        Span::Ip(t) => print!(" Ip({:?})", t.original()),
                        Span::NonIp(s) => print!(" NonIp({s:?})"),
                    }
                }
                println!(" ]");
            }
            ClassifiedLine::NoIp(s) => {
                println!("NoIp  {s:?}");
            }
        }
    }

    let input = vec![
        "# group one",
        "192.168.1.0/24",
        "10.0.0.0/8",
        "172.16.0.0/12",
        "",
        "# group two",
        "- 192.168.2.0/24",
        "- 10.10.0.0/16",
        "",
        "# group three (mixed content lines)",
        "network: 172.16.5.0/24 172.16.1.0/24",
        "\"10.0.1.0/24\", \"10.0.2.0/24\"",
    ];

    let classified: Vec<ClassifiedLine> = input
        .iter()
        .map(|line| classify_line(line, &opts))
        .collect();

    let sorted = sort_blocks(classified, &opts);

    for line in &sorted {
        match line {
            ClassifiedLine::HasIp {
                spans,
                sort_key,
                warnings,
                ..
            } => {
                for w in warnings {
                    eprintln!("{w}");
                }
                print!("HasIp sort_key={sort_key:20}  spans=[");
                for span in spans {
                    match span {
                        Span::Ip(t) => print!(" Ip({:?})", t.original()),
                        Span::NonIp(s) => print!(" NonIp({s:?})"),
                    }
                }
                println!(" ]");
            }
            ClassifiedLine::NoIp(s) => {
                println!("{s}");
            }
        }
    }
}

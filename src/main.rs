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

use ipnet::IpNet;
use ipsort::{parse, sort};
use std::str::FromStr;

fn main() {
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

    let opts = sort::SortOptions::default();
    networks.sort_by(|a, b| sort::compare(a, b, &opts));

    for net in &networks {
        println!("{net}");
    }
}

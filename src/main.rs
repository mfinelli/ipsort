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
use classify::{ClassifiedLine, classify_line};
use ipsort::{blocks, classify, output, sort};
use output::{OutputOptions, render_line};
use sort::SortOptions;

fn main() {
    let input = vec![
        "# group one: plain CIDRs",
        "192.168.1.0/24",
        "10.0.0.0/8",
        "172.16.0.0/12",
        "",
        "# group two: yaml list",
        "- 192.168.2.0/24",
        "- 10.10.0.0/16",
        "",
        "# group three: multi-ip lines",
        "network: 172.16.5.0/24 172.16.1.0/24",
        "\"192.168.0.0/16\", \"10.0.0.0/8\"",
        "",
        "# group four: host bits and bare IPs",
        "10.0.0.5/24",
        "192.168.1.1",
    ];

    let sort_opts = SortOptions::default();
    let out_opts = OutputOptions::default();
    // let out_opts = OutputOptions { normalize: true, ..Default::default() };
    // let out_opts = OutputOptions { ips_only: true, ..Default::default() };

    let classified: Vec<ClassifiedLine> = input
        .iter()
        .map(|line| classify_line(line, &sort_opts))
        .collect();

    let sorted = sort_blocks(classified, &sort_opts);

    for line in &sorted {
        // Emit any warnings to stderr before rendering
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

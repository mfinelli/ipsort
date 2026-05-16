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

//! Block-level sorting of classified input lines.
//!
//! This module operates on the output of [`crate::classify`], grouping
//! [`ClassifiedLine::HasIp`] lines into contiguous blocks separated by
//! [`ClassifiedLine::NoIp`] lines, sorting each block independently, and
//! reassembling the result.
//!
//! # Block model
//! A block is a contiguous run of [`ClassifiedLine::HasIp`] lines. Blocks are
//! delimited by [`ClassifiedLine::NoIp`] lines (empty lines, comments, or any
//! line with no recognizable IP address), which act as anchors and are
//! preserved in their original positions in the output.
//!
//! For example, given:
//!
//! ```text
//! # group one
//! 192.168.1.0/24
//! 10.0.0.0/8
//!
//! # group two
//! 172.16.2.0/24
//! 172.16.1.0/24
//! ```
//!
//! The two IP groups are sorted independently, with the comment and blank line
//! staying in place between them.
//!
//! # Error handling
//! `sort_blocks` is a pure transformation and does not check whether any IP
//! addresses were found. The caller is responsible for erroring if the input
//! contained no IP addresses at all.

use crate::classify::ClassifiedLine;
use crate::sort::{SortOptions, compare};

/// Sort [`ClassifiedLine::HasIp`] lines within each block, preserving
/// [`ClassifiedLine::NoIp`] lines as block separators in their original
/// positions.
///
/// Lines are sorted by their `sort_key` using [`compare`] with the provided
/// [`SortOptions`]. Each contiguous run of `HasIp` lines is sorted
/// independently.
///
/// If the input contains no `HasIp` lines, the input is returned unchanged.
/// The caller should check for this case and error if appropriate.
pub fn sort_blocks(
    lines: Vec<ClassifiedLine>,
    opts: &SortOptions,
) -> Vec<ClassifiedLine> {
    let mut output: Vec<ClassifiedLine> = Vec::with_capacity(lines.len());
    let mut buffer: Vec<ClassifiedLine> = Vec::new();

    for line in lines {
        match line {
            ClassifiedLine::HasIp { .. } => {
                buffer.push(line);
            }
            ClassifiedLine::NoIp(_) => {
                flush_buffer(&mut buffer, &mut output, opts);
                output.push(line);
            }
        }
    }

    // Flush any remaining HasIp lines at end of input
    flush_buffer(&mut buffer, &mut output, opts);

    output
}

/// Sort the buffer of [`ClassifiedLine::HasIp`] lines and drain it into
/// output. Does nothing if the buffer is empty.
fn flush_buffer(
    buffer: &mut Vec<ClassifiedLine>,
    output: &mut Vec<ClassifiedLine>,
    opts: &SortOptions,
) {
    if buffer.is_empty() {
        return;
    }
    buffer.sort_by(|a, b| {
        let a_key = match a {
            ClassifiedLine::HasIp { sort_key, .. } => sort_key,
            ClassifiedLine::NoIp(_) => unreachable!(),
        };
        let b_key = match b {
            ClassifiedLine::HasIp { sort_key, .. } => sort_key,
            ClassifiedLine::NoIp(_) => unreachable!(),
        };
        compare(a_key, b_key, opts)
    });
    output.extend(buffer.drain(..));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classify::classify_line;
    use crate::sort::SortOptions;
    use ipnet::IpNet;
    use std::str::FromStr;

    fn opts() -> SortOptions {
        SortOptions::default()
    }

    fn classify(line: &str) -> ClassifiedLine {
        classify_line(line, &opts())
    }

    fn sort_key(line: &ClassifiedLine) -> IpNet {
        match line {
            ClassifiedLine::HasIp { sort_key, .. } => *sort_key,
            ClassifiedLine::NoIp(s) => {
                panic!("expected HasIp, got NoIp({s:?})")
            }
        }
    }

    fn original(line: &ClassifiedLine) -> String {
        match line {
            ClassifiedLine::HasIp { spans, .. } => spans
                .iter()
                .map(|s| match s {
                    crate::classify::Span::Ip(t) => t.original(),
                    crate::classify::Span::NonIp(s) => s.as_str(),
                })
                .collect(),
            ClassifiedLine::NoIp(s) => s.clone(),
        }
    }

    fn net(s: &str) -> IpNet {
        IpNet::from_str(s).unwrap()
    }

    #[test]
    fn test_single_block_sorted() {
        let lines = vec![
            classify("192.168.0.0/16"),
            classify("10.0.0.0/8"),
            classify("172.16.0.0/12"),
        ];
        let result = sort_blocks(lines, &opts());
        assert_eq!(sort_key(&result[0]), net("10.0.0.0/8"));
        assert_eq!(sort_key(&result[1]), net("172.16.0.0/12"));
        assert_eq!(sort_key(&result[2]), net("192.168.0.0/16"));
    }

    #[test]
    fn test_already_sorted_unchanged() {
        let lines = vec![
            classify("10.0.0.0/8"),
            classify("172.16.0.0/12"),
            classify("192.168.0.0/16"),
        ];
        let result = sort_blocks(lines, &opts());
        assert_eq!(sort_key(&result[0]), net("10.0.0.0/8"));
        assert_eq!(sort_key(&result[1]), net("172.16.0.0/12"));
        assert_eq!(sort_key(&result[2]), net("192.168.0.0/16"));
    }

    #[test]
    fn test_single_line_unchanged() {
        let lines = vec![classify("10.0.0.0/8")];
        let result = sort_blocks(lines, &opts());
        assert_eq!(result.len(), 1);
        assert_eq!(sort_key(&result[0]), net("10.0.0.0/8"));
    }

    #[test]
    fn test_two_blocks_sorted_independently() {
        let lines = vec![
            classify("192.168.0.0/16"),
            classify("10.0.0.0/8"),
            classify(""), // separator
            classify("172.16.2.0/24"),
            classify("172.16.1.0/24"),
        ];
        let result = sort_blocks(lines, &opts());

        // First block
        assert_eq!(sort_key(&result[0]), net("10.0.0.0/8"));
        assert_eq!(sort_key(&result[1]), net("192.168.0.0/16"));
        // Separator
        assert!(matches!(&result[2], ClassifiedLine::NoIp(_)));
        // Second block
        assert_eq!(sort_key(&result[3]), net("172.16.1.0/24"));
        assert_eq!(sort_key(&result[4]), net("172.16.2.0/24"));
    }

    #[test]
    fn test_separator_preserved_in_position() {
        let lines = vec![
            classify("192.168.0.0/16"),
            classify("# comment"),
            classify("10.0.0.0/8"),
        ];
        let result = sort_blocks(lines, &opts());
        assert!(matches!(&result[1], ClassifiedLine::NoIp(_)));
        assert_eq!(original(&result[1]), "# comment");
    }

    #[test]
    fn test_multiple_consecutive_separators_preserved() {
        let lines = vec![
            classify("192.168.0.0/16"),
            classify(""),
            classify("# comment"),
            classify(""),
            classify("10.0.0.0/8"),
        ];
        let result = sort_blocks(lines, &opts());
        assert_eq!(result.len(), 5);
        assert!(matches!(&result[1], ClassifiedLine::NoIp(_)));
        assert!(matches!(&result[2], ClassifiedLine::NoIp(_)));
        assert!(matches!(&result[3], ClassifiedLine::NoIp(_)));
    }

    #[test]
    fn test_leading_separator_preserved() {
        let lines = vec![
            classify("# header"),
            classify("192.168.0.0/16"),
            classify("10.0.0.0/8"),
        ];
        let result = sort_blocks(lines, &opts());
        assert!(matches!(&result[0], ClassifiedLine::NoIp(_)));
        assert_eq!(original(&result[0]), "# header");
        assert_eq!(sort_key(&result[1]), net("10.0.0.0/8"));
        assert_eq!(sort_key(&result[2]), net("192.168.0.0/16"));
    }

    #[test]
    fn test_trailing_separator_preserved() {
        let lines = vec![
            classify("192.168.0.0/16"),
            classify("10.0.0.0/8"),
            classify("# footer"),
        ];
        let result = sort_blocks(lines, &opts());
        assert_eq!(sort_key(&result[0]), net("10.0.0.0/8"));
        assert_eq!(sort_key(&result[1]), net("192.168.0.0/16"));
        assert!(matches!(&result[2], ClassifiedLine::NoIp(_)));
        assert_eq!(original(&result[2]), "# footer");
    }

    #[test]
    fn test_empty_input() {
        let result = sort_blocks(vec![], &opts());
        assert!(result.is_empty());
    }

    #[test]
    fn test_all_no_ip() {
        let lines =
            vec![classify("# just comments"), classify(""), classify("---")];
        let result = sort_blocks(lines, &opts());
        assert_eq!(result.len(), 3);
        assert!(result.iter().all(|l| matches!(l, ClassifiedLine::NoIp(_))));
    }

    #[test]
    fn test_output_length_preserved() {
        let lines = vec![
            classify("192.168.0.0/16"),
            classify("# comment"),
            classify("10.0.0.0/8"),
            classify("172.16.0.0/12"),
        ];
        let len = lines.len();
        let result = sort_blocks(lines, &opts());
        assert_eq!(result.len(), len);
    }

    #[test]
    fn test_same_network_shorter_prefix_first() {
        let lines = vec![
            classify("10.0.0.0/24"),
            classify("10.0.0.0/8"),
            classify("10.0.0.0/16"),
        ];
        let result = sort_blocks(lines, &opts());
        assert_eq!(sort_key(&result[0]), net("10.0.0.0/8"));
        assert_eq!(sort_key(&result[1]), net("10.0.0.0/16"));
        assert_eq!(sort_key(&result[2]), net("10.0.0.0/24"));
    }

    #[test]
    fn test_ipv6_block_sorted() {
        let lines =
            vec![classify("2001:db8:1::/48"), classify("2001:db8::/32")];
        let result = sort_blocks(lines, &opts());
        assert_eq!(sort_key(&result[0]), net("2001:db8::/32"));
        assert_eq!(sort_key(&result[1]), net("2001:db8:1::/48"));
    }

    #[test]
    fn test_mixed_ipv4_before_ipv6_default() {
        let lines = vec![classify("2001:db8::/32"), classify("10.0.0.0/8")];
        let result = sort_blocks(lines, &opts());
        assert_eq!(sort_key(&result[0]), net("10.0.0.0/8"));
        assert_eq!(sort_key(&result[1]), net("2001:db8::/32"));
    }

    #[test]
    fn test_mixed_ipv6_first_flag() {
        let opts = SortOptions { ipv6_first: true };
        let lines = vec![classify("10.0.0.0/8"), classify("2001:db8::/32")];
        let result = sort_blocks(lines, &opts);
        assert_eq!(sort_key(&result[0]), net("2001:db8::/32"));
        assert_eq!(sort_key(&result[1]), net("10.0.0.0/8"));
    }

    #[test]
    fn test_decorated_lines_sorted_by_sort_key() {
        let lines =
            vec![classify("- 192.168.0.0/16"), classify("- 10.0.0.0/8")];
        let result = sort_blocks(lines, &opts());
        assert_eq!(sort_key(&result[0]), net("10.0.0.0/8"));
        assert_eq!(sort_key(&result[1]), net("192.168.0.0/16"));
    }

    #[test]
    fn test_decorated_lines_original_preserved() {
        let lines =
            vec![classify("- 192.168.0.0/16"), classify("- 10.0.0.0/8")];
        let result = sort_blocks(lines, &opts());
        assert_eq!(original(&result[0]), "- 10.0.0.0/8");
        assert_eq!(original(&result[1]), "- 192.168.0.0/16");
    }
}

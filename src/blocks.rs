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
//!
//! `deduplicate_blocks` returns a [`DeduplicateError`] if a duplicate is found
//! on a multi-IP line in non-ips-only mode, since the correct action is
//! ambiguous and silently dropping content would be surprising.

use crate::classify::{ClassifiedLine, Span};
use crate::output::IpsOnlyMode;
use crate::sort::{SortOptions, compare};
use ipnet::IpNet;
use std::collections::HashSet;

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

/// Error returned when `--unique` encounters an ambiguous duplicate on a
/// multi-IP line in non-ips-only mode.
///
/// The caller should print `line` and `duplicate_ip` in a helpful error
/// message and exit with a non-zero status code.
#[derive(Debug)]
pub struct DeduplicateError {
    /// The original line content that caused the error.
    pub line: String,
    /// The normalized CIDR that was already seen.
    pub duplicate_ip: IpNet,
}

/// Deduplicate [`ClassifiedLine::HasIp`] lines, removing lines whose IPs have
/// already been seen in the output.
///
/// Behaviour depends on the number of IPs on a line and the output mode:
///
/// - **Single-IP line**: if the IP has been seen, the line is silently
///   dropped.
/// - **Multi-IP line, `--ips-only`** ([`IpsOnlyMode::Flat`] or
///   [`IpsOnlyMode::WithStructure`]): each IP is checked independently; seen
///   IPs are skipped, unseen IPs are kept. No error.
/// - **Multi-IP line, default mode**: if **any** IP has been seen, returns a
///   [`DeduplicateError`] (the ambiguity of which IP to remove and how to
///   handle the surrounding decoration requires the user to clean up their
///   input).
/// - **`NoIp` lines**: always passed through unchanged.
///
/// Must be called **after** [`sort_blocks`] so that duplicates are adjacent
/// and the seen set grows in sorted order.
pub fn deduplicate_blocks(
    lines: Vec<ClassifiedLine>,
    ips_only: &IpsOnlyMode,
) -> Result<Vec<ClassifiedLine>, DeduplicateError> {
    let mut seen: HashSet<IpNet> = HashSet::new();
    let mut output: Vec<ClassifiedLine> = Vec::with_capacity(lines.len());

    for line in lines {
        match line {
            ClassifiedLine::NoIp(_) => output.push(line),
            ClassifiedLine::HasIp {
                spans,
                sort_key: _,
                warnings,
            } => {
                // Step 1: intra-line dedup (remove duplicate IPs within this
                // line regardless of mode). Safe because there is no
                // ambiguity about which line to drop when duplicates are on
                // the same line.
                let mut intra_seen: HashSet<IpNet> = HashSet::new();
                let mut deduped_spans: Vec<Span> = Vec::new();
                for span in spans {
                    match &span {
                        Span::NonIp(_) => deduped_spans.push(span),
                        Span::Ip(t) => {
                            if let Some(net) = t.network().copied() {
                                if intra_seen.insert(net) {
                                    deduped_spans.push(span);
                                }
                                // else: intra-line duplicate, silently drop
                            }
                        }
                    }
                }

                // Recalculate IPs and sort_key from deduped spans
                let ips: Vec<IpNet> = deduped_spans
                    .iter()
                    .filter_map(|s| match s {
                        Span::Ip(t) => t.network().copied(),
                        Span::NonIp(_) => None,
                    })
                    .collect();

                if ips.is_empty() {
                    // All IPs were intra-line dupes (drop the line entirely)
                    continue;
                }

                let sort_key = ips
                    .iter()
                    .min_by(|a, b| compare(a, b, &SortOptions::default()))
                    .copied()
                    .unwrap();

                let spans = deduped_spans;
                let is_multi_ip = ips.len() > 1;

                // Step 2: inter-line dedup against the global seen set
                match ips_only {
                    IpsOnlyMode::Flat | IpsOnlyMode::WithStructure => {
                        // Per-IP dedup: filter seen IPs out of the line's spans
                        // and rebuild. If all IPs are dupes, drop the line.
                        let mut any_kept = false;
                        let mut new_spans: Vec<Span> = Vec::new();
                        let mut ip_iter = ips.iter();

                        for span in spans {
                            match &span {
                                Span::NonIp(_) => new_spans.push(span),
                                Span::Ip(_) => {
                                    let net = ip_iter.next().unwrap();
                                    if seen.contains(net) {
                                        // skip this IP span
                                    } else {
                                        seen.insert(*net);
                                        any_kept = true;
                                        new_spans.push(span);
                                    }
                                }
                            }
                        }

                        if any_kept {
                            // Recalculate sort_key from remaining spans
                            let new_sort_key = new_spans
                                .iter()
                                .filter_map(|s| match s {
                                    Span::Ip(t) => t.network().copied(),
                                    Span::NonIp(_) => None,
                                })
                                .min_by(|a, b| {
                                    compare(a, b, &SortOptions::default())
                                })
                                .unwrap();

                            output.push(ClassifiedLine::HasIp {
                                spans: new_spans,
                                sort_key: new_sort_key,
                                warnings,
                            });
                        }
                    }
                    IpsOnlyMode::Off => {
                        if is_multi_ip {
                            // Error on any seen IP
                            for net in &ips {
                                if seen.contains(net) {
                                    let line_str: String = spans
                                        .iter()
                                        .map(|s| match s {
                                            Span::Ip(t) => t.original(),
                                            Span::NonIp(s) => s.as_str(),
                                        })
                                        .collect();
                                    return Err(DeduplicateError {
                                        line: line_str,
                                        duplicate_ip: *net,
                                    });
                                }
                            }
                            // No dupes (add all and keep line)
                            for net in &ips {
                                seen.insert(*net);
                            }
                            output.push(ClassifiedLine::HasIp {
                                spans,
                                sort_key,
                                warnings,
                            });
                        } else {
                            // Single-IP line: silently drop if seen
                            let net = ips[0];
                            if !seen.contains(&net) {
                                seen.insert(net);
                                output.push(ClassifiedLine::HasIp {
                                    spans,
                                    sort_key,
                                    warnings,
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classify::classify_line;
    use crate::output::IpsOnlyMode;
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

    fn dedup(
        lines: Vec<ClassifiedLine>,
    ) -> Result<Vec<ClassifiedLine>, DeduplicateError> {
        deduplicate_blocks(lines, &IpsOnlyMode::Off)
    }

    fn dedup_flat(
        lines: Vec<ClassifiedLine>,
    ) -> Result<Vec<ClassifiedLine>, DeduplicateError> {
        deduplicate_blocks(lines, &IpsOnlyMode::Flat)
    }

    fn dedup_off(
        lines: Vec<ClassifiedLine>,
    ) -> Result<Vec<ClassifiedLine>, DeduplicateError> {
        deduplicate_blocks(lines, &IpsOnlyMode::Off)
    }

    fn ip_count(line: &ClassifiedLine) -> usize {
        match line {
            ClassifiedLine::HasIp { spans, .. } => {
                spans.iter().filter(|s| matches!(s, Span::Ip(_))).count()
            }
            ClassifiedLine::NoIp(_) => 0,
        }
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
        let opts = SortOptions {
            ipv6_first: true,
            reverse: false,
        };
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

    #[test]
    fn test_no_duplicates_unchanged() {
        let lines = vec![classify("10.0.0.0/8"), classify("192.168.0.0/16")];
        let result = dedup(lines).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_duplicate_single_ip_dropped() {
        let lines = vec![
            classify("10.0.0.0/8"),
            classify("10.0.0.0/8"),
            classify("192.168.0.0/16"),
        ];
        let result = dedup(lines).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(sort_key(&result[0]), net("10.0.0.0/8"));
        assert_eq!(sort_key(&result[1]), net("192.168.0.0/16"));
    }

    #[test]
    fn test_host_bits_normalized_for_dedup() {
        // 10.0.0.5/24 and 10.0.0.0/24 are the same after normalization
        let lines = vec![classify("10.0.0.0/24"), classify("10.0.0.5/24")];
        let result = dedup(lines).unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_different_prefix_lengths_not_deduped() {
        // 10.0.0.0/8 and 10.0.0.0/24 are different
        let lines = vec![classify("10.0.0.0/8"), classify("10.0.0.0/24")];
        let result = dedup(lines).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_no_ip_lines_pass_through() {
        let lines = vec![
            classify("10.0.0.0/8"),
            classify("# comment"),
            classify("10.0.0.0/8"),
        ];
        let result = dedup(lines).unwrap();
        assert_eq!(result.len(), 2); // comment + first IP, second IP dropped
        assert!(matches!(&result[1], ClassifiedLine::NoIp(_)));
    }

    #[test]
    fn test_decorated_duplicate_dropped() {
        let lines = vec![classify("- 10.0.0.0/8"), classify("- 10.0.0.0/8")];
        let result = dedup(lines).unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_multi_ip_duplicate_errors() {
        let lines = vec![
            classify("10.0.0.0/8"),
            classify("192.168.0.0/16 10.0.0.0/8"),
        ];
        assert!(dedup(lines).is_err());
    }

    #[test]
    fn test_multi_ip_error_contains_duplicate_ip() {
        let lines = vec![
            classify("10.0.0.0/8"),
            classify("192.168.0.0/16 10.0.0.0/8"),
        ];
        let err = dedup(lines).unwrap_err();
        assert_eq!(err.duplicate_ip, net("10.0.0.0/8"));
    }

    #[test]
    fn test_multi_ip_no_duplicates_ok() {
        let lines = vec![
            classify("10.0.0.0/8 192.168.0.0/16"),
            classify("172.16.0.0/12"),
        ];
        assert!(dedup(lines).is_ok());
    }

    #[test]
    fn test_multi_ip_non_sort_key_duplicate_also_errors() {
        // 192.168.0.0/16 is not the sort key of the second line but is still a
        // dupe
        let lines = vec![
            classify("192.168.0.0/16"),
            classify("10.0.0.0/8 192.168.0.0/16"),
        ];
        assert!(dedup(lines).is_err());
    }

    #[test]
    fn test_ips_only_duplicate_skipped_not_errored() {
        let lines = vec![
            classify("10.0.0.0/8"),
            classify("192.168.0.0/16 10.0.0.0/8"),
        ];
        assert!(dedup_flat(lines).is_ok());
    }

    #[test]
    fn test_ips_only_duplicate_ip_removed_from_line() {
        let lines = vec![
            classify("10.0.0.0/8"),
            classify("192.168.0.0/16 10.0.0.0/8"),
        ];
        let result = dedup_flat(lines).unwrap();
        // Second line should only have 192.168.0.0/16
        assert_eq!(result.len(), 2);
        assert_eq!(sort_key(&result[1]), net("192.168.0.0/16"));
    }

    #[test]
    fn test_ips_only_all_dupes_line_dropped() {
        let lines = vec![classify("10.0.0.0/8"), classify("10.0.0.0/8")];
        let result = dedup_flat(lines).unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_ips_only_partial_dedup_keeps_unique_ips() {
        let lines = vec![
            classify("10.0.0.0/8 192.168.0.0/16"),
            classify("10.0.0.0/8 172.16.0.0/12"),
        ];
        let result = dedup_flat(lines).unwrap();
        // First line unchanged, second line only has 172.16.0.0/12
        assert_eq!(result.len(), 2);
        assert_eq!(sort_key(&result[1]), net("172.16.0.0/12"));
    }

    #[test]
    fn test_intra_line_dup_removed_off_mode() {
        let lines = vec![classify("10.0.0.0/8 10.0.0.0/8 192.168.0.0/16")];
        let result = dedup_off(lines).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(ip_count(&result[0]), 2);
        assert_eq!(sort_key(&result[0]), net("10.0.0.0/8"));
    }

    #[test]
    fn test_intra_line_all_same_ips_becomes_single() {
        let lines = vec![classify("10.0.0.0/8 10.0.0.0/8")];
        let result = dedup_off(lines).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(ip_count(&result[0]), 1);
    }

    #[test]
    fn test_intra_line_single_arg_dedup() {
        let lines = vec![classify("10.10.10.10/32 10.10.10.10/32")];
        let result = dedup_off(lines).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(ip_count(&result[0]), 1);
        assert_eq!(sort_key(&result[0]), net("10.10.10.10/32"));
    }

    #[test]
    fn test_intra_line_dedup_then_inter_line_dedup() {
        // Line 1: 10.0.0.0/8 (single)
        // Line 2: 10.0.0.0/8 10.0.0.0/8 192.168.0.0/16
        //   -> after intra dedup: 10.0.0.0/8 192.168.0.0/16
        //   -> 10.0.0.0/8 is now an inter-line dup (error in Off mode)
        let lines = vec![
            classify("10.0.0.0/8"),
            classify("10.0.0.0/8 10.0.0.0/8 192.168.0.0/16"),
        ];
        assert!(dedup_off(lines).is_err());
    }

    #[test]
    fn test_intra_line_dedup_flat_mode() {
        let lines = vec![classify("10.0.0.0/8 10.0.0.0/8 192.168.0.0/16")];
        let result = dedup_flat(lines).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(ip_count(&result[0]), 2);
    }

    #[test]
    fn test_intra_line_all_dupes_line_dropped() {
        // All IPs are the same: after intra dedup only one remains,
        // then inter dedup sees it's new, so line is kept with one IP
        let lines = vec![classify("10.0.0.0/8 10.0.0.0/8")];
        let result = dedup_off(lines).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(ip_count(&result[0]), 1);
    }
}

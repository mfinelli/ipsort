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

//! Line classification for `ipsort`.
//!
//! This module takes raw input lines and classifies them as either containing
//! at least one IP address ([`ClassifiedLine::HasIp`]) or containing no IP
//! addresses ([`ClassifiedLine::NoIp`]).
//!
//! # Span model
//! Lines are represented as a sequence of [`Span`] values (alternating runs
//! of IP tokens and non-IP content). This preserves the full structure of the
//! original line so that output reconstruction can reorder IP spans while
//! leaving decoration (prefixes, punctuation, surrounding text) intact.
//!
//! For example, `"- 192.168.1.0/24 10.0.0.0/8"` produces:
//!
//! ```text
//! [NonIp("- "), Ip(192.168.1.0/24), NonIp(" "), Ip(10.0.0.0/8)]
//! ```
//!
//! Adjacent non-IP content is always merged into a single [`Span::NonIp`].
//! IP candidates that fail to parse are absorbed into adjacent `NonIp` spans.
//!
//! # Sort key
//! When a line contains multiple IP addresses, the [`ClassifiedLine::HasIp`]
//! `sort_key` is the lowest IP on the line according to
//! [`crate::sort::compare`]. This determines where the line is sorted within
//! its block.
//!
//! # Warnings
//! Host-bits-set tokens produce warnings that are collected in
//! [`ClassifiedLine::HasIp::warnings`] rather than emitted directly to stderr.
//! The caller is responsible for emitting warnings.

use crate::parse::{ParsedToken, is_cidr_char, parse_token};
use crate::sort::{SortOptions, compare};
use ipnet::IpNet;

/// A single span within a line (either an IP address token or a run of non-IP
/// content).
///
/// Lines are represented as `Vec<Span>` to preserve full structural
/// information for output reconstruction.
#[derive(Debug)]
pub enum Span {
    /// A recognized IP address or CIDR block.
    Ip(ParsedToken),
    /// A run of non-IP content (decoration, punctuation, whitespace, text).
    /// Adjacent non-IP content is always merged into a single `NonIp` span.
    NonIp(String),
}

/// A line of input after classification.
#[derive(Debug)]
pub enum ClassifiedLine {
    /// The line contains at least one recognizable IP address or CIDR.
    HasIp {
        /// The line represented as alternating IP and non-IP spans.
        /// Used for output reconstruction with reordered IPs.
        spans: Vec<Span>,
        /// The lowest IP on this line, used as the sort key for the line.
        sort_key: IpNet,
        /// Warning messages for any tokens with host bits set. Empty if none.
        /// The caller is responsible for emitting these to stderr.
        warnings: Vec<String>,
    },
    /// The line contains no recognizable IP address. Acts as a block
    /// separator. The original line is preserved for output.
    NoIp(String),
}

/// Produce an interleaved sequence of CIDR-character runs and
/// non-CIDR-character runs from a line, preserving all content.
///
/// Each element is `(is_cidr_run, content)`. Runs of the same type are never
/// adjacent (they strictly alternate).
fn interleave_runs(line: &str) -> Vec<(bool, &str)> {
    if line.is_empty() {
        return vec![];
    }

    let mut runs = Vec::new();
    let mut start = 0;

    while start < line.len() {
        let c = line[start..].chars().next().unwrap();
        let current_is_cidr = is_cidr_char(c);
        let mut end = start + c.len_utf8();

        while end < line.len() {
            let next_c = line[end..].chars().next().unwrap();
            if is_cidr_char(next_c) != current_is_cidr {
                break;
            }
            end += next_c.len_utf8();
        }

        runs.push((current_is_cidr, &line[start..end]));
        start = end;
    }

    runs
}

/// Classify a single line of input into a sequence of [`Span`]s.
///
/// CIDR-character runs are attempted as IP tokens via [`parse_token`]. Runs
/// that fail to parse are merged with adjacent non-IP content into
/// [`Span::NonIp`]. Adjacent `NonIp` spans are always merged.
fn spanify(line: &str) -> Vec<Span> {
    let runs = interleave_runs(line);
    let mut spans: Vec<Span> = Vec::new();

    for (is_cidr_run, content) in runs {
        if is_cidr_run {
            let parsed = parse_token(content);
            if parsed.is_ip() {
                spans.push(Span::Ip(parsed));
            } else {
                merge_non_ip(&mut spans, content);
            }
        } else {
            merge_non_ip(&mut spans, content);
        }
    }

    spans
}

/// Append `content` to the last [`Span::NonIp`] if one exists, otherwise push
/// a new one. This maintains the invariant that adjacent NonIp spans are
/// merged.
fn merge_non_ip(spans: &mut Vec<Span>, content: &str) {
    match spans.last_mut() {
        Some(Span::NonIp(s)) => s.push_str(content),
        _ => spans.push(Span::NonIp(content.to_string())),
    }
}

/// Classify a single line of input.
///
/// The line is decomposed into [`Span`]s. If any `Ip` spans are found, the
/// line is classified as [`ClassifiedLine::HasIp`] with the sort key set to
/// the lowest IP. Otherwise it is [`ClassifiedLine::NoIp`].
///
/// The `opts` parameter determines the sort key when the line contains
/// multiple IP addresses.
///
/// # Examples
/// ```rust
/// use ipsort::classify::{classify_line, ClassifiedLine};
/// use ipsort::sort::SortOptions;
///
/// let opts = SortOptions::default();
///
/// // A plain CIDR line
/// let line = classify_line("10.0.0.0/8", &opts);
/// assert!(matches!(line, ClassifiedLine::HasIp { .. }));
///
/// // A YAML-style decorated line
/// let line = classify_line("- 192.168.0.0/16", &opts);
/// assert!(matches!(line, ClassifiedLine::HasIp { .. }));
///
/// // A non-IP line
/// let line = classify_line("# this is a comment", &opts);
/// assert!(matches!(line, ClassifiedLine::NoIp(_)));
///
/// // An empty line
/// let line = classify_line("", &opts);
/// assert!(matches!(line, ClassifiedLine::NoIp(_)));
/// ```
pub fn classify_line(line: &str, opts: &SortOptions) -> ClassifiedLine {
    let spans = spanify(line);

    let mut warnings: Vec<String> = Vec::new();
    let mut networks: Vec<IpNet> = Vec::new();

    for span in &spans {
        if let Span::Ip(token) = span {
            if let ParsedToken::ValidCidr {
                original,
                network,
                had_host_bits: true,
            } = token
            {
                warnings.push(format!(
                    "warning: host bits set in {original:?}, treating as {network}"
                ));
            }
            if let Some(net) = token.network() {
                networks.push(*net);
            }
        }
    }

    if networks.is_empty() {
        return ClassifiedLine::NoIp(line.to_string());
    }

    networks.sort_by(|a, b| compare(a, b, opts));
    let sort_key = networks[0];

    ClassifiedLine::HasIp {
        spans,
        sort_key,
        warnings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn opts() -> SortOptions {
        SortOptions::default()
    }

    fn has_ip(line: &str) -> (IpNet, Vec<String>, Vec<Span>) {
        match classify_line(line, &opts()) {
            ClassifiedLine::HasIp {
                sort_key,
                warnings,
                spans,
            } => (sort_key, warnings, spans),
            ClassifiedLine::NoIp(s) => panic!("expected HasIp for {s:?}"),
        }
    }

    fn assert_no_ip(line: &str) {
        match classify_line(line, &opts()) {
            ClassifiedLine::NoIp(s) => assert_eq!(s, line),
            ClassifiedLine::HasIp { .. } => {
                panic!("expected NoIp for {line:?}")
            }
        }
    }

    fn assert_sort_key(line: &str, expected: &str) {
        let (sort_key, _, _) = has_ip(line);
        assert_eq!(
            sort_key,
            IpNet::from_str(expected).unwrap(),
            "sort_key mismatch for {line:?}"
        );
    }

    fn ip_count(spans: &[Span]) -> usize {
        spans.iter().filter(|s| matches!(s, Span::Ip(_))).count()
    }

    fn non_ip_count(spans: &[Span]) -> usize {
        spans.iter().filter(|s| matches!(s, Span::NonIp(_))).count()
    }

    fn reconstruct(spans: &[Span]) -> String {
        spans
            .iter()
            .map(|s| match s {
                Span::Ip(t) => t.original(),
                Span::NonIp(s) => s.as_str(),
            })
            .collect()
    }

    #[test]
    fn test_empty_line_is_no_ip() {
        assert_no_ip("");
    }

    #[test]
    fn test_whitespace_only_is_no_ip() {
        assert_no_ip("   ");
    }

    #[test]
    fn test_yaml_comment_is_no_ip() {
        assert_no_ip("# this is a comment");
    }

    #[test]
    fn test_plain_text_is_no_ip() {
        assert_no_ip("somekey: somevalue");
    }

    #[test]
    fn test_yaml_separator_is_no_ip() {
        assert_no_ip("---");
    }

    #[test]
    fn test_no_ip_preserves_original() {
        let line = "# comment with no ip";
        match classify_line(line, &opts()) {
            ClassifiedLine::NoIp(s) => assert_eq!(s, line),
            _ => panic!("expected NoIp"),
        }
    }

    #[test]
    fn test_plain_cidr() {
        assert_sort_key("10.0.0.0/8", "10.0.0.0/8");
    }

    #[test]
    fn test_bare_ip_promoted() {
        assert_sort_key("192.168.1.1", "192.168.1.1/32");
    }

    #[test]
    fn test_yaml_list_item() {
        assert_sort_key("- 10.0.0.0/8", "10.0.0.0/8");
    }

    #[test]
    fn test_quoted_cidr() {
        assert_sort_key("\"192.168.0.0/16\"", "192.168.0.0/16");
    }

    #[test]
    fn test_quoted_cidr_with_trailing_comma() {
        assert_sort_key("\"10.0.0.0/8\",", "10.0.0.0/8");
    }

    #[test]
    fn test_yaml_key_value() {
        assert_sort_key("network: 172.16.0.0/12", "172.16.0.0/12");
    }

    #[test]
    fn test_two_cidrs_whitespace_separated() {
        assert_sort_key("192.168.0.0/16 10.0.0.0/8", "10.0.0.0/8");
    }

    #[test]
    fn test_two_cidrs_comma_separated() {
        assert_sort_key("192.168.0.0/16,10.0.0.0/8", "10.0.0.0/8");
    }

    #[test]
    fn test_two_cidrs_semicolon_separated() {
        assert_sort_key("192.168.0.0/16;10.0.0.0/8", "10.0.0.0/8");
    }

    #[test]
    fn test_sort_key_is_lowest_network_address() {
        assert_sort_key(
            "172.16.0.0/12 10.0.0.0/8 192.168.0.0/16",
            "10.0.0.0/8",
        );
    }

    #[test]
    fn test_sort_key_same_network_picks_shorter_prefix() {
        // 10.0.0.0/8 and 10.0.0.0/24 have same network addr; /8 is lower
        // (shorter prefix first)
        assert_sort_key("10.0.0.0/24 10.0.0.0/8", "10.0.0.0/8");
    }

    #[test]
    fn test_mixed_ip_and_text_sort_key() {
        assert_sort_key("somekey: 192.168.1.0/24 10.0.0.0/8", "10.0.0.0/8");
    }

    #[test]
    fn test_json_array_of_cidrs() {
        assert_sort_key("[\"192.168.0.0/16\", \"10.0.0.0/8\"]", "10.0.0.0/8");
    }

    #[test]
    fn test_plain_cidr_single_ip_span() {
        let (_, _, spans) = has_ip("10.0.0.0/8");
        assert_eq!(ip_count(&spans), 1);
        assert_eq!(non_ip_count(&spans), 0);
    }

    #[test]
    fn test_yaml_list_item_spans() {
        let (_, _, spans) = has_ip("- 10.0.0.0/8");
        assert_eq!(ip_count(&spans), 1);
        assert_eq!(non_ip_count(&spans), 1);
    }

    #[test]
    fn test_two_ips_whitespace_spans() {
        let (_, _, spans) = has_ip("10.0.0.0/8 192.168.0.0/16");
        assert_eq!(ip_count(&spans), 2);
        assert_eq!(non_ip_count(&spans), 1);
    }

    #[test]
    fn test_two_ips_comma_spans() {
        let (_, _, spans) = has_ip("10.0.0.0/8,192.168.0.0/16");
        assert_eq!(ip_count(&spans), 2);
        assert_eq!(non_ip_count(&spans), 1);
    }

    #[test]
    fn test_non_ip_candidate_merged_into_non_ip_span() {
        // "1.2.3.4.5" looks like a cidr run but fails to parse (should be
        // NonIp)
        let (_, _, spans) = has_ip("1.2.3.4.5 10.0.0.0/8");
        assert_eq!(ip_count(&spans), 1);
        assert_eq!(non_ip_count(&spans), 1);
    }

    #[test]
    fn test_reconstruct_plain_cidr() {
        let (_, _, spans) = has_ip("10.0.0.0/8");
        assert_eq!(reconstruct(&spans), "10.0.0.0/8");
    }

    #[test]
    fn test_reconstruct_yaml_list_item() {
        let (_, _, spans) = has_ip("- 10.0.0.0/8");
        assert_eq!(reconstruct(&spans), "- 10.0.0.0/8");
    }

    #[test]
    fn test_reconstruct_two_ips() {
        let line = "10.0.0.0/8 192.168.0.0/16";
        let (_, _, spans) = has_ip(line);
        assert_eq!(reconstruct(&spans), line);
    }

    #[test]
    fn test_reconstruct_quoted_with_comma() {
        let line = "\"10.0.0.0/8\", \"192.168.0.0/16\"";
        let (_, _, spans) = has_ip(line);
        assert_eq!(reconstruct(&spans), line);
    }

    #[test]
    fn test_reconstruct_yaml_key_value() {
        let line = "network: 172.16.0.0/12";
        let (_, _, spans) = has_ip(line);
        assert_eq!(reconstruct(&spans), line);
    }

    #[test]
    fn test_no_warning_for_clean_cidr() {
        let (_, warnings, _) = has_ip("10.0.0.0/8");
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_warning_for_host_bits_set() {
        let (_, warnings, _) = has_ip("10.0.0.5/8");
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("host bits set"));
        assert!(warnings[0].contains("10.0.0.5/8"));
    }

    #[test]
    fn test_warning_contains_normalized_network() {
        let (_, warnings, _) = has_ip("10.0.0.5/8");
        assert!(warnings[0].contains("10.0.0.0/8"));
    }

    #[test]
    fn test_multiple_warnings_for_multiple_host_bits() {
        let (_, warnings, _) = has_ip("10.0.0.5/8 192.168.1.50/24");
        assert_eq!(warnings.len(), 2);
    }

    #[test]
    fn test_no_warning_for_bare_ip() {
        let (_, warnings, _) = has_ip("192.168.1.1");
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_ipv6_cidr() {
        assert_sort_key("2001:db8::/32", "2001:db8::/32");
    }

    #[test]
    fn test_mixed_ipv4_ipv6_sort_key_is_ipv4_first() {
        assert_sort_key("2001:db8::/32 10.0.0.0/8", "10.0.0.0/8");
    }

    #[test]
    fn test_mixed_ipv4_ipv6_sort_key_ipv6_first_flag() {
        let opts = SortOptions {
            ipv6_first: true,
            reverse: false,
        };
        match classify_line("2001:db8::/32 10.0.0.0/8", &opts) {
            ClassifiedLine::HasIp { sort_key, .. } => {
                assert_eq!(sort_key, IpNet::from_str("2001:db8::/32").unwrap());
            }
            _ => panic!("expected HasIp"),
        }
    }
}

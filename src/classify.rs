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
//! # Line splitting
//!
//! Tokens are split on any character that cannot appear inside a valid CIDR
//! (the same set stripped by [`crate::parse::strip_cidr_punctuation`]). This
//! means both whitespace-separated and delimiter-separated input is handled
//! correctly:
//!
//! - `10.0.0.0/8 192.168.0.0/16` -> two tokens
//! - `10.0.0.0/8,192.168.0.0/16` -> two tokens
//! - `"10.0.0.0/8", "192.168.0.0/16"` -> two tokens
//! - `- 10.0.0.0/8` -> two tokens, `-` becomes `NotAnIp`
//!
//! Empty tokens produced by splitting are discarded before parsing.
//!
//! # Sort key
//!
//! When a line contains multiple IP addresses, the [`ClassifiedLine::HasIp`]
//! `sort_key` is the lowest IP on the line according to
//! [`crate::sort::compare`]. This determines where the line is sorted within
//! its block.
//!
//! # Warnings
//!
//! Host-bits-set tokens produce warnings that are collected in
//! [`ClassifiedLine::HasIp::warnings`] rather than emitted directly to stderr.
//! The caller is responsible for emitting warnings.

use crate::parse::{ParsedToken, is_cidr_char, parse_token};
use crate::sort::{SortOptions, compare};
use ipnet::IpNet;

/// A line of input after classification.
#[derive(Debug)]
pub enum ClassifiedLine {
    /// The line contains at least one recognizable IP address or CIDR.
    HasIp {
        /// The original unmodified line, preserved for output.
        original: String,
        /// The lowest IP on this line, used as the sort key for the line.
        sort_key: IpNet,
        /// All tokens parsed from the line, in original order.
        tokens: Vec<ParsedToken>,
        /// Warning messages for any tokens with host bits set. Empty if none.
        /// The caller is responsible for emitting these to stderr.
        warnings: Vec<String>,
    },
    /// The line contains no recognizable IP address. Acts as a block
    /// separator. The original line is preserved for output.
    NoIp(String),
}

/// Split a line into candidate tokens on any character that is not a valid
/// CIDR character (as defined by [`crate::parse::is_cidr_char`]).
///
/// Empty tokens produced by splitting on consecutive delimiters are discarded.
/// The caller should pass each resulting token to
/// [`crate::parse::parse_token`].
fn split_tokens(line: &str) -> Vec<&str> {
    line.split(|c: char| !is_cidr_char(c))
        .filter(|s| !s.is_empty())
        .collect()
}

/// Classify a single line of input.
///
/// The line is split into tokens, each token is parsed via
/// [`crate::parse::parse_token`], and the result is classified as
/// [`ClassifiedLine::HasIp`] or [`ClassifiedLine::NoIp`].
///
/// The `opts` parameter is used to determine the sort key when the line
/// contains multiple IP addresses (the lowest IP according to the sort order).
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
    let raw_tokens = split_tokens(line);
    let tokens: Vec<ParsedToken> =
        raw_tokens.iter().map(|t| parse_token(t)).collect();

    // Collect warnings for host-bits-set tokens
    let warnings: Vec<String> = tokens
        .iter()
        .filter_map(|t| {
            if let ParsedToken::ValidCidr { original, network,
                had_host_bits: true } = t {
                Some(format!(
                    "warning: host bits set in {original:?}, treating as {network}"
                ))
            } else {
                None
            }
        })
        .collect();

    // Find all IP networks on this line
    let mut networks: Vec<&IpNet> =
        tokens.iter().filter_map(|t| t.network()).collect();

    if networks.is_empty() {
        return ClassifiedLine::NoIp(line.to_string());
    }

    // Sort to find the lowest IP (this is the line's sort key)
    networks.sort_by(|a, b| compare(a, b, opts));
    let sort_key = *networks[0];

    ClassifiedLine::HasIp {
        original: line.to_string(),
        sort_key,
        tokens,
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

    fn has_ip(line: &str) -> (IpNet, Vec<String>) {
        match classify_line(line, &opts()) {
            ClassifiedLine::HasIp {
                sort_key, warnings, ..
            } => (sort_key, warnings),
            ClassifiedLine::NoIp(s) => panic!("expected HasIp for {s:?}"),
        }
    }

    fn assert_no_ip(line: &str) {
        match classify_line(line, &opts()) {
            ClassifiedLine::NoIp(s) => assert_eq!(s, line),
            ClassifiedLine::HasIp { original, .. } => {
                panic!("expected NoIp for {original:?}")
            }
        }
    }

    fn assert_sort_key(line: &str, expected: &str) {
        let (sort_key, _) = has_ip(line);
        assert_eq!(
            sort_key,
            IpNet::from_str(expected).unwrap(),
            "sort_key mismatch for {line:?}"
        );
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
    fn test_has_ip_preserves_original() {
        let line = "- 10.0.0.0/8";
        match classify_line(line, &opts()) {
            ClassifiedLine::HasIp { original, .. } => {
                assert_eq!(original, line)
            }
            _ => panic!("expected HasIp"),
        }
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
    fn test_no_warning_for_clean_cidr() {
        let (_, warnings) = has_ip("10.0.0.0/8");
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_warning_for_host_bits_set() {
        let (_, warnings) = has_ip("10.0.0.5/8");
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("host bits set"));
        assert!(warnings[0].contains("10.0.0.5/8"));
    }

    #[test]
    fn test_warning_contains_normalized_network() {
        let (_, warnings) = has_ip("10.0.0.5/8");
        assert!(warnings[0].contains("10.0.0.0/8"));
    }

    #[test]
    fn test_multiple_warnings_for_multiple_host_bits() {
        let (_, warnings) = has_ip("10.0.0.5/8 192.168.1.50/24");
        assert_eq!(warnings.len(), 2);
    }

    #[test]
    fn test_no_warning_for_bare_ip() {
        // Bare IPs are promoted to /32 (no host bits warning)
        let (_, warnings) = has_ip("192.168.1.1");
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_token_count_single() {
        match classify_line("10.0.0.0/8", &opts()) {
            ClassifiedLine::HasIp { tokens, .. } => assert_eq!(tokens.len(), 1),
            _ => panic!("expected HasIp"),
        }
    }

    #[test]
    fn test_token_count_mixed_line() {
        match classify_line("- 10.0.0.0/8", &opts()) {
            ClassifiedLine::HasIp { tokens, .. } => assert_eq!(tokens.len(), 1),
            _ => panic!("expected HasIp"),
        }
    }

    #[test]
    fn test_token_count_multiple_ips() {
        match classify_line("10.0.0.0/8 192.168.0.0/16", &opts()) {
            ClassifiedLine::HasIp { tokens, .. } => assert_eq!(tokens.len(), 2),
            _ => panic!("expected HasIp"),
        }
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
        let opts = SortOptions { ipv6_first: true };
        match classify_line("2001:db8::/32 10.0.0.0/8", &opts) {
            ClassifiedLine::HasIp { sort_key, .. } => {
                assert_eq!(sort_key, IpNet::from_str("2001:db8::/32").unwrap());
            }
            _ => panic!("expected HasIp"),
        }
    }
}

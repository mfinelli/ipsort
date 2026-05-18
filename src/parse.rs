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

//! Token-level CIDR parsing and normalization.
//!
//! This module is responsible for taking a raw string token (a single
//! whitespace-delimited word from a line of input) and determining whether it
//! represents an IP address or CIDR block. It handles the following cases:
//!
//! - **Valid CIDR** (`10.0.0.0/8`, `2001:db8::/32`): parsed and normalized.
//!   Normalization clears host bits so that `10.0.0.5/24` is stored as
//!   `10.0.0.0/24` for sorting purposes, while the original string is
//!   preserved for output.
//! - **Bare IP address** (`10.0.0.1`, `::1`): promoted to `/32` or `/128`
//!   respectively, treated as a valid CIDR for sorting.
//! - **Non-IP content** (`somekey:`, `-`, `#comment`, `"hello"`): returned as
//!   `NotAnIp` for passthrough.
//!
//! Before parsing, leading and trailing characters that cannot appear in a
//! valid CIDR are stripped. This handles common punctuation wrapping such as
//! `"10.0.0.0/8",`, `[10.0.0.0/8]`, or `10.0.0.0/8;` without requiring a
//! regex. The stripping is done by removing any character that is not in the
//! set `[0-9a-fA-F.:/]` from both ends of the token.

use ipnet::IpNet;
use std::net::IpAddr;
use std::str::FromStr;

/// The result of attempting to parse a single string token as a CIDR or IP
/// address.
#[derive(Clone, Debug, PartialEq)]
pub enum ParsedToken {
    /// A valid CIDR block, possibly with host bits set in the original.
    ///
    /// `network` always has host bits cleared (canonical network address).
    /// `had_host_bits` is true if the original string had host bits set,
    /// indicating a warning should be emitted by the caller.
    ValidCidr {
        original: String,
        network: IpNet,
        had_host_bits: bool,
    },
    /// A bare IP address (no prefix length) that has been promoted to /32
    /// (IPv4) or /128 (IPv6).
    BareIp { original: String, network: IpNet },
    /// A token that could not be interpreted as an IP address or CIDR.
    /// The original string is preserved for passthrough output.
    NotAnIp(String),
}

impl ParsedToken {
    /// Returns the normalized [`IpNet`] if this token is a valid address,
    /// or `None` if it is not an IP.
    pub fn network(&self) -> Option<&IpNet> {
        match self {
            ParsedToken::ValidCidr { network, .. } => Some(network),
            ParsedToken::BareIp { network, .. } => Some(network),
            ParsedToken::NotAnIp(_) => None,
        }
    }

    /// Returns the original string as provided (before any normalization).
    pub fn original(&self) -> &str {
        match self {
            ParsedToken::ValidCidr { original, .. } => original,
            ParsedToken::BareIp { original, .. } => original,
            ParsedToken::NotAnIp(s) => s,
        }
    }

    /// Returns `true` if this token is a recognizable IP address or CIDR.
    pub fn is_ip(&self) -> bool {
        !matches!(self, ParsedToken::NotAnIp(_))
    }
}

/// Attempt to parse a single whitespace-delimited token as a CIDR or IP
/// address.
///
/// Leading and trailing non-CIDR punctuation is stripped before parsing (see
/// `strip_cidr_punctuation`). The original unstripped token is preserved in
/// the returned value for output purposes.
///
/// Parsing is attempted in this order:
/// 1. [`IpNet::from_str`]: handles `10.0.0.0/8`, `2001:db8::/32`, etc.
/// 2. [`IpAddr::from_str`]: handles bare addresses, promoted to /32 or /128.
/// 3. If both fail, returns [`ParsedToken::NotAnIp`].
///
/// # Examples
/// ```rust
/// use ipsort::parse::{parse_token, ParsedToken};
///
/// // Valid CIDR
/// let t = parse_token("10.0.0.0/8");
/// assert!(matches!(t, ParsedToken::ValidCidr { had_host_bits: false, .. }));
///
/// // Host bits set
/// let t = parse_token("10.0.0.5/24");
/// assert!(matches!(t, ParsedToken::ValidCidr { had_host_bits: true, .. }));
///
/// // Bare IP promoted to /32
/// let t = parse_token("192.168.1.1");
/// assert!(matches!(t, ParsedToken::BareIp { .. }));
///
/// // Punctuation-wrapped token
/// let t = parse_token("\"10.0.0.0/8\",");
/// assert!(t.is_ip());
///
/// // Not an IP
/// let t = parse_token("somekey:");
/// assert!(!t.is_ip());
/// ```
pub fn parse_token(token: &str) -> ParsedToken {
    let stripped = strip_cidr_punctuation(token);

    if stripped.is_empty() {
        return ParsedToken::NotAnIp(token.to_string());
    }

    // Try full CIDR first
    if let Ok(net) = IpNet::from_str(stripped) {
        let normalized = net.trunc(); // clears host bits
        let had_host_bits = net.addr() != normalized.network();
        return ParsedToken::ValidCidr {
            original: token.to_string(),
            network: normalized,
            had_host_bits,
        };
    }

    // Try bare IP address, promote to /32 or /128
    if let Ok(addr) = IpAddr::from_str(stripped) {
        let network = IpNet::from(addr); // /32 for v4, /128 for v6
        return ParsedToken::BareIp {
            original: token.to_string(),
            network,
        };
    }

    ParsedToken::NotAnIp(token.to_string())
}

/// Returns `true` if a character can appear inside a valid CIDR or IP address.
///
/// The valid set is `[0-9a-fA-F.:/]`:
/// - `0-9`, `a-f`, `A-F`: decimal octets and hexadecimal IPv6 groups
/// - `.`: IPv4 octet separator
/// - `:`: IPv6 group separator
/// - `/`: CIDR prefix length separator
///
/// This is the single source of truth for what constitutes a CIDR character,
/// used by both [`strip_cidr_punctuation`] and the line spanification logic
/// in [`crate::classify`].
pub(crate) fn is_cidr_char(c: char) -> bool {
    c.is_ascii_hexdigit() || matches!(c, '.' | ':' | '/')
}

/// Strip leading and trailing characters that are not valid CIDR characters
/// (as defined by [`is_cidr_char`]) from both ends of a token.
///
/// This handles common punctuation wrapping such as `"10.0.0.0/8",`,
/// `[10.0.0.0/8]`, or `10.0.0.0/8;` without requiring a regex.
///
/// Only the ends are stripped; characters in the middle are left alone so
/// that malformed tokens like `10.0.0.1/24abc` are not silently accepted.
fn strip_cidr_punctuation(s: &str) -> &str {
    s.trim_matches(|c: char| !is_cidr_char(c))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_valid_cidr(
        token: &str,
        expected_network: &str,
        expected_host_bits: bool,
    ) {
        match parse_token(token) {
            ParsedToken::ValidCidr {
                original,
                network,
                had_host_bits,
            } => {
                assert_eq!(original, token);
                assert_eq!(
                    network.to_string(),
                    expected_network,
                    "network mismatch for {token}"
                );
                assert_eq!(
                    had_host_bits, expected_host_bits,
                    "had_host_bits mismatch for {token}"
                );
            }
            other => panic!("expected ValidCidr for {token:?}, got {other:?}"),
        }
    }

    fn assert_bare_ip(token: &str, expected_network: &str) {
        match parse_token(token) {
            ParsedToken::BareIp { original, network } => {
                assert_eq!(original, token);
                assert_eq!(
                    network.to_string(),
                    expected_network,
                    "network mismatch for {token}"
                );
            }
            other => panic!("expected BareIp for {token:?}, got {other:?}"),
        }
    }

    fn assert_not_ip(token: &str) {
        match parse_token(token) {
            ParsedToken::NotAnIp(s) => assert_eq!(s, token),
            other => panic!("expected NotAnIp for {token:?}, got {other:?}"),
        }
    }

    #[test]
    fn test_ipv4_cidr_clean() {
        assert_valid_cidr("10.0.0.0/8", "10.0.0.0/8", false);
    }

    #[test]
    fn test_ipv4_cidr_class_b() {
        assert_valid_cidr("192.168.0.0/16", "192.168.0.0/16", false);
    }

    #[test]
    fn test_ipv4_cidr_slash32() {
        assert_valid_cidr("10.0.0.1/32", "10.0.0.1/32", false);
    }

    #[test]
    fn test_ipv4_cidr_slash0() {
        assert_valid_cidr("0.0.0.0/0", "0.0.0.0/0", false);
    }

    #[test]
    fn test_ipv4_host_bits_set() {
        assert_valid_cidr("10.0.0.5/24", "10.0.0.0/24", true);
    }

    #[test]
    fn test_ipv4_host_bits_set_preserves_original() {
        match parse_token("10.0.0.5/24") {
            ParsedToken::ValidCidr { original, .. } => {
                assert_eq!(original, "10.0.0.5/24")
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn test_ipv4_host_bits_set_class_b() {
        assert_valid_cidr("172.16.5.1/16", "172.16.0.0/16", true);
    }

    #[test]
    fn test_ipv6_cidr_clean() {
        assert_valid_cidr("2001:db8::/32", "2001:db8::/32", false);
    }

    #[test]
    fn test_ipv6_cidr_slash128() {
        assert_valid_cidr("::1/128", "::1/128", false);
    }

    #[test]
    fn test_ipv6_cidr_slash0() {
        assert_valid_cidr("::/0", "::/0", false);
    }

    #[test]
    fn test_ipv6_host_bits_set() {
        assert_valid_cidr("2001:db8::1/32", "2001:db8::/32", true);
    }

    #[test]
    fn test_bare_ipv4_promoted_to_slash32() {
        assert_bare_ip("192.168.1.1", "192.168.1.1/32");
    }

    #[test]
    fn test_bare_ipv4_zero() {
        assert_bare_ip("0.0.0.0", "0.0.0.0/32");
    }

    #[test]
    fn test_bare_ipv4_broadcast() {
        assert_bare_ip("255.255.255.255", "255.255.255.255/32");
    }

    #[test]
    fn test_bare_ipv6_promoted_to_slash128() {
        assert_bare_ip("2001:db8::1", "2001:db8::1/128");
    }

    #[test]
    fn test_bare_ipv6_loopback() {
        assert_bare_ip("::1", "::1/128");
    }

    #[test]
    fn test_bare_ip_preserves_original() {
        match parse_token("10.0.0.1") {
            ParsedToken::BareIp { original, .. } => {
                assert_eq!(original, "10.0.0.1")
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn test_double_quoted_with_trailing_comma() {
        assert_valid_cidr("\"10.0.0.0/8\",", "10.0.0.0/8", false);
    }

    #[test]
    fn test_single_quoted() {
        assert_valid_cidr("'10.0.0.0/8'", "10.0.0.0/8", false);
    }

    #[test]
    fn test_square_brackets() {
        assert_valid_cidr("[10.0.0.0/8]", "10.0.0.0/8", false);
    }

    #[test]
    fn test_trailing_comma() {
        assert_valid_cidr("10.0.0.0/8,", "10.0.0.0/8", false);
    }

    #[test]
    fn test_trailing_semicolon() {
        assert_valid_cidr("10.0.0.0/8;", "10.0.0.0/8", false);
    }

    #[test]
    fn test_curly_braces() {
        assert_valid_cidr("{10.0.0.0/8}", "10.0.0.0/8", false);
    }

    #[test]
    fn test_parentheses() {
        assert_valid_cidr("(10.0.0.0/8)", "10.0.0.0/8", false);
    }

    #[test]
    fn test_yaml_list_dash_is_not_ip() {
        // "-" alone (yaml list marker split from value) is not an IP
        assert_not_ip("-");
    }

    #[test]
    fn test_punctuation_stripping_preserves_original() {
        match parse_token("\"10.0.0.0/8\",") {
            ParsedToken::ValidCidr { original, .. } => {
                assert_eq!(original, "\"10.0.0.0/8\",")
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn test_bare_ip_with_punctuation() {
        assert_bare_ip("\"192.168.1.1\"", "192.168.1.1/32");
    }

    #[test]
    fn test_plain_word() {
        assert_not_ip("somekey:");
    }

    #[test]
    fn test_yaml_comment() {
        assert_not_ip("#comment");
    }

    #[test]
    fn test_empty_string() {
        assert_not_ip("");
    }

    #[test]
    fn test_only_punctuation() {
        assert_not_ip("---");
    }

    #[test]
    fn test_version_number_not_ip() {
        // Version numbers look IP-like but have too many octets or wrong
        // format
        assert_not_ip("1.2.3.4.5");
    }

    #[test]
    fn test_invalid_prefix_length() {
        // /33 is invalid for IPv4; host bits logic doesn't apply, it's
        // malformed
        assert_not_ip("10.0.0.0/33");
    }

    #[test]
    fn test_invalid_octet() {
        assert_not_ip("999.0.0.1/24");
    }

    #[test]
    fn test_word_with_numbers() {
        assert_not_ip("v1.2.3");
    }

    #[test]
    fn test_network_returns_some_for_valid_cidr() {
        let t = parse_token("10.0.0.0/8");
        assert!(t.network().is_some());
    }

    #[test]
    fn test_network_returns_some_for_bare_ip() {
        let t = parse_token("10.0.0.1");
        assert!(t.network().is_some());
    }

    #[test]
    fn test_network_returns_none_for_not_ip() {
        let t = parse_token("hello");
        assert!(t.network().is_none());
    }

    #[test]
    fn test_is_ip_true_for_cidr() {
        assert!(parse_token("10.0.0.0/8").is_ip());
    }

    #[test]
    fn test_is_ip_true_for_bare() {
        assert!(parse_token("10.0.0.1").is_ip());
    }

    #[test]
    fn test_is_ip_false_for_not_ip() {
        assert!(!parse_token("hello").is_ip());
    }

    #[test]
    fn test_original_roundtrip_not_ip() {
        let token = "some-random-text";
        assert_eq!(parse_token(token).original(), token);
    }

    #[test]
    fn test_strip_nothing_to_strip() {
        assert_eq!(strip_cidr_punctuation("10.0.0.0/8"), "10.0.0.0/8");
    }

    #[test]
    fn test_strip_double_quotes() {
        assert_eq!(strip_cidr_punctuation("\"10.0.0.0/8\""), "10.0.0.0/8");
    }

    #[test]
    fn test_strip_single_quotes() {
        assert_eq!(strip_cidr_punctuation("'10.0.0.0/8'"), "10.0.0.0/8");
    }

    #[test]
    fn test_strip_trailing_comma() {
        assert_eq!(strip_cidr_punctuation("10.0.0.0/8,"), "10.0.0.0/8");
    }

    #[test]
    fn test_strip_trailing_semicolon() {
        assert_eq!(strip_cidr_punctuation("10.0.0.0/8;"), "10.0.0.0/8");
    }

    #[test]
    fn test_strip_square_brackets() {
        assert_eq!(strip_cidr_punctuation("[10.0.0.0/8]"), "10.0.0.0/8");
    }

    #[test]
    fn test_strip_curly_braces() {
        assert_eq!(strip_cidr_punctuation("{10.0.0.0/8}"), "10.0.0.0/8");
    }

    #[test]
    fn test_strip_parentheses() {
        assert_eq!(strip_cidr_punctuation("(10.0.0.0/8)"), "10.0.0.0/8");
    }

    #[test]
    fn test_strip_mixed_punctuation() {
        assert_eq!(strip_cidr_punctuation("\"10.0.0.0/8\","), "10.0.0.0/8");
    }

    #[test]
    fn test_strip_bare_ip() {
        assert_eq!(strip_cidr_punctuation("\"192.168.1.1\""), "192.168.1.1");
    }

    #[test]
    fn test_strip_all_punctuation_returns_empty() {
        assert_eq!(strip_cidr_punctuation("---"), "");
    }

    #[test]
    fn test_strip_only_whitespace_adjacent() {
        // whitespace is not a valid cidr char so it gets stripped too
        assert_eq!(strip_cidr_punctuation(" 10.0.0.0/8 "), "10.0.0.0/8");
    }

    #[test]
    fn test_strip_does_not_touch_middle() {
        // internal non-cidr chars are preserved (malformed token, not our job
        // to fix)
        assert_eq!(
            strip_cidr_punctuation("\"10.0.0.1/24abc\""),
            "10.0.0.1/24abc"
        );
    }

    #[test]
    fn test_strip_ipv6() {
        assert_eq!(
            strip_cidr_punctuation("\"2001:db8::/32\","),
            "2001:db8::/32"
        );
    }
}

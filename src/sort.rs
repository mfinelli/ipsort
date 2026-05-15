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

//! Sorting primitives for CIDR/IP address ordering.
//!
//! This module provides [`SortOptions`] and the [`compare`] function, which
//! together define the canonical ordering used throughout `ipsort`.
//!
//! # Ordering rules
//!
//! Two [`IpNet`] values are ordered as follows:
//!
//! 1. **IP family**: IPv4 and IPv6 are sorted as separate groups. By default
//!    IPv4 comes first; [`SortOptions::ipv6_first`] inverts this.
//! 2. **Network address**: within the same family, addresses are compared
//!    numerically (ascending). IPv4 as a 32-bit integer, IPv6 as a 128-bit
//!    integer.
//! 3. **Prefix length**: when two addresses have the same network address,
//!    the shorter prefix (larger block) comes first. So `10.0.0.0/8` sorts
//!    before `10.0.0.0/24`.

use ipnet::IpNet;
use std::cmp::Ordering;
use std::net::IpAddr;

/// Runtime options that affect how IP addresses are ordered.
///
/// Pass this to [`compare`] to control sort behaviour. Construct with
/// [`SortOptions::default()`] for standard IPv4-first ordering.
#[derive(Debug, Clone, PartialEq)]
pub struct SortOptions {
    /// When `true`, IPv6 addresses sort before IPv4 addresses in mixed input.
    /// Defaults to `false` (IPv4 first).
    pub ipv6_first: bool,
}

impl Default for SortOptions {
    fn default() -> Self {
        SortOptions { ipv6_first: false }
    }
}

/// Compare two [`IpNet`] values according to `opts`.
///
/// See the [module-level documentation](self) for the full ordering rules.
///
/// # Examples
/// ```
/// use ipnet::IpNet;
/// use std::cmp::Ordering;
/// use std::str::FromStr;
/// use ipsort::sort::{compare, SortOptions};
///
/// let opts = SortOptions::default();
///
/// let a = IpNet::from_str("10.0.0.0/8").unwrap();
/// let b = IpNet::from_str("10.0.0.0/24").unwrap();
/// assert_eq!(compare(&a, &b, &opts), Ordering::Less); // /8 before /24
///
/// let c = IpNet::from_str("192.168.0.0/16").unwrap();
/// assert_eq!(compare(&a, &c, &opts), Ordering::Less); // 10.x before 192.x
/// ```
pub fn compare(a: &IpNet, b: &IpNet, opts: &SortOptions) -> Ordering {
    let a_is_v4 = matches!(a, IpNet::V4(_));
    let b_is_v4 = matches!(b, IpNet::V4(_));

    // Step 1: order by IP family
    if a_is_v4 != b_is_v4 {
        return if opts.ipv6_first {
            // IPv6 first: v6 < v4
            if a_is_v4 {
                Ordering::Greater
            } else {
                Ordering::Less
            }
        } else {
            // IPv4 first (default): v4 < v6
            if a_is_v4 {
                Ordering::Less
            } else {
                Ordering::Greater
            }
        };
    }

    // Step 2: same family - compare network addresses numerically
    let addr_order = match (a.network(), b.network()) {
        (IpAddr::V4(a4), IpAddr::V4(b4)) => {
            let a_int = u32::from(a4);
            let b_int = u32::from(b4);
            a_int.cmp(&b_int)
        }
        (IpAddr::V6(a6), IpAddr::V6(b6)) => {
            let a_int = u128::from(a6);
            let b_int = u128::from(b6);
            a_int.cmp(&b_int)
        }
        // Unreachable: we've already established both are the same family
        _ => unreachable!(),
    };

    if addr_order != Ordering::Equal {
        return addr_order;
    }

    // Step 3: same network address - shorter prefix (larger block) first
    a.prefix_len().cmp(&b.prefix_len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn net(s: &str) -> IpNet {
        IpNet::from_str(s).unwrap()
    }

    fn opts() -> SortOptions {
        SortOptions::default()
    }

    fn opts_v6_first() -> SortOptions {
        SortOptions { ipv6_first: true }
    }

    #[test]
    fn test_ipv4_lower_address_first() {
        assert_eq!(
            compare(&net("10.0.0.0/8"), &net("192.168.0.0/16"), &opts()),
            Ordering::Less
        );
    }

    #[test]
    fn test_ipv4_higher_address_last() {
        assert_eq!(
            compare(&net("192.168.0.0/16"), &net("10.0.0.0/8"), &opts()),
            Ordering::Greater
        );
    }

    #[test]
    fn test_ipv4_equal_address_and_prefix() {
        assert_eq!(
            compare(&net("10.0.0.0/8"), &net("10.0.0.0/8"), &opts()),
            Ordering::Equal
        );
    }

    #[test]
    fn test_ipv4_adjacent_addresses() {
        assert_eq!(
            compare(&net("10.0.0.0/24"), &net("10.0.1.0/24"), &opts()),
            Ordering::Less
        );
    }

    #[test]
    fn test_ipv4_same_network_shorter_prefix_first() {
        assert_eq!(
            compare(&net("10.0.0.0/8"), &net("10.0.0.0/24"), &opts()),
            Ordering::Less
        );
    }

    #[test]
    fn test_ipv4_same_network_longer_prefix_last() {
        assert_eq!(
            compare(&net("10.0.0.0/24"), &net("10.0.0.0/8"), &opts()),
            Ordering::Greater
        );
    }

    #[test]
    fn test_ipv4_same_network_slash16_vs_slash24() {
        assert_eq!(
            compare(&net("172.16.0.0/16"), &net("172.16.0.0/24"), &opts()),
            Ordering::Less
        );
    }

    #[test]
    fn test_ipv4_slash0_before_everything() {
        assert_eq!(
            compare(&net("0.0.0.0/0"), &net("10.0.0.0/8"), &opts()),
            Ordering::Less
        );
    }

    #[test]
    fn test_ipv4_slash32_after_same_network() {
        assert_eq!(
            compare(&net("10.0.0.1/32"), &net("10.0.0.1/32"), &opts()),
            Ordering::Equal
        );
    }

    #[test]
    fn test_ipv6_lower_address_first() {
        assert_eq!(
            compare(&net("2001:db8::/32"), &net("2001:db8:1::/48"), &opts()),
            Ordering::Less
        );
    }

    #[test]
    fn test_ipv6_equal() {
        assert_eq!(
            compare(&net("2001:db8::/32"), &net("2001:db8::/32"), &opts()),
            Ordering::Equal
        );
    }

    #[test]
    fn test_ipv6_same_network_shorter_prefix_first() {
        assert_eq!(
            compare(&net("2001:db8::/32"), &net("2001:db8::/48"), &opts()),
            Ordering::Less
        );
    }

    #[test]
    fn test_ipv6_loopback_vs_other() {
        assert_eq!(
            compare(&net("::1/128"), &net("2001:db8::/32"), &opts()),
            Ordering::Less
        );
    }

    #[test]
    fn test_ipv6_slash0_before_everything() {
        assert_eq!(
            compare(&net("::/0"), &net("2001:db8::/32"), &opts()),
            Ordering::Less
        );
    }

    #[test]
    fn test_mixed_ipv4_before_ipv6_by_default() {
        assert_eq!(
            compare(&net("10.0.0.0/8"), &net("2001:db8::/32"), &opts()),
            Ordering::Less
        );
    }

    #[test]
    fn test_mixed_ipv6_after_ipv4_by_default() {
        assert_eq!(
            compare(&net("2001:db8::/32"), &net("10.0.0.0/8"), &opts()),
            Ordering::Greater
        );
    }

    #[test]
    fn test_mixed_high_ipv4_still_before_ipv6() {
        // Even a high IPv4 address sorts before any IPv6 address in default
        // mode
        assert_eq!(
            compare(&net("192.168.0.0/16"), &net("::1/128"), &opts()),
            Ordering::Less
        );
    }

    #[test]
    fn test_mixed_ipv6_first_flag() {
        assert_eq!(
            compare(
                &net("2001:db8::/32"),
                &net("10.0.0.0/8"),
                &opts_v6_first()
            ),
            Ordering::Less
        );
    }

    #[test]
    fn test_mixed_ipv4_after_ipv6_when_flag_set() {
        assert_eq!(
            compare(
                &net("10.0.0.0/8"),
                &net("2001:db8::/32"),
                &opts_v6_first()
            ),
            Ordering::Greater
        );
    }

    #[test]
    fn test_mixed_ipv6_first_does_not_affect_ipv4_ordering() {
        // Within IPv4, ordering should be unaffected by ipv6_first
        assert_eq!(
            compare(
                &net("10.0.0.0/8"),
                &net("192.168.0.0/16"),
                &opts_v6_first()
            ),
            Ordering::Less
        );
    }

    #[test]
    fn test_mixed_ipv6_first_does_not_affect_ipv6_ordering() {
        // Within IPv6, ordering should be unaffected by ipv6_first
        assert_eq!(
            compare(
                &net("2001:db8::/32"),
                &net("2001:db8:1::/48"),
                &opts_v6_first()
            ),
            Ordering::Less
        );
    }

    #[test]
    fn test_default_sort_options_ipv4_first() {
        assert!(!SortOptions::default().ipv6_first);
    }

    #[test]
    fn test_sort_options_clone() {
        let a = SortOptions { ipv6_first: true };
        let b = a.clone();
        assert_eq!(a, b);
    }
}

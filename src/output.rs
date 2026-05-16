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

//! Output rendering for `ipsort`.
//!
//! This module is responsible for converting sorted [`ClassifiedLine`] values
//! back into strings for printing. It handles intra-line IP sorting
//! (reordering the `Ip` spans within a single line), decoration preservation,
//! and the output mode flags `--normalize`, `--ips-only`, and
//! `--ips-only-with-structure`.
//!
//! # Rendering modes
//! **Default**: `NonIp` spans are emitted verbatim; `Ip` spans are replaced
//! with their sorted counterparts using the original token string.
//!
//! **`--normalize`**: same as default but `Ip` spans emit the canonical
//! network string (`10.0.0.0/8`) rather than the original token
//! (`10.0.0.5/8`). Bare IPs gain explicit prefix lengths (`192.168.1.1` ->
//! `192.168.1.1/32`).
//!
//! **`--ips-only`** ([`IpsOnlyMode::Flat`]): all `NonIp` spans and all
//! [`ClassifiedLine::NoIp`] lines are discarded. Each `Ip` span becomes one
//! output line. The entire input is treated as a single flat pool.
//!
//! **`--ips-only-with-structure`** ([`IpsOnlyMode::WithStructure`]): `NonIp`
//! spans within `HasIp` lines are discarded but [`ClassifiedLine::NoIp`] lines
//! are preserved as block separators. Each `Ip` span becomes one output line.
//!
//! `--ips-only` and `--ips-only-with-structure` are mutually exclusive.
//!
//! # Intra-line sorting
//! [`render_line`] sorts the IP addresses within a line before substituting
//! them back into the span positions. This is the final step that ensures
//! multi-IP lines like `"192.168.0.0/16 10.0.0.0/8"` are emitted as
//! `"10.0.0.0/8 192.168.0.0/16"`.

use crate::classify::{ClassifiedLine, Span};
use crate::parse::ParsedToken;
use crate::sort::{SortOptions, compare};

/// Controls whether and how non-IP content is stripped from output.
///
/// `--ips-only` and `--ips-only-with-structure` are mutually exclusive.
/// The CLI layer is responsible for enforcing this.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum IpsOnlyMode {
    /// Default: preserve all content, mirror input format.
    #[default]
    Off,
    /// `--ips-only`: discard all non-IP content and non-IP lines.
    /// Each `Ip` span becomes one output line. No block separators.
    Flat,
    /// `--ips-only-with-structure`: discard decoration within lines but
    /// preserve `NoIp` lines as block separators.
    /// Each `Ip` span becomes one output line per block.
    WithStructure,
}

/// Runtime options controlling how lines are rendered to output strings.
///
/// Construct with [`OutputOptions::default()`] for standard behaviour.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct OutputOptions {
    /// When `true`, emit canonical network strings instead of original tokens.
    /// Host bits are cleared and bare IPs gain explicit prefix lengths.
    pub normalize: bool,
    /// Controls IP-extraction mode. See [`IpsOnlyMode`].
    pub ips_only: IpsOnlyMode,
}

/// Return the string to emit for a single [`ParsedToken`] given output
/// options.
///
/// - Default: the original token string as provided in the input
/// - `--normalize`: the canonical network string from `ipnet`
fn render_token(token: &ParsedToken, opts: &OutputOptions) -> String {
    if opts.normalize {
        match token.network() {
            Some(net) => net.to_string(),
            None => token.original().to_string(),
        }
    } else {
        token.original().to_string()
    }
}

/// Render a single [`ClassifiedLine`] into zero or more output strings.
///
/// Returns a `Vec<String>` because ips-only modes can expand a single input
/// line into multiple output lines (one per IP span), and `--ips-only` can
/// collapse `NoIp` lines to nothing. In default mode the vec always contains
/// exactly one element.
///
/// For [`ClassifiedLine::NoIp`]:
/// - Default and `--ips-only-with-structure`: return the original line
/// - `--ips-only`: return an empty vec (line is discarded)
///
/// For [`ClassifiedLine::HasIp`]:
/// - Default: reconstruct the line with IPs sorted, decoration preserved
/// - `--ips-only` or `--ips-only-with-structure`: emit one line per IP span,
///   decoration discarded
///
/// # Examples
/// ```rust
/// use ipsort::classify::classify_line;
/// use ipsort::output::{render_line, OutputOptions};
/// use ipsort::sort::SortOptions;
///
/// let sort_opts = SortOptions::default();
/// let out_opts = OutputOptions::default();
///
/// let line = classify_line("192.168.0.0/16 10.0.0.0/8", &sort_opts);
/// let rendered = render_line(&line, &out_opts, &sort_opts);
/// assert_eq!(rendered, vec!["10.0.0.0/8 192.168.0.0/16"]);
/// ```
pub fn render_line(
    line: &ClassifiedLine,
    out_opts: &OutputOptions,
    sort_opts: &SortOptions,
) -> Vec<String> {
    match line {
        ClassifiedLine::NoIp(s) => match out_opts.ips_only {
            IpsOnlyMode::Flat => vec![], // discard non-IP lines
            IpsOnlyMode::Off | IpsOnlyMode::WithStructure => vec![s.clone()], // preserve as separator
        },
        ClassifiedLine::HasIp { spans, .. } => {
            // Collect and sort IP tokens from this line
            let mut ip_tokens: Vec<&ParsedToken> = spans
                .iter()
                .filter_map(|s| match s {
                    Span::Ip(t) => Some(t),
                    Span::NonIp(_) => None,
                })
                .collect();

            ip_tokens.sort_by(|a, b| {
                let a_net = a.network().unwrap();
                let b_net = b.network().unwrap();
                compare(a_net, b_net, sort_opts)
            });

            match out_opts.ips_only {
                IpsOnlyMode::Flat | IpsOnlyMode::WithStructure => {
                    // One output line per IP, decoration discarded
                    ip_tokens
                        .iter()
                        .map(|t| render_token(t, out_opts))
                        .collect()
                }
                IpsOnlyMode::Off => {
                    // Walk spans, substituting sorted IPs in order
                    let mut ip_iter = ip_tokens.into_iter();
                    let mut result = String::new();
                    for span in spans {
                        match span {
                            Span::Ip(_) => {
                                if let Some(token) = ip_iter.next() {
                                    result.push_str(&render_token(
                                        token, out_opts,
                                    ));
                                }
                            }
                            Span::NonIp(s) => result.push_str(s),
                        }
                    }
                    vec![result]
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classify::classify_line;

    fn sort_opts() -> SortOptions {
        SortOptions::default()
    }

    fn out_opts() -> OutputOptions {
        OutputOptions::default()
    }

    fn render(line: &str) -> Vec<String> {
        let classified = classify_line(line, &sort_opts());
        render_line(&classified, &out_opts(), &sort_opts())
    }

    fn render_with(line: &str, out_opts: &OutputOptions) -> Vec<String> {
        let classified = classify_line(line, &sort_opts());
        render_line(&classified, out_opts, &sort_opts())
    }

    fn render_with_sort(line: &str, sort_opts: &SortOptions) -> Vec<String> {
        let classified = classify_line(line, sort_opts);
        render_line(&classified, &out_opts(), sort_opts)
    }

    #[test]
    fn test_no_ip_passthrough_default() {
        assert_eq!(render("# comment"), vec!["# comment"]);
    }

    #[test]
    fn test_empty_line_passthrough_default() {
        assert_eq!(render(""), vec![""]);
    }

    #[test]
    fn test_yaml_separator_passthrough_default() {
        assert_eq!(render("---"), vec!["---"]);
    }

    #[test]
    fn test_no_ip_passthrough_with_structure() {
        let opts = OutputOptions {
            ips_only: IpsOnlyMode::WithStructure,
            ..Default::default()
        };
        assert_eq!(render_with("# comment", &opts), vec!["# comment"]);
    }

    #[test]
    fn test_no_ip_discarded_flat() {
        let opts = OutputOptions {
            ips_only: IpsOnlyMode::Flat,
            ..Default::default()
        };
        assert_eq!(render_with("# comment", &opts), Vec::<String>::new());
    }

    #[test]
    fn test_empty_line_discarded_flat() {
        let opts = OutputOptions {
            ips_only: IpsOnlyMode::Flat,
            ..Default::default()
        };
        assert_eq!(render_with("", &opts), Vec::<String>::new());
    }

    #[test]
    fn test_plain_cidr_unchanged() {
        assert_eq!(render("10.0.0.0/8"), vec!["10.0.0.0/8"]);
    }

    #[test]
    fn test_bare_ip_preserved() {
        assert_eq!(render("192.168.1.1"), vec!["192.168.1.1"]);
    }

    #[test]
    fn test_yaml_list_item_preserved() {
        assert_eq!(render("- 10.0.0.0/8"), vec!["- 10.0.0.0/8"]);
    }

    #[test]
    fn test_yaml_key_value_preserved() {
        assert_eq!(
            render("network: 172.16.0.0/12"),
            vec!["network: 172.16.0.0/12"]
        );
    }

    #[test]
    fn test_quoted_cidr_preserved() {
        assert_eq!(render("\"10.0.0.0/8\""), vec!["\"10.0.0.0/8\""]);
    }

    #[test]
    fn test_two_ips_sorted() {
        assert_eq!(
            render("192.168.0.0/16 10.0.0.0/8"),
            vec!["10.0.0.0/8 192.168.0.0/16"]
        );
    }

    #[test]
    fn test_three_ips_sorted() {
        assert_eq!(
            render("192.168.0.0/16 10.0.0.0/8 172.16.0.0/12"),
            vec!["10.0.0.0/8 172.16.0.0/12 192.168.0.0/16"]
        );
    }

    #[test]
    fn test_two_ips_comma_separated_sorted() {
        assert_eq!(
            render("192.168.0.0/16,10.0.0.0/8"),
            vec!["10.0.0.0/8,192.168.0.0/16"]
        );
    }

    #[test]
    fn test_decoration_preserved_after_sort() {
        assert_eq!(
            render("somekey: 192.168.1.0/24 10.0.0.0/8"),
            vec!["somekey: 10.0.0.0/8 192.168.1.0/24"]
        );
    }

    #[test]
    fn test_quoted_ips_sorted() {
        assert_eq!(
            render("\"192.168.0.0/16\", \"10.0.0.0/8\""),
            vec!["\"10.0.0.0/8\", \"192.168.0.0/16\""]
        );
    }

    #[test]
    fn test_already_sorted_unchanged() {
        assert_eq!(
            render("10.0.0.0/8 192.168.0.0/16"),
            vec!["10.0.0.0/8 192.168.0.0/16"]
        );
    }

    #[test]
    fn test_same_network_shorter_prefix_first() {
        assert_eq!(
            render("10.0.0.0/24 10.0.0.0/8"),
            vec!["10.0.0.0/8 10.0.0.0/24"]
        );
    }

    #[test]
    fn test_host_bits_original_preserved_by_default() {
        assert_eq!(render("10.0.0.5/24"), vec!["10.0.0.5/24"]);
    }

    #[test]
    fn test_normalize_clean_cidr_unchanged() {
        let opts = OutputOptions {
            normalize: true,
            ..Default::default()
        };
        assert_eq!(render_with("10.0.0.0/8", &opts), vec!["10.0.0.0/8"]);
    }

    #[test]
    fn test_normalize_host_bits_cleared() {
        let opts = OutputOptions {
            normalize: true,
            ..Default::default()
        };
        assert_eq!(render_with("10.0.0.5/24", &opts), vec!["10.0.0.0/24"]);
    }

    #[test]
    fn test_normalize_bare_ip_gets_prefix() {
        let opts = OutputOptions {
            normalize: true,
            ..Default::default()
        };
        assert_eq!(render_with("192.168.1.1", &opts), vec!["192.168.1.1/32"]);
    }

    #[test]
    fn test_normalize_bare_ipv6_gets_prefix() {
        let opts = OutputOptions {
            normalize: true,
            ..Default::default()
        };
        assert_eq!(render_with("2001:db8::1", &opts), vec!["2001:db8::1/128"]);
    }

    #[test]
    fn test_normalize_decoration_preserved() {
        let opts = OutputOptions {
            normalize: true,
            ..Default::default()
        };
        assert_eq!(render_with("- 10.0.0.5/24", &opts), vec!["- 10.0.0.0/24"]);
    }

    #[test]
    fn test_normalize_multiple_ips() {
        let opts = OutputOptions {
            normalize: true,
            ..Default::default()
        };
        assert_eq!(
            render_with("192.168.1.50/24 10.0.0.5/8", &opts),
            vec!["10.0.0.0/8 192.168.1.0/24"]
        );
    }

    #[test]
    fn test_ips_only_flat_single_ip() {
        let opts = OutputOptions {
            ips_only: IpsOnlyMode::Flat,
            ..Default::default()
        };
        assert_eq!(render_with("10.0.0.0/8", &opts), vec!["10.0.0.0/8"]);
    }

    #[test]
    fn test_ips_only_flat_strips_decoration() {
        let opts = OutputOptions {
            ips_only: IpsOnlyMode::Flat,
            ..Default::default()
        };
        assert_eq!(render_with("- 10.0.0.0/8", &opts), vec!["10.0.0.0/8"]);
    }

    #[test]
    fn test_ips_only_flat_two_ips_two_lines() {
        let opts = OutputOptions {
            ips_only: IpsOnlyMode::Flat,
            ..Default::default()
        };
        assert_eq!(
            render_with("192.168.0.0/16 10.0.0.0/8", &opts),
            vec!["10.0.0.0/8", "192.168.0.0/16"]
        );
    }

    #[test]
    fn test_ips_only_flat_strips_yaml_key() {
        let opts = OutputOptions {
            ips_only: IpsOnlyMode::Flat,
            ..Default::default()
        };
        assert_eq!(
            render_with("somekey: 192.168.1.0/24 10.0.0.0/8", &opts),
            vec!["10.0.0.0/8", "192.168.1.0/24"]
        );
    }

    #[test]
    fn test_ips_only_flat_preserves_original_token() {
        let opts = OutputOptions {
            ips_only: IpsOnlyMode::Flat,
            ..Default::default()
        };
        assert_eq!(render_with("10.0.0.5/24", &opts), vec!["10.0.0.5/24"]);
    }

    #[test]
    fn test_ips_only_with_structure_single_ip() {
        let opts = OutputOptions {
            ips_only: IpsOnlyMode::WithStructure,
            ..Default::default()
        };
        assert_eq!(render_with("10.0.0.0/8", &opts), vec!["10.0.0.0/8"]);
    }

    #[test]
    fn test_ips_only_with_structure_strips_decoration() {
        let opts = OutputOptions {
            ips_only: IpsOnlyMode::WithStructure,
            ..Default::default()
        };
        assert_eq!(render_with("- 10.0.0.0/8", &opts), vec!["10.0.0.0/8"]);
    }

    #[test]
    fn test_ips_only_with_structure_two_ips_two_lines() {
        let opts = OutputOptions {
            ips_only: IpsOnlyMode::WithStructure,
            ..Default::default()
        };
        assert_eq!(
            render_with("192.168.0.0/16 10.0.0.0/8", &opts),
            vec!["10.0.0.0/8", "192.168.0.0/16"]
        );
    }

    #[test]
    fn test_ips_only_with_structure_preserves_no_ip_line() {
        let opts = OutputOptions {
            ips_only: IpsOnlyMode::WithStructure,
            ..Default::default()
        };
        assert_eq!(render_with("# comment", &opts), vec!["# comment"]);
    }

    #[test]
    fn test_ips_only_flat_with_normalize() {
        let opts = OutputOptions {
            ips_only: IpsOnlyMode::Flat,
            normalize: true,
        };
        assert_eq!(render_with("- 10.0.0.5/24", &opts), vec!["10.0.0.0/24"]);
    }

    #[test]
    fn test_ips_only_flat_normalize_bare_ip() {
        let opts = OutputOptions {
            ips_only: IpsOnlyMode::Flat,
            normalize: true,
        };
        assert_eq!(render_with("192.168.1.1", &opts), vec!["192.168.1.1/32"]);
    }

    #[test]
    fn test_ips_only_with_structure_normalize() {
        let opts = OutputOptions {
            ips_only: IpsOnlyMode::WithStructure,
            normalize: true,
        };
        assert_eq!(render_with("- 10.0.0.5/24", &opts), vec!["10.0.0.0/24"]);
    }

    #[test]
    fn test_ipv6_single() {
        assert_eq!(render("2001:db8::/32"), vec!["2001:db8::/32"]);
    }

    #[test]
    fn test_mixed_ipv4_ipv6_sorted() {
        assert_eq!(
            render("2001:db8::/32 10.0.0.0/8"),
            vec!["10.0.0.0/8 2001:db8::/32"]
        );
    }

    #[test]
    fn test_mixed_ipv6_first_flag() {
        let sort_opts = SortOptions {
            ipv6_first: true,
            reverse: false,
        };
        assert_eq!(
            render_with_sort("10.0.0.0/8 2001:db8::/32", &sort_opts),
            vec!["2001:db8::/32 10.0.0.0/8"]
        );
    }

    #[test]
    fn test_single_ip_returns_one_element_vec() {
        assert_eq!(render("10.0.0.0/8").len(), 1);
    }

    #[test]
    fn test_no_ip_default_returns_one_element_vec() {
        assert_eq!(render("# comment").len(), 1);
    }

    #[test]
    fn test_no_ip_flat_returns_empty_vec() {
        let opts = OutputOptions {
            ips_only: IpsOnlyMode::Flat,
            ..Default::default()
        };
        assert_eq!(render_with("# comment", &opts).len(), 0);
    }

    #[test]
    fn test_two_ips_flat_returns_two_element_vec() {
        let opts = OutputOptions {
            ips_only: IpsOnlyMode::Flat,
            ..Default::default()
        };
        assert_eq!(render_with("10.0.0.0/8 192.168.0.0/16", &opts).len(), 2);
    }
}

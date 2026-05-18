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

use assert_cmd::Command;
use predicates::prelude::*;

fn ipsort() -> Command {
    Command::cargo_bin("ipsort").unwrap()
}

#[test]
fn test_stdin_basic_sort() {
    ipsort()
        .write_stdin("192.168.1.0/24\n10.0.0.0/8\n172.16.0.0/12\n")
        .assert()
        .success()
        .stdout("10.0.0.0/8\n172.16.0.0/12\n192.168.1.0/24\n");
}

#[test]
fn test_stdin_already_sorted() {
    ipsort()
        .write_stdin("10.0.0.0/8\n172.16.0.0/12\n192.168.1.0/24\n")
        .assert()
        .success()
        .stdout("10.0.0.0/8\n172.16.0.0/12\n192.168.1.0/24\n");
}

#[test]
fn test_stdin_bare_ips() {
    ipsort()
        .write_stdin("192.168.1.1\n10.0.0.1\n")
        .assert()
        .success()
        .stdout("10.0.0.1\n192.168.1.1\n");
}

#[test]
fn test_stdin_single_ip() {
    ipsort()
        .write_stdin("10.0.0.0/8\n")
        .assert()
        .success()
        .stdout("10.0.0.0/8\n");
}

#[test]
fn test_positional_args_multiple() {
    ipsort()
        .args(["192.168.1.0/24", "10.0.0.0/8", "172.16.0.0/12"])
        .assert()
        .success()
        .stdout("10.0.0.0/8\n172.16.0.0/12\n192.168.1.0/24\n");
}

#[test]
fn test_positional_single_arg_space_separated() {
    ipsort()
        .arg("192.168.1.0/24 10.0.0.0/8 172.16.0.0/12")
        .assert()
        .success()
        .stdout("10.0.0.0/8 172.16.0.0/12 192.168.1.0/24\n");
}

#[test]
fn test_positional_single_arg_comma_separated() {
    ipsort()
        .arg("192.168.1.0/24,10.0.0.0/8,172.16.0.0/12")
        .assert()
        .success()
        .stdout("10.0.0.0/8,172.16.0.0/12,192.168.1.0/24\n");
}

#[test]
fn test_same_network_shorter_prefix_first() {
    ipsort()
        .write_stdin("10.0.0.0/24\n10.0.0.0/8\n10.0.0.0/16\n")
        .assert()
        .success()
        .stdout("10.0.0.0/8\n10.0.0.0/16\n10.0.0.0/24\n");
}

#[test]
fn test_yaml_list_decoration_preserved() {
    ipsort()
        .write_stdin("- 192.168.1.0/24\n- 10.0.0.0/8\n- 172.16.0.0/12\n")
        .assert()
        .success()
        .stdout("- 10.0.0.0/8\n- 172.16.0.0/12\n- 192.168.1.0/24\n");
}

#[test]
fn test_json_quoted_cidrs_preserved() {
    ipsort()
        .write_stdin("\"192.168.0.0/16\", \"10.0.0.0/8\"\n")
        .assert()
        .success()
        .stdout("\"10.0.0.0/8\", \"192.168.0.0/16\"\n");
}

#[test]
fn test_yaml_key_value_preserved() {
    ipsort()
        .write_stdin("network: 192.168.0.0/16\nnetwork: 10.0.0.0/8\n")
        .assert()
        .success()
        .stdout("network: 10.0.0.0/8\nnetwork: 192.168.0.0/16\n");
}

#[test]
fn test_multi_ip_line_intra_sorted() {
    ipsort()
        .write_stdin("192.168.0.0/16 10.0.0.0/8\n")
        .assert()
        .success()
        .stdout("10.0.0.0/8 192.168.0.0/16\n");
}

#[test]
fn test_blank_line_separator() {
    ipsort()
        .write_stdin(
            "192.168.1.0/24\n10.0.0.0/8\n\n172.16.2.0/24\n172.16.1.0/24\n",
        )
        .assert()
        .success()
        .stdout("10.0.0.0/8\n192.168.1.0/24\n\n172.16.1.0/24\n172.16.2.0/24\n");
}

#[test]
fn test_comment_separator_preserved() {
    ipsort()
        .write_stdin("# group one\n192.168.1.0/24\n10.0.0.0/8\n# group two\n172.16.2.0/24\n172.16.1.0/24\n")
        .assert()
        .success()
        .stdout("# group one\n10.0.0.0/8\n192.168.1.0/24\n# group two\n172.16.1.0/24\n172.16.2.0/24\n");
}

#[test]
fn test_leading_comment_preserved() {
    ipsort()
        .write_stdin("# header\n192.168.1.0/24\n10.0.0.0/8\n")
        .assert()
        .success()
        .stdout("# header\n10.0.0.0/8\n192.168.1.0/24\n");
}

#[test]
fn test_trailing_comment_preserved() {
    ipsort()
        .write_stdin("192.168.1.0/24\n10.0.0.0/8\n# footer\n")
        .assert()
        .success()
        .stdout("10.0.0.0/8\n192.168.1.0/24\n# footer\n");
}

#[test]
fn test_ipv4_before_ipv6_default() {
    ipsort()
        .write_stdin("2001:db8::/32\n10.0.0.0/8\n")
        .assert()
        .success()
        .stdout("10.0.0.0/8\n2001:db8::/32\n");
}

#[test]
fn test_ipv6_first_flag() {
    ipsort()
        .args(["--ipv6-first"])
        .write_stdin("10.0.0.0/8\n2001:db8::/32\n")
        .assert()
        .success()
        .stdout("2001:db8::/32\n10.0.0.0/8\n");
}

#[test]
fn test_reverse() {
    ipsort()
        .args(["--reverse", "10.0.0.0/8", "172.16.0.0/12", "192.168.1.0/24"])
        .assert()
        .success()
        .stdout("192.168.1.0/24\n172.16.0.0/12\n10.0.0.0/8\n");
}

#[test]
fn test_reverse_short_flag() {
    ipsort()
        .args(["-r", "10.0.0.0/8", "172.16.0.0/12"])
        .assert()
        .success()
        .stdout("172.16.0.0/12\n10.0.0.0/8\n");
}

#[test]
fn test_normalize_host_bits() {
    ipsort()
        .args(["--normalize"])
        .write_stdin("10.0.0.5/24\n")
        .assert()
        .success()
        .stdout("10.0.0.0/24\n");
}

#[test]
fn test_normalize_bare_ip() {
    ipsort()
        .args(["--normalize"])
        .write_stdin("192.168.1.1\n")
        .assert()
        .success()
        .stdout("192.168.1.1/32\n");
}

#[test]
fn test_normalize_bare_ipv6() {
    ipsort()
        .args(["--normalize"])
        .write_stdin("2001:db8::1\n")
        .assert()
        .success()
        .stdout("2001:db8::1/128\n");
}

#[test]
fn test_normalize_preserves_decoration() {
    ipsort()
        .args(["--normalize"])
        .write_stdin("- 10.0.0.5/24\n")
        .assert()
        .success()
        .stdout("- 10.0.0.0/24\n");
}

#[test]
fn test_unique_drops_duplicate_lines() {
    ipsort()
        .args(["--unique"])
        .write_stdin("10.0.0.0/8\n192.168.0.0/16\n10.0.0.0/8\n")
        .assert()
        .success()
        .stdout("10.0.0.0/8\n192.168.0.0/16\n");
}

#[test]
fn test_unique_short_flag() {
    ipsort()
        .args(["-u"])
        .write_stdin("10.0.0.0/8\n10.0.0.0/8\n")
        .assert()
        .success()
        .stdout("10.0.0.0/8\n");
}

#[test]
fn test_unique_normalizes_for_comparison() {
    // 10.0.0.5/24 and 10.0.0.0/24 normalize to the same network
    ipsort()
        .args(["--unique"])
        .write_stdin("10.0.0.0/24\n10.0.0.5/24\n")
        .assert()
        .success()
        .stdout("10.0.0.0/24\n");
}

#[test]
fn test_unique_different_prefix_lengths_not_deduped() {
    ipsort()
        .args(["--unique"])
        .write_stdin("10.0.0.0/8\n10.0.0.0/24\n")
        .assert()
        .success()
        .stdout("10.0.0.0/8\n10.0.0.0/24\n");
}

#[test]
fn test_unique_multi_ip_line_errors() {
    ipsort()
        .args(["--unique"])
        .write_stdin("10.0.0.0/8\n192.168.0.0/16 10.0.0.0/8\n")
        .assert()
        .failure()
        .stderr(predicate::str::contains("10.0.0.0/8"))
        .stderr(predicate::str::contains("--unique"));
}

#[test]
fn test_unique_with_ips_only_no_error_on_multi_ip() {
    ipsort()
        .args(["--unique", "--ips-only"])
        .write_stdin("10.0.0.0/8\n192.168.0.0/16 10.0.0.0/8\n")
        .assert()
        .success()
        .stdout("10.0.0.0/8\n192.168.0.0/16\n");
}

#[test]
fn test_unique_intra_line_dedup() {
    ipsort()
        .args(["--unique"])
        .arg("10.10.10.10/32 10.10.10.10/32")
        .assert()
        .success()
        .stdout(predicate::str::contains("10.10.10.10/32"));
}

#[test]
fn test_ips_only_strips_decoration() {
    ipsort()
        .args(["--ips-only"])
        .write_stdin("- 192.168.1.0/24\n- 10.0.0.0/8\n")
        .assert()
        .success()
        .stdout("10.0.0.0/8\n192.168.1.0/24\n");
}

#[test]
fn test_ips_only_discards_non_ip_lines() {
    ipsort()
        .args(["--ips-only"])
        .write_stdin("# comment\n10.0.0.0/8\n\n192.168.0.0/16\n")
        .assert()
        .success()
        .stdout("10.0.0.0/8\n192.168.0.0/16\n");
}

#[test]
fn test_ips_only_multi_ip_line_one_per_output_line() {
    ipsort()
        .args(["--ips-only"])
        .write_stdin("192.168.0.0/16 10.0.0.0/8\n")
        .assert()
        .success()
        .stdout("10.0.0.0/8\n192.168.0.0/16\n");
}

#[test]
fn test_ips_only_with_structure_strips_decoration() {
    ipsort()
        .args(["--ips-only-with-structure"])
        .write_stdin("- 192.168.1.0/24\n- 10.0.0.0/8\n")
        .assert()
        .success()
        .stdout("10.0.0.0/8\n192.168.1.0/24\n");
}

#[test]
fn test_ips_only_with_structure_preserves_separators() {
    ipsort()
        .args(["--ips-only-with-structure"])
        .write_stdin("# group one\n192.168.1.0/24\n10.0.0.0/8\n\n# group two\n172.16.2.0/24\n172.16.1.0/24\n")
        .assert()
        .success()
        .stdout("# group one\n10.0.0.0/8\n192.168.1.0/24\n\n# group two\n172.16.1.0/24\n172.16.2.0/24\n");
}

#[test]
fn test_ips_only_and_ips_only_with_structure_mutually_exclusive() {
    ipsort()
        .args(["--ips-only", "--ips-only-with-structure", "10.0.0.0/8"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn test_inline_crosses_block_separator() {
    ipsort()
        .args(["--inline"])
        .write_stdin("192.168.1.0/24\n\n10.0.0.0/8\n")
        .assert()
        .success()
        .stdout("10.0.0.0/8\n\n192.168.1.0/24\n");
}

#[test]
fn test_inline_multi_line_yaml_value() {
    ipsort()
        .args(["--inline"])
        .write_stdin("allowed_ips: 192.168.1.0/24 10.0.0.0/8\n  172.16.2.0/24 172.16.1.0/24\n")
        .assert()
        .success()
        .stdout("allowed_ips: 10.0.0.0/8 172.16.1.0/24\n  172.16.2.0/24 192.168.1.0/24\n");
}

#[test]
fn test_inline_preserves_decoration() {
    ipsort()
        .args(["--inline"])
        .write_stdin("- 192.168.1.0/24\n- 10.0.0.0/8\n")
        .assert()
        .success()
        .stdout("- 10.0.0.0/8\n- 192.168.1.0/24\n");
}

#[test]
fn test_inline_with_unique() {
    ipsort()
        .args(["--inline", "--unique"])
        .write_stdin("10.0.0.0/8\n192.168.0.0/16\n10.0.0.0/8\n")
        .assert()
        .success()
        .stdout("10.0.0.0/8\n192.168.0.0/16\n\n");
}

#[test]
fn test_host_bits_warning_to_stderr() {
    ipsort()
        .write_stdin("10.0.0.5/24\n")
        .assert()
        .success()
        .stdout("10.0.0.5/24\n")
        .stderr(predicate::str::contains("host bits set"));
}

#[test]
fn test_host_bits_original_preserved_in_output() {
    ipsort()
        .write_stdin("10.0.0.5/24\n")
        .assert()
        .success()
        .stdout("10.0.0.5/24\n");
}

#[test]
fn test_no_ips_found_exits_nonzero() {
    ipsort()
        .write_stdin("# just a comment\n---\n")
        .assert()
        .failure()
        .stderr(predicate::str::contains("no IP addresses found"));
}

#[test]
fn test_empty_pipe_exits_nonzero() {
    ipsort().write_stdin("").assert().failure();
}

#[test]
fn test_success_exits_zero() {
    ipsort().write_stdin("10.0.0.0/8\n").assert().success();
}

#[test]
fn test_invalid_flag_exits_nonzero() {
    ipsort().args(["--not-a-flag"]).assert().failure();
}

#[test]
fn test_aggregate_two_halves() {
    ipsort()
        .args(["--aggregate"])
        .write_stdin("10.0.0.0/25\n10.0.0.128/25\n")
        .assert()
        .success()
        .stdout("10.0.0.0/24\n");
}

#[test]
fn test_aggregate_short_flag() {
    ipsort()
        .args(["-a"])
        .write_stdin("10.0.0.0/25\n10.0.0.128/25\n")
        .assert()
        .success()
        .stdout("10.0.0.0/24\n");
}

#[test]
fn test_aggregate_four_subnets() {
    ipsort()
        .args(["--aggregate"])
        .write_stdin("10.0.0.0/24\n10.0.1.0/24\n10.0.2.0/24\n10.0.3.0/24\n")
        .assert()
        .success()
        .stdout("10.0.0.0/22\n");
}

#[test]
fn test_aggregate_partial() {
    ipsort()
        .args(["--aggregate"])
        .write_stdin("10.0.0.0/25\n10.0.0.128/25\n192.168.0.0/24\n")
        .assert()
        .success()
        .stdout("10.0.0.0/24\n192.168.0.0/24\n");
}

#[test]
fn test_aggregate_no_aggregation_possible() {
    ipsort()
        .args(["--aggregate"])
        .write_stdin("10.0.0.0/24\n192.168.0.0/24\n")
        .assert()
        .success()
        .stdout("10.0.0.0/24\n192.168.0.0/24\n");
}

#[test]
fn test_aggregate_preserves_decoration() {
    ipsort()
        .args(["--aggregate"])
        .write_stdin("- 10.0.0.0/25\n- 10.0.0.128/25\n")
        .assert()
        .success()
        .stdout("- 10.0.0.0/24\n");
}

#[test]
fn test_aggregate_first_line_wins_decoration() {
    ipsort()
        .args(["--aggregate"])
        .write_stdin("first: 10.0.0.0/25\nsecond: 10.0.0.128/25\n")
        .assert()
        .success()
        .stdout("first: 10.0.0.0/24\n");
}

#[test]
fn test_aggregate_per_block() {
    ipsort()
        .args(["--aggregate"])
        .write_stdin(
            "10.0.0.0/25\n10.0.0.128/25\n\n192.168.0.0/25\n192.168.0.128/25\n",
        )
        .assert()
        .success()
        .stdout("10.0.0.0/24\n\n192.168.0.0/24\n");
}

#[test]
fn test_aggregate_separator_preserved() {
    ipsort()
        .args(["--aggregate"])
        .write_stdin("10.0.0.0/25\n10.0.0.128/25\n# comment\n192.168.0.0/24\n")
        .assert()
        .success()
        .stdout("10.0.0.0/24\n# comment\n192.168.0.0/24\n");
}

#[test]
fn test_aggregate_multi_ip_line_error() {
    ipsort()
        .args(["--aggregate"])
        .write_stdin("10.0.0.0/25 192.168.0.0/24\n10.0.0.128/25\n")
        .assert()
        .failure()
        .stderr(predicate::str::contains("--aggregate"))
        .stderr(predicate::str::contains("10.0.0.0/24"));
}

#[test]
fn test_aggregate_ipv6() {
    ipsort()
        .args(["--aggregate"])
        .write_stdin("2001:db8::/33\n2001:db8:8000::/33\n")
        .assert()
        .success()
        .stdout("2001:db8::/32\n");
}

#[test]
fn test_aggregate_with_reverse() {
    ipsort()
        .args(["--aggregate", "--reverse"])
        .write_stdin(
            "10.0.0.0/25\n10.0.0.128/25\n192.168.0.0/25\n192.168.0.128/25\n",
        )
        .assert()
        .success()
        .stdout("192.168.0.0/24\n10.0.0.0/24\n");
}

#[test]
fn test_check_already_sorted_exits_zero() {
    ipsort()
        .args(["--check"])
        .write_stdin("10.0.0.0/8\n172.16.0.0/12\n192.168.0.0/16\n")
        .assert()
        .success()
        .stdout("");
}

#[test]
fn test_check_unsorted_exits_nonzero() {
    ipsort()
        .args(["--check"])
        .write_stdin("192.168.0.0/16\n10.0.0.0/8\n")
        .assert()
        .failure();
}

#[test]
fn test_check_reports_first_out_of_order_line() {
    ipsort()
        .args(["--check"])
        .write_stdin("192.168.0.0/16\n10.0.0.0/8\n")
        .assert()
        .failure()
        .stderr(predicate::str::contains("192.168.0.0/16"))
        .stderr(predicate::str::contains("line 1"));
}

#[test]
fn test_check_no_stdout_on_failure() {
    ipsort()
        .args(["--check"])
        .write_stdin("192.168.0.0/16\n10.0.0.0/8\n")
        .assert()
        .failure()
        .stdout("");
}

#[test]
fn test_check_no_stdout_on_success() {
    ipsort()
        .args(["--check"])
        .write_stdin("10.0.0.0/8\n192.168.0.0/16\n")
        .assert()
        .success()
        .stdout("");
}

#[test]
fn test_check_short_flag() {
    ipsort()
        .args(["-c"])
        .write_stdin("10.0.0.0/8\n192.168.0.0/16\n")
        .assert()
        .success();
}

#[test]
fn test_check_with_unique_detects_duplicates() {
    ipsort()
        .args(["--check", "--unique"])
        .write_stdin("10.0.0.0/8\n10.0.0.0/8\n192.168.0.0/16\n")
        .assert()
        .failure();
}

#[test]
fn test_check_with_unique_passes_when_no_duplicates() {
    ipsort()
        .args(["--check", "--unique"])
        .write_stdin("10.0.0.0/8\n192.168.0.0/16\n")
        .assert()
        .success();
}

#[test]
fn test_check_with_aggregate_detects_unaggregated() {
    ipsort()
        .args(["--check", "--aggregate"])
        .write_stdin("10.0.0.0/25\n10.0.0.128/25\n")
        .assert()
        .failure();
}

#[test]
fn test_check_with_aggregate_passes_when_aggregated() {
    ipsort()
        .args(["--check", "--aggregate"])
        .write_stdin("10.0.0.0/24\n192.168.0.0/24\n")
        .assert()
        .success();
}

#[test]
fn test_check_with_ips_only_sorted() {
    ipsort()
        .args(["--check", "--ips-only"])
        .write_stdin("- 10.0.0.0/8\n- 192.168.0.0/16\n")
        .assert()
        .success();
}

#[test]
fn test_check_with_ips_only_unsorted() {
    ipsort()
        .args(["--check", "--ips-only"])
        .write_stdin("- 192.168.0.0/16\n- 10.0.0.0/8\n")
        .assert()
        .failure();
}

#[test]
fn test_check_with_block_separators_sorted() {
    ipsort()
        .args(["--check"])
        .write_stdin("# group\n10.0.0.0/8\n192.168.0.0/16\n\n172.16.1.0/24\n172.16.2.0/24\n")
        .assert()
        .success();
}

#[test]
fn test_check_with_block_separators_unsorted() {
    ipsort()
        .args(["--check"])
        .write_stdin("# group\n192.168.0.0/16\n10.0.0.0/8\n")
        .assert()
        .failure();
}

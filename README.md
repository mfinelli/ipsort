# ipsort

![CI](https://github.com/mfinelli/ipsort/actions/workflows/default.yml/badge.svg)

`ipsort` sorts IP addresses and CIDR blocks by their actual numeric value rather
than lexicographically. It handles plain lists, YAML and JSON list items, inline
config values, and other mixed content, preserving surrounding decoration while
reordering only the addresses.

## Features

- Sorts IPv4 and IPv6 addresses numerically
- Preserves decoration: YAML list markers, JSON brackets, inline keys,
  punctuation
- Block separator model: non-IP lines (comments, blank lines) divide input into
  independently sorted groups
- `--inline` mode for sorting IPs spread across multiple lines within a single
  logical unit
- `--unique` deduplication by normalized CIDR
- `--normalize` for canonical network string output
- `--ips-only` and `--ips-only-with-structure` for extracting bare addresses
- Composable in shell pipelines

## Installation

### From a release

Download the binary for your platform from the
[releases page](https://github.com/mfinelli/ipsort/releases), make it
executable, and place it somewhere on your `PATH`:

```sh
chmod +x ipsort
mv ipsort ~/.local/bin/
```

Shell completions and the manpage are included in the release archive.

### From source

Requires Rust and `scdoc` (for the manpage).

```sh
git clone https://github.com/mfinelli/ipsort.git
cd ipsort
make
```

The compiled binary, shell completions, and manpage are written to
`target/release/`. To build and install to your system:

```sh
sudo make install    # installs to /usr/local by default
sudo make uninstall  # removes installed files
```

## Usage

```
ipsort [OPTIONS] [ADDRESS...]
```

Read from stdin when no addresses are given. Use `-` to read explicitly from
stdin.

### Options

| Flag                        | Short | Description                                                          |
| --------------------------- | ----- | -------------------------------------------------------------------- |
| `--reverse`                 | `-r`  | Reverse the sort order                                               |
| `--ipv6-first`              |       | Sort IPv6 before IPv4 in mixed input                                 |
| `--unique`                  | `-u`  | Deduplicate by normalized CIDR, keeping first occurrence             |
| `--inline`                  |       | Sort all IPs globally across the entire input                        |
| `--normalize`               |       | Emit canonical network strings (clears host bits, adds `/32`/`/128`) |
| `--ips-only`                |       | Strip decoration, emit one bare IP per line, discard non-IP lines    |
| `--ips-only-with-structure` |       | Strip decoration, emit one bare IP per line, preserve non-IP lines   |

`--ips-only` and `--ips-only-with-structure` are mutually exclusive.

## Examples

**Sort a list from stdin:**

```sh
$ printf '192.168.1.0/24\n10.0.0.0/8\n172.16.0.0/12\n' | ipsort
10.0.0.0/8
172.16.0.0/12
192.168.1.0/24
```

**Sort positional arguments:**

```sh
$ ipsort 192.168.1.0/24 10.0.0.0/8 172.16.0.0/12
10.0.0.0/8
172.16.0.0/12
192.168.1.0/24
```

**Sort a YAML list, preserving list decoration:**

```sh
$ cat <<EOF | ipsort
- 192.168.1.0/24
- 10.0.0.0/8
- 172.16.0.0/12
EOF
- 10.0.0.0/8
- 172.16.0.0/12
- 192.168.1.0/24
```

**Sort with block separators preserved:**

```sh
$ cat <<EOF | ipsort
# internal ranges
192.168.1.0/24
10.0.0.0/8

# dmz
172.16.2.0/24
172.16.1.0/24
EOF
# internal ranges
10.0.0.0/8
192.168.1.0/24

# dmz
172.16.1.0/24
172.16.2.0/24
```

**Sort IPs from a JSON field using `jq`:**

```sh
$ jq '.networks[]' config.json | ipsort
```

**Sort a multi-line YAML value with `--inline`:**

```sh
$ cat <<EOF | ipsort --inline
allowed_ips: 192.168.1.0/24 10.0.0.0/8
  172.16.2.0/24 172.16.1.0/24
EOF
allowed_ips: 10.0.0.0/8 172.16.1.0/24
  172.16.2.0/24 192.168.1.0/24
```

**Deduplicate a list:**

```sh
$ printf '10.0.0.0/8\n192.168.0.0/16\n10.0.0.0/8\n' | ipsort --unique
10.0.0.0/8
192.168.0.0/16
```

**Normalize to canonical form:**

```sh
$ printf '10.0.0.5/24\n192.168.1.1\n' | ipsort --normalize
10.0.0.0/24
192.168.1.1/32
```

**Extract bare IPs, discarding all decoration and structure:**

```sh
$ cat <<EOF | ipsort --ips-only
# group one
- 192.168.1.0/24
- 10.0.0.0/8

# group two
- 172.16.0.0/12
EOF
10.0.0.0/8
172.16.0.0/12
192.168.1.0/24
```

**Sort in reverse:**

```sh
$ ipsort --reverse 10.0.0.0/8 172.16.0.0/12 192.168.1.0/24
192.168.1.0/24
172.16.0.0/12
10.0.0.0/8
```

## Input format

`ipsort` tokenizes each line by splitting on any character that cannot appear in
a valid CIDR address, so it handles all common delimiter styles without
configuration:

| Input style          | Example                          |
| -------------------- | -------------------------------- |
| Space-separated      | `10.0.0.0/8 192.168.0.0/16`      |
| Comma-separated      | `10.0.0.0/8,192.168.0.0/16`      |
| Quoted JSON-style    | `"10.0.0.0/8", "192.168.0.0/16"` |
| YAML list items      | `- 10.0.0.0/8`                   |
| Inline config values | `network: 10.0.0.0/8`            |

Bare IP addresses (without a prefix length) are accepted and treated as `/32` or
`/128` for sorting. CIDRs with host bits set (e.g. `10.0.0.5/24`) are accepted
with a warning to stderr (host bits are cleared for sorting), and the original
string is preserved in output unless `--normalize` is set.

## Contributing

Contributions are welcome. The codebase is structured as a library
(`src/lib.rs`) with a thin CLI binary (`src/main.rs`). The library modules are:

- `parse`: token-level CIDR parsing
- `classify`: line spanification and classification
- `sort`: sort comparator and options
- `blocks`: block-level and inline sorting, deduplication
- `output`: output rendering

For a full account of the design decisions, requirements, and tradeoffs behind
the implementation, see `DESIGN.md`.

Run the test suite with:

```sh
cargo test
```

## License

Licensed under the GNU GPL version 3.0 or later.

```
ipsort: versitile ip address sorting tool
Copyright 2026 Mario Finelli

This program is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with this program.  If not, see <https://www.gnu.org/licenses/>.
```

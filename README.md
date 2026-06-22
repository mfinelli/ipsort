# ipsort

![CI](https://github.com/mfinelli/ipsort/actions/workflows/default.yml/badge.svg)

`ipsort` sorts IP addresses and CIDR blocks by their actual numeric value rather
than lexicographically. It handles plain lists, YAML and JSON list items, inline
config values, and other mixed content, preserving surrounding decoration while
reordering only the addresses.

> [!NOTE]
>
> This project is feature-complete and in low-activity maintenance mode. I
> actively monitor issues for bugs or feature requests and keep dependencies
> updated, but no major active development is planned.

## Features

- Sorts IPv4 and IPv6 addresses numerically
- Preserves decoration: YAML list markers, JSON brackets, inline keys,
  punctuation
- Block separator model: non-IP lines (comments, blank lines) divide input into
  independently sorted groups
- `--inline` mode for sorting IPs spread across multiple lines within a single
  logical unit
- `--aggregate` to merge adjacent CIDRs into their minimal supernet
  representation
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

### From crates.io

You need to have [cargo](https://doc.rust-lang.org/stable/cargo/) installed and
then you can install [ipsort](https://crates.io/crates/ipsort) directly from
[crates.io](https://crates.io):

```sh
cargo install ipsort
```

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
| `--aggregate`               | `-a`  | Merge adjacent CIDRs into their minimal supernet representation      |
| `--check`                   | `-c`  | Exit 0 if input is already sorted/aggregated/unique, 1 otherwise     |
| `--inline`                  | `-i`  | Sort all IPs globally across the entire input                        |
| `--normalize`               | `-n`  | Emit canonical network strings (clears host bits, adds `/32`/`/128`) |
| `--ips-only`                |       | Strip decoration, emit one bare IP per line, discard non-IP lines    |
| `--ips-only-with-structure` |       | Strip decoration, emit one bare IP per line, preserve non-IP lines   |

`--ips-only` and `--ips-only-with-structure` are mutually exclusive.

### Exit codes

| Code | Meaning                                                            |
| ---- | ------------------------------------------------------------------ |
| `0`  | Success                                                            |
| `1`  | `--check` ran successfully; input is not in the expected state     |
| `2`  | Operational error (no input, no IPs found, conflicting flag usage) |

## Examples

**Sort a list from stdin:**

```sh
$ cat <<EOF | ipsort
192.168.1.0/24
10.0.0.0/8
172.16.0.0/12
EOF
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

**Read from a file using shell redirection:**

```sh
$ ipsort < addresses.txt
```

**Sort a file in place using `sponge` from `moreutils`:**

```sh
$ ipsort < addresses.txt | sponge addresses.txt
```

Without `sponge`, redirecting to the same file you are reading from will
truncate it before `ipsort` reads it. `sponge` buffers the output and writes it
only after the input is fully read.

**Sort an entire YAML file, leaving all structure intact:**

```sh
$ cat firewall.yml
# firewall rules
allowed_sources:
  - 192.168.1.0/24
  - 10.0.0.0/8
  - 172.16.5.0/24

denied_sources:
  - 10.99.0.0/16
  - 192.168.99.0/24

$ ipsort < firewall.yml
# firewall rules
allowed_sources:
  - 10.0.0.0/8
  - 172.16.5.0/24
  - 192.168.1.0/24

denied_sources:
  - 10.99.0.0/16
  - 192.168.99.0/24
```

Comments, blank lines, and YAML keys are all preserved exactly. Only the IP
addresses are reordered, independently within each block separated by blank
lines.

**Sort IPs in a Kubernetes ingress annotation with `--inline`:**

```sh
$ ipsort --inline < ingress.yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: my-service
  annotations:
    nginx.ingress.kubernetes.io/whitelist-source-range: >-
      10.0.0.0/8,
      10.99.0.0/16,
      172.16.5.0/24,
      192.168.1.0/24
spec:
  rules:
  - host: my-service.example.com
```

The entire YAML structure is preserved. `--inline` treats all IPs across the
file as one pool, which is what you want when a list spans multiple lines within
a single annotation value.

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
$ cat <<EOF | ipsort --unique
10.0.0.0/8
192.168.0.0/16
10.0.0.0/8
EOF
10.0.0.0/8
192.168.0.0/16
```

**Aggregate subnets into their minimal supernet:**

```sh
$ cat <<EOF | ipsort --aggregate
- 10.0.0.0/25
- 10.0.0.128/25
- 192.168.0.0/24
EOF
- 10.0.0.0/24
- 192.168.0.0/24
```

**Normalize to canonical form:**

```sh
$ cat <<EOF | ipsort --normalize
10.0.0.5/24
192.168.1.1
EOF
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

**Check whether a file is already sorted (useful in CI):**

```sh
$ ipsort --check < addresses.txt && echo "sorted" || echo "not sorted"
```

With other flags, checks whether the input satisfies those conditions:

```sh
$ ipsort --check --unique < addresses.txt  # exits 1 if duplicates exist
$ ipsort --check --aggregate < addresses.txt  # exits 1 if CIDRs can be merged
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

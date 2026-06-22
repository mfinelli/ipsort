ipsort(1) "v__VERSION__" "ipsort __DATE__"

# NAME

ipsort - sort IP addresses and CIDRs numerically

# SYNOPSIS

*ipsort* [OPTIONS] [_ADDRESS_...]

# DESCRIPTION

*ipsort* sorts IP addresses and CIDR blocks by their actual numeric value
rather than lexicographically. It is designed to be flexible with input
formats: it can sort plain lists of addresses, YAML or JSON list items,
inline config values, and other mixed content while preserving surrounding
decoration.

Input is read from stdin when no addresses are given, or from positional
arguments otherwise. Use *-* as the sole argument to read explicitly from
stdin.

IPv4 and IPv6 addresses are both supported. Mixed input is sorted with IPv4
first by default.

Bare IP addresses without a prefix length are accepted and treated as _/32_
(IPv4) or _/128_ (IPv6) for sorting purposes.

Lines containing no recognizable IP address act as block separators: each
group of IP lines between separators is sorted independently, with separators
preserved in place.

Because non-IP content is always preserved verbatim, *ipsort* can be given an
entire YAML, TOML, or config file as input; only the IP addresses will be
reordered, leaving comments, keys, blank lines, and all other content untouched.

# OPTIONS

*-r*, *--reverse*
	Reverse the sort order.

*--ipv6-first*
	Sort IPv6 addresses before IPv4 in mixed input. Default is IPv4 first.

*-u*, *--unique*
	Remove duplicate addresses. Comparison is by normalized CIDR (canonical
	network address plus prefix length). The first occurrence is kept.

	For single-IP lines, duplicate lines are silently dropped. For multi-IP
	lines in default mode, encountering a duplicate is an error: the ambiguity
	of which IP to remove requires the user to clean up the input. When
	*--ips-only* or *--ips-only-with-structure* is set, each IP on a multi-IP
	line is checked independently and duplicates are removed without error.

*-i*, *--inline*
	Collect all IP addresses from the entire input into one pool, sort
	globally, then redistribute sorted IPs back into their original positions.
	Decoration and surrounding content are preserved. Block separator logic
	does not apply (the entire input is treated as one sort scope).

	Useful when IPs are spread across multiple lines within a single logical
	unit, such as a multi-line YAML value.

*-n*, *--normalize*
	Emit canonical network strings instead of the original token. Host bits are
	cleared (_10.0.0.5/24_ becomes _10.0.0.0/24_) and bare IPs gain explicit
	prefix lengths (_192.168.1.1_ becomes _192.168.1.1/32_). Surrounding
	decoration is unaffected.

*--ips-only*
	Strip all non-IP content and emit one bare IP address per line. Each IP
	address in the input becomes one output line. Non-IP lines (comments, blank
	lines, etc.) are discarded entirely.

*--ips-only-with-structure*
	Like *--ips-only*, but preserves non-IP lines as block separators. Each IP
	address becomes one output line, decoration is stripped, and block
	separator logic applies normally.

	*--ips-only* and *--ips-only-with-structure* are mutually exclusive.

*-c*, *--check*
	Check whether the input is already in the state the current flags would
	produce. Exits 0 if no changes would be made, 1 otherwise. No output is
	written to stdout. The first out-of-order or unsatisfied line is reported
	to stderr on failure.

	Combines naturally with other flags: *--check --unique* exits 1 if
	duplicates exist; *--check --aggregate* exits 1 if any CIDRs can be
	merged; *--check --normalize* exits 1 if any token is not in canonical
	form.

*-a*, *--aggregate*
	Merge adjacent CIDR blocks into their minimal supernet representation.
	For each group of CIDRs that can be combined, the first line in the group
	becomes the output line with the aggregated CIDR substituted in; remaining
	lines in the group are dropped. Surrounding decoration on the winning line
	is preserved.

	Aggregation is applied per block. Use *--inline* for global aggregation
	across the entire input.

	For multi-IP lines involved in a cross-line aggregation, *--aggregate*
	returns an error. Clean up the input and try again.

	Subsumes *--unique*: if *--aggregate* is set, *--unique* has no additional
	effect.

# INPUT FORMAT

*ipsort* tokenizes each line by splitting on any character that cannot appear
in a valid CIDR address. This means it handles all common delimiter styles
without configuration:

- Space-separated: _10.0.0.0/8 192.168.0.0/16_
- Comma-separated: _10.0.0.0/8,192.168.0.0/16_
- Quoted JSON-style: _"10.0.0.0/8", "192.168.0.0/16"_
- YAML list items: _- 10.0.0.0/8_
- Inline config values: _network: 10.0.0.0/8_

Tokens that look like CIDRs but fail to parse (e.g. _10.0.0.1/33_) are
treated as non-IP content and passed through with a warning to stderr.

CIDRs with host bits set (e.g. _10.0.0.5/24_) are accepted. The host bits
are cleared for sorting purposes and a warning is emitted to stderr. The
original string is preserved in output unless *--normalize* is set.

# OUTPUT FORMAT

By default, output mirrors the input format exactly. Decoration, delimiters,
and surrounding content are preserved. Only the IP addresses are reordered.

For example, _"192.168.0.0/16", "10.0.0.0/8"_ becomes
_"10.0.0.0/8", "192.168.0.0/16"_ (the quotes and comma are untouched).

For structured formats (JSON, YAML), the recommended pattern is to extract
fields with *jq*(1) or *yq*(1), pipe through *ipsort*, and reconstruct
downstream.

# EXIT STATUS

*0*
	Success.

*1*
	*--check* ran successfully and determined the input is not in the expected
	state.

*2*
	Operational error: no input provided, no IP addresses found, *--unique*
	encountered an ambiguous duplicate on a multi-IP line, *--aggregate*
	encountered a multi-IP line involved in a cross-line aggregation, or
	invalid arguments (reported by the argument parser).

# EXAMPLES

Sort a list of addresses from stdin:

```
$ cat <<EOF | ipsort
192.168.1.0/24
10.0.0.0/8
172.16.0.0/12
EOF
10.0.0.0/8
172.16.0.0/12
192.168.1.0/24
```

Sort positional arguments:

```
$ ipsort 192.168.1.0/24 10.0.0.0/8 172.16.0.0/12
10.0.0.0/8
172.16.0.0/12
192.168.1.0/24
```

Sort a YAML list, preserving list decoration:

```
$ cat <<EOF | ipsort
- 192.168.1.0/24
- 10.0.0.0/8
- 172.16.0.0/12
EOF
- 10.0.0.0/8
- 172.16.0.0/12
- 192.168.1.0/24
```

Sort with block separators preserved:

```
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

Sort IPs from a JSON field using *jq*(1):

```
$ jq '.networks[]' config.json | ipsort
```

Sort a multi-line YAML value with *--inline*:

```
$ cat <<EOF | ipsort --inline
allowed_ips: 192.168.1.0/24 10.0.0.0/8
  172.16.2.0/24 172.16.1.0/24
EOF
allowed_ips: 10.0.0.0/8 172.16.1.0/24
  172.16.2.0/24 192.168.1.0/24
```

Deduplicate a list:

```
$ cat <<EOF | ipsort --unique
10.0.0.0/8
192.168.0.0/16
10.0.0.0/8
EOF
10.0.0.0/8
192.168.0.0/16
```

Normalize output to canonical form:

```
$ cat <<EOF | ipsort --normalize
10.0.0.5/24
192.168.1.1
EOF
10.0.0.0/24
192.168.1.1/32
```

Extract bare IPs from decorated input:

```
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

Sort in reverse order:

```
$ ipsort --reverse 10.0.0.0/8 172.16.0.0/12 192.168.1.0/24
192.168.1.0/24
172.16.0.0/12
10.0.0.0/8
```

Check whether a file is already sorted:

```
$ ipsort --check < addresses.txt && echo "sorted" || echo "not sorted"
```

Check whether a file has no duplicates:

```
$ ipsort --check --unique < addresses.txt
```

Aggregate subnets into their minimal supernet:

```
$ cat <<EOF | ipsort --aggregate
10.0.0.0/25
10.0.0.128/25
10.0.1.0/24
192.168.0.0/24
EOF
10.0.0.0/23
192.168.0.0/24
```

Aggregate a YAML list, preserving decoration on the winning line:

```
$ cat <<EOF | ipsort --aggregate
- 10.0.0.0/25
- 10.0.0.128/25
- 192.168.0.0/24
EOF
- 10.0.0.0/24
- 192.168.0.0/24
```

Read from a file using shell redirection:

```
$ ipsort < addresses.txt
```

Sort a file in place using *sponge*(1) from *moreutils*:

```
$ ipsort < addresses.txt | sponge addresses.txt
```

Without *sponge*, redirecting output to the same file being read will truncate
it before *ipsort* reads it. *sponge* buffers the full output before writing.

Sort an entire YAML file, leaving all structure intact:

```
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

Comments, blank lines, and YAML keys are preserved exactly. IPs are sorted
independently within each block separated by blank lines.

Sort IPs in a Kubernetes ingress annotation with *--inline*:

```
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

The entire YAML structure is preserved. *--inline* treats all IPs across the
file as one pool, sorting them globally while leaving everything else in place.

# NOTES

When an IP is removed from a multi-IP line by *--unique* in *--ips-only* or
*--ips-only-with-structure* mode, adjacent delimiters (spaces, commas, etc.)
that were between IPs may remain in the output. Users who need clean output
in this case should use *--ips-only* to strip all decoration.

# SEE ALSO

*sort*(1), *jq*(1), *uniq*(1), *sponge*(1)

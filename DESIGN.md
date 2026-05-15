# `ipsort` — Requirements & Design Document

## Overview

`ipsort` is a CLI tool that sorts IP addresses (expressed as CIDRs) by their actual numeric value rather than lexicographically. It is designed to be flexible with input formats, tolerant of mixed content, and composable in shell pipelines.

---

## Input

### Sources

- **Stdin** — piped or redirected input
- **Positional arguments** — one or more arguments on the command line
- **No file arguments** — file input is explicitly out of scope; users should use shell redirection (`ipsort < file.txt`)

Stdin and positional arguments are mutually exclusive; if both are provided then ipsort exits with a non-zero exit code.

### Positional Argument Modes

- **Single argument**: a comma-separated or space-separated list of CIDRs (e.g. `ipsort "10.0.0.0/8, 192.168.0.0/16"`)
- **Multiple arguments**: each argument is a single CIDR (e.g. `ipsort 10.0.0.0/8 192.168.0.0/16`)

### Address Format

- All addresses are treated as CIDRs
- Bare IP addresses without a prefix length are treated as `/32` (IPv4) or `/128` (IPv6)
- CIDRs with host bits set (e.g. `10.0.0.5/24`) are accepted — see *Parsing* for handling

### Multiline Input

Multiline input is fully supported. Each line is processed according to the rules below.

---

## Parsing

### Line Classification

Each line of input falls into one of three categories:

1. **IP-only line** — the entire line is a single CIDR (possibly with surrounding whitespace)
2. **Mixed line** — the line contains one or more CIDRs alongside other non-IP content (e.g. `- 10.0.0.0/8` or `somekey: 10.0.0.0/8 192.168.0.0/16`)
3. **Non-IP line** — the line contains no recognizable IP address or CIDR (e.g. a YAML comment, a blank line, a key with a non-IP value)

### Mixed Lines

- The **whole line is the unit of output** — decoration and surrounding content are preserved exactly
- The line is **sorted into position** based on the lowest IP found on that line (after intra-line sorting)
- If a line contains **multiple IPs**, those IPs are sorted within the line first, then the line is sorted into the global output by its lowest IP

### Non-IP Lines — Block Separator Model

- Lines containing no IP address are treated as **block separators**
- Each separator (or run of consecutive non-IP lines) divides the input into independent sort regions; each region is sorted separately
- The separator block stays in place between its neighboring IP groups
- This means a YAML comment or blank line between two groups of IPs keeps those groups sorted independently, with the comment preserved between them

**Rationale**: this is the most predictable behavior when sorting structured documents or config snippets. It avoids non-IP lines "floating" to the top or being dropped, and matches user intuition when copy-pasting chunks of config from an editor.

### Host Bits Set

- e.g. `10.0.0.5/24` — technically malformed (host bits are set outside the prefix)
- **Behavior**: normalize to the network address (`10.0.0.0/24`) for sort purposes; preserve the original string in output
- **Emit a warning to stderr**

### Malformed CIDRs

- e.g. `10.0.0.1/33`, `999.0.0.1/24` — not parseable as a valid CIDR
- **Behavior**: treat as non-IP content (passthrough), emit a warning to stderr
- **Rationale**: hard erroring would break pipelines; silent passthrough could mask typos. Stderr warning threads the needle.

### Nothing Parseable

- If the entire input contains no recognizable IP address, `ipsort` exits with a non-zero error code.

---

## Sorting

### Primary Sort Key

- **Network address**, numeric (not lexicographic) — IPv4 as a 32-bit integer, IPv6 as a 128-bit integer

### Tie-breaking (same network address, different prefix length)

- Sort by **prefix length ascending** — shorter prefix (larger block) comes first
- e.g. `10.0.0.0/8` before `10.0.0.0/24`
- **Rationale**: matches how routing tables are conventionally read; broader blocks before their subnets feels natural when scanning

### IPv4 vs IPv6

- Both are supported
- **Default**: IPv4 addresses sort before IPv6 addresses in mixed input
- **Flag**: `--ipv6-first` reverses this, placing IPv6 before IPv4

### Sort Scope (normal mode)

- Sorting is applied **per block** — each group of IP lines between non-IP separators is sorted independently

---

## `--inline` Mode

### Purpose

Handles the case where IPs are spread across multiple lines within a single logical unit — e.g. a multi-line YAML value:

```yaml
somekey: 11.0.0.0/8 12.0.0.0/8
  10.0.0.0/8 13.0.0.0/8
```

In this case the user wants all four IPs sorted and redistributed across the original line positions.

### Behavior

- All IP tokens across the **entire input** are collected into one pool and sorted globally
- IPs are reinserted into their original token positions in document order
- Non-IP content on each line (decoration, keys, punctuation) is preserved exactly
- Lines that become "empty" of IPs after reflow are **kept in place** — lines are never dropped, to avoid mangling surrounding document structure
- Block separator logic does **not** apply in `--inline` mode (the whole input is one sort scope)
- `--unique` deduplication (if enabled) applies across the entire reflowed pool

**Rationale**: dropping lines silently could corrupt a YAML or config document in ways that are hard to detect. Preserving the line structure keeps the output safe to paste back into the source.

---

## Output

### Default Format — Mirror Input

- The output format mirrors the input format
- If the input was comma-separated, output is comma-separated
- If the input was space-separated, output is space-separated
- Line decoration (e.g. `- `, JSON brackets, inline YAML keys) is preserved on each line

**Rationale**: `ipsort` is intended to be used in workflows like `jq .field[] | ipsort | ...` where the tool is one step in a pipeline. Mangling the format would break downstream consumers. For structured formats (JSON, YAML), the recommended pattern is to use `jq` or `yq` to extract fields first, pipe through `ipsort`, and reconstruct — rather than having `ipsort` parse and re-serialize structured formats itself.

### `--one-per-line`

- Override output format: always emit one address per line regardless of input format

### Deduplication: `--unique` / `-u`

- Remove duplicate addresses from output
- Comparison is by **normalized CIDR**: canonical network address + prefix length
  - e.g. `10.0.0.5/24` and `10.0.0.0/24` are considered equal after normalization
  - `10.0.0.0/8` and `10.0.0.0/24` are **not** equal (different prefix length)
- When deduplicating, the **first occurrence** is kept
- Applies globally, including in `--inline` mode
- **Rationale**: comparing original strings is too strict (misses formatting variants); comparing only network address would incorrectly collapse different-sized blocks. Normalized CIDR is the right unit of identity.

---

## Flags Summary

| Flag | Description |
|---|---|
| `--inline` | Reorder all IP tokens freely across the entire input rather than sorting line-by-line |
| `--unique` / `-u` | Deduplicate by normalized CIDR, keeping first occurrence |
| `--one-per-line` | Always output one address per line, overriding input format mirroring |
| `--ipv6-first` | In mixed IPv4/IPv6 input, sort IPv6 addresses before IPv4 |

---

## Design Decisions & Tradeoffs

### Why not parse JSON/YAML natively?

Full structured format support would require parsing and re-serializing, which risks mangling comments, key ordering, and formatting. The idiomatic alternative — `jq`/`yq` to extract, `ipsort` to sort, reconstruct downstream — is more composable and keeps `ipsort` focused. Heuristic decoration preservation handles the 95% case (copy-pasted config snippets, YAML lists) without the complexity.

### Why block separators instead of global sort?

Global sorting of a mixed document would move lines across non-IP boundaries, breaking the structure of the surrounding content. Block separators give the user predictable, local sorting that respects document structure. `--inline` is the explicit opt-in for cases where global reflow is actually wanted.

### Why preserve lines emptied by `--inline` reflow?

Dropping lines silently could corrupt a YAML or config document in ways that are hard to detect. Preserving empty line structure keeps the output safe to paste back.

### Why warn on host-bits-set rather than error?

These addresses appear in real-world configs (copy-paste errors, routing table exports). Hard erroring would make `ipsort` brittle in pipelines. A stderr warning preserves the pipeline while surfacing the issue.

### Why keep duplicates by default?

Input may be intentionally duplicated (e.g. two separate config blocks that both reference the same address). Silent deduplication would be a destructive surprise. `--unique` is the explicit opt-in.

### Why prefix-length ascending on tie-break?

Matches routing table conventions. When you see `10.0.0.0/8` and `10.0.0.0/24` together, the broad block contextualizes the specific one. Ascending prefix length (larger block first) reads naturally top-to-bottom.

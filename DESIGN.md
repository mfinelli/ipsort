# `ipsort` — Requirements & Design Document

## Overview

`ipsort` is a CLI tool that sorts IP addresses (expressed as CIDRs) by their actual numeric value rather than lexicographically. It is designed to be flexible with input formats, tolerant of mixed content, and composable in shell pipelines.

---

## Input

### Sources

- **Stdin** — piped or redirected input
- **Positional arguments** — one or more arguments on the command line
- **No file arguments** — file input is explicitly out of scope; users should use shell redirection (`ipsort < file.txt`)

Stdin and positional arguments are mutually exclusive. If both are provided, `ipsort` exits with a non-zero error code.

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

### Span Model

Lines are not classified into fixed categories. Instead, each line is decomposed into an ordered sequence of **spans** — alternating runs of IP tokens and non-IP content:

- **`Ip` span** — a recognized CIDR or bare IP address token
- **`NonIp` span** — any surrounding content (decoration, punctuation, whitespace, keys, comments)

Adjacent non-IP content is always merged into a single `NonIp` span. IP candidates that fail to parse (e.g. version numbers like `1.2.3.4.5`) are absorbed into adjacent `NonIp` spans.

For example, `"- 192.168.1.0/24 10.0.0.0/8"` produces:

```
[NonIp("- "), Ip("192.168.1.0/24"), NonIp(" "), Ip("10.0.0.0/8")]
```

This span representation fully describes the line structure, enabling output reconstruction that reorders `Ip` spans while leaving `NonIp` spans exactly in place.

### Line Classification

After spanification, lines fall into two categories:

- **`HasIp`** — the line contains at least one `Ip` span. It carries a `sort_key` (the lowest IP on the line), the full span list, and any warnings.
- **`NoIp`** — the line contains no `Ip` spans. It acts as a block separator and is preserved verbatim.

### Token Extraction

Tokens are identified by splitting on any character that cannot appear inside a valid CIDR — the set `[0-9a-fA-F.:/]`. Everything outside this set is non-IP content. This handles all common delimiter styles without a regex:

- `10.0.0.0/8 192.168.0.0/16` → two IP tokens, one NonIp (the space)
- `10.0.0.0/8,192.168.0.0/16` → two IP tokens, one NonIp (the comma)
- `"10.0.0.0/8", "192.168.0.0/16"` → two IP tokens, NonIp spans for the surrounding punctuation
- `- 10.0.0.0/8` → one IP token, one NonIp (`"- "`)

### Non-IP Lines — Block Separator Model

- Lines containing no IP address (`NoIp`) act as **block separators**
- Each separator (or run of consecutive non-IP lines) divides the input into independent sort regions; each region is sorted separately
- The separator block stays in place between its neighboring IP groups

**Rationale**: this is the most predictable behavior when sorting structured documents or config snippets. It avoids non-IP lines "floating" to the top or being dropped, and matches user intuition when copy-pasting chunks of config from an editor.

### Sort Key

When a line has multiple IP addresses, the `sort_key` is the lowest IP on the line, determined by the same comparator used for inter-line sorting. This determines where the line is positioned within its block.

### Host Bits Set

- e.g. `10.0.0.5/24` — technically malformed (host bits are set outside the prefix)
- **Sort behavior**: normalized to the network address (`10.0.0.0/24`) for sort purposes
- **Output behavior**: the original string is preserved by default; `--normalize` emits the canonical form
- **Always emits a warning to stderr**

### Malformed CIDRs

- e.g. `10.0.0.1/33`, `999.0.0.1/24` — not parseable as a valid CIDR
- **Behavior**: treated as non-IP content (absorbed into a `NonIp` span), warning emitted to stderr
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

- Sorting is applied **per block** — each group of `HasIp` lines between `NoIp` separators is sorted independently

---

## `--inline` Mode

### Purpose

Handles the case where IPs are spread across multiple lines within a single logical unit — e.g. a multi-line YAML value:

```yaml
somekey: 11.0.0.0/8 12.0.0.0/8
  10.0.0.0/8 13.0.0.0/8
```

In this case the user wants all four IPs sorted and redistributed across the original token positions.

### Behavior

- All `Ip` spans across the **entire input** are collected into one pool and sorted globally
- IPs are reinserted into their original span positions in document order
- `NonIp` spans on each line are preserved exactly
- Lines that become "empty" of IPs after reflow are **kept in place** — lines are never dropped, to avoid mangling surrounding document structure
- Block separator logic does **not** apply in `--inline` mode (the whole input is one sort scope)
- `--unique` deduplication (if enabled) applies across the entire reflowed pool

**Rationale**: dropping lines silently could corrupt a YAML or config document in ways that are hard to detect. Preserving the line structure keeps the output safe to paste back into the source.

---

## Output

### Default Format — Mirror Input

- Output mirrors the input format exactly — decoration, delimiters, and surrounding content are preserved via the span model
- `NonIp` spans are always emitted verbatim
- `Ip` spans are replaced with the sorted IP token (original string by default, canonical form with `--normalize`)

**Rationale**: `ipsort` is intended to be used in workflows like `jq .field[] | ipsort | ...` where the tool is one step in a pipeline. Mangling the format would break downstream consumers. For structured formats (JSON, YAML), the recommended pattern is to use `jq` or `yq` to extract fields first, pipe through `ipsort`, and reconstruct — rather than having `ipsort` parse and re-serialize structured formats itself.

### `--ips-only`

- Strip all `NonIp` spans and emit one bare IP address per line
- Decoration, keys, punctuation, and surrounding text are discarded
- Each `Ip` span becomes one output line
- **Rationale**: `--one-per-line` was the original name but was misleading — the flag doesn't just change line formatting, it strips all non-IP content entirely

### `--normalize`

- Emit the canonical network string for each IP rather than the original token
- Host bits are cleared: `10.0.0.5/24` → `10.0.0.0/24`
- Bare IPs get explicit prefix lengths: `192.168.1.1` → `192.168.1.1/32`
- Does not affect `NonIp` spans, which are always emitted verbatim

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
| `--inline` | Reorder all IP tokens freely across the entire input rather than sorting per block |
| `--unique` / `-u` | Deduplicate by normalized CIDR, keeping first occurrence |
| `--ips-only` | Strip all non-IP content and emit one bare address per line |
| `--normalize` | Emit canonical network strings (clears host bits, adds `/32`/`/128` to bare IPs) |
| `--reverse` | Reverse the sort order |
| `--ipv6-first` | In mixed IPv4/IPv6 input, sort IPv6 addresses before IPv4 |

---

## Internal Implementation

### Module Structure

```
src/
  main.rs       — binary entry point; thin wrapper over the library
  lib.rs        — library root; declares all modules
  parse.rs      — token-level CIDR parsing (ParsedToken, parse_token, is_cidr_char)
  classify.rs   — line spanification and classification (Span, ClassifiedLine, classify_line)
  sort.rs       — sort comparator and options (SortOptions, compare)
  blocks.rs     — block-level sorting (sort_blocks)
  output.rs     — output reconstruction (OutputOptions, render_line)
```

### Key Types

- **`ParsedToken`** — the result of parsing a single token: `ValidCidr`, `BareIp`, or `NotAnIp`. Carries both the original string and the normalized `IpNet`.
- **`Span`** — a single span within a line: `Ip(ParsedToken)` or `NonIp(String)`.
- **`ClassifiedLine`** — a fully classified line: `HasIp { spans, sort_key, warnings }` or `NoIp(String)`.
- **`SortOptions`** — runtime sort configuration: `ipv6_first`, `reverse`.
- **`OutputOptions`** — runtime output configuration: `normalize`, `ips_only`.

### Dependency Direction

```
main / lib
  ↓
output  ←──────────────┐
  ↓                    │
blocks                 │
  ↓                    │
classify ──────────────┘
  ↓
sort
  ↓
parse
```

No circular dependencies. `output` imports from both `classify` and `sort`. `blocks` imports from `classify` and `sort`. `classify` imports from `parse` and `sort`. `sort` and `parse` have no internal dependencies.

---

## Design Decisions & Tradeoffs

### Why the span model instead of line categories?

The original design classified lines into IP-only, mixed, and non-IP categories. During implementation it became clear that output reconstruction requires knowing the exact position of every IP token and every piece of surrounding content. The span model captures this directly — a line is just a sequence of `Ip` and `NonIp` spans — making reconstruction trivial and eliminating a separate classification step.

### Why not parse JSON/YAML natively?

Full structured format support would require parsing and re-serializing, which risks mangling comments, key ordering, and formatting. The idiomatic alternative — `jq`/`yq` to extract, `ipsort` to sort, reconstruct downstream — is more composable and keeps `ipsort` focused. The span model handles the 95% case (copy-pasted config snippets, YAML lists) without the complexity.

### Why block separators instead of global sort?

Global sorting of a mixed document would move lines across non-IP boundaries, breaking the structure of the surrounding content. Block separators give the user predictable, local sorting that respects document structure. `--inline` is the explicit opt-in for cases where global reflow is actually wanted.

### Why preserve lines emptied by `--inline` reflow?

Dropping lines silently could corrupt a YAML or config document in ways that are hard to detect. Preserving empty line structure keeps the output safe to paste back.

### Why `--ips-only` instead of `--one-per-line`?

`--one-per-line` implied only a formatting change. The actual behavior — stripping all non-IP decoration and emitting one bare address per output line — is more accurately described as extraction, not reformatting. `--ips-only` names what you get.

### Why preserve original token strings by default?

Users piping content through `ipsort` expect their data back in the same form they gave it. Silent normalization (e.g. turning `10.0.0.5/24` into `10.0.0.0/24`) would be a surprising mutation. `--normalize` is the explicit opt-in for canonical output.

### Why warn on host-bits-set rather than error?

These addresses appear in real-world configs (copy-paste errors, routing table exports). Hard erroring would make `ipsort` brittle in pipelines. A stderr warning preserves the pipeline while surfacing the issue.

### Why keep duplicates by default?

Input may be intentionally duplicated (e.g. two separate config blocks that both reference the same address). Silent deduplication would be a destructive surprise. `--unique` is the explicit opt-in.

### Why prefix-length ascending on tie-break?

Matches routing table conventions. When you see `10.0.0.0/8` and `10.0.0.0/24` together, the broad block contextualizes the specific one. Ascending prefix length (larger block first) reads naturally top-to-bottom.

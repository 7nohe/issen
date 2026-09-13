# Changelog

All notable changes to issen are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html) as described under
[Versioning](README.md#versioning).

## [Unreleased]

## [0.1.0] - 2026-09-13

First release.

### Linting

- 17 rules ported from `textlint-rule-preset-ja-technical-writing` 12.0.2, with
  the preset's thresholds as defaults. Over 950 Japanese documents issen
  reproduces 97.9% of what textlint reports on these rules; the measurement and
  every known difference are in [COMPATIBILITY.md](COMPATIBILITY.md).
- Three document validators, inactive until configured: `required-headings`,
  `forbidden` (literal or `/regex/flags`), and `terminology`.
- Morphological analysis by Lindera 6.0.0 with IPADIC compiled into the binary.
  No external process, dictionary download, or network access at run time.

### Output

- `--format text` for people, `--format json` for agents, and
  `--format textlint` for textlint's own formatters and for diffing the two.
- Every position and length is in UTF-16 code units, as textlint and LSP use.

### Fixing

- `--fix` applies every available fix, re-lints, and repeats until nothing is
  left to fix. A fix that would cross inline Markdown syntax is withheld and the
  finding is still reported.
- `--stdin --fix` writes the fixed document to stdout and the report to stderr.

### Configuration

- `issen.yml`, merged key by key over the preset, so changing a rule's severity
  keeps its thresholds.
- `gate.fail-on` and `gate.max-warnings` decide the exit code. Unknown rule
  names and gate values are rejected rather than ignored.

### Distribution

- Prebuilt archives for Linux (x86_64 and aarch64, each linked against glibc
  and statically against musl), macOS (Apple silicon and Intel), and Windows
  (x86_64). Each carries the binary, `LICENSE`, and `NOTICE`, whose IPADIC
  notice has to travel with any redistributed binary.
- On Linux, prefer the glibc archive. The musl one is for systems without a
  recent glibc; it is slower on large batches of files.
- Building from source needs Rust 1.88 or later.

[Unreleased]: https://github.com/7nohe/issen/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/7nohe/issen/releases/tag/v0.1.0

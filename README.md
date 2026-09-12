# issen

日本語版: [README.ja.md](README.ja.md)

A fast, deterministic Markdown linter for Japanese prose — built as a harness for documents written by AI agents.

Issen (一閃) is a single flash of the blade: one pass, and what the document should not carry is gone.

- Morphological analysis by [Lindera](https://github.com/lindera/lindera) with the embedded IPADIC dictionary. One binary, no external process, no network.
- Rules are ported from `textlint-rule-preset-ja-technical-writing`. Fixtures check that issen reports the same rule ID, line, and column as textlint does.
- Output for people (text), for agents (JSON with character offsets and fixes), and for comparison (textlint-compatible JSON).

## Install

```bash
cargo install --path .
```

The IPADIC dictionary is compiled into the binary, so the result is around 50 MB and needs nothing else at run time.

## Usage

```bash
issen README.md
cat generated.md | issen --stdin --format json
issen docs/*.md --format textlint   # to diff against textlint
issen --fix docs/*.md               # fix what can be fixed, report the rest
cat generated.md | issen --stdin --fix > fixed.md   # document to stdout, report to stderr
issen --list-rules
```

`--fix` applies every available fix, re-lints, and repeats until nothing is left to fix (at most 10 rounds). The exit code follows the `gate` setting; by default any error exits 1.

### Driving it from an agent

```text
generate document → issen --stdin --fix --format json → read what remains → rewrite → run again → exit 0
```

A diagnostic looks like this. For the document `# T\n\nﾃｽﾄです。\n`:

```json
{
  "rule": "no-hankaku-kana",
  "severity": "error",
  "message": "Disallow to use 半角カタカナ: \"ﾃｽﾄ\"",
  "line": 3,
  "column": 1,
  "endLine": 3,
  "endColumn": 4,
  "range": [5, 8],
  "fix": { "range": [5, 8], "text": "テスト" }
}
```

`range` is a pair of character offsets from the start of the document. An agent can splice a replacement in without re-parsing. Rule IDs are stable and match textlint's.

## Configuration

`./issen.yml` is read automatically. Options are merged key by key over the preset, so naming a rule to change its severity keeps its thresholds.

```yaml
rules:
  sentence-length: { max: 90 }
  no-exclamation-question-mark: false        # disable
  ja-no-weak-phrase: { severity: warning }   # error by default
  no-doubled-joshi: { min_interval: 1, allow: ["も"] }
  no-mix-dearu-desumasu:
    preferInBody: ですます
    preferInList: である

  # Document validators, inactive until configured.
  required-headings: { headings: [Overview, Requirements] }
  forbidden: { patterns: ["never fails", "/guaranteed .*/"] }   # /…/ is a regex
  terminology:
    terms:
      - { preferred: GitHub, avoid: [Github, github] }          # replaced by --fix

gate:
  fail-on: error      # error (default) | warning | never
  max-warnings: 3     # exit 1 above this; --max-warnings overrides
```

## Rules

Twelve rules read Japanese morphology or script and only apply to Japanese text. The rest work on any language.

| rule | what it checks | Japanese-specific | needs morphology |
| --- | --- | --- | --- |
| sentence-length | at most 100 characters per sentence | | |
| max-comma | at most 3 commas per sentence | | |
| no-invalid-control-character | control characters | | |
| no-zero-width-spaces | zero-width spaces | | |
| no-exclamation-question-mark | `!` and `?`, both widths | | |
| required-headings | headings the document must carry | | |
| forbidden | phrases the document must not carry | | |
| terminology | one spelling per term | | |
| max-ten | at most 3 読点 per sentence (not counting one between two nouns) | yes | yes |
| max-kanji-continuous-len | at most 6 kanji in a row | yes | |
| no-mix-dearu-desumasu | one style per heading, body, and list | yes | yes |
| ja-no-mixed-period | paragraphs end with 。 | yes | |
| no-double-negative-ja | double negatives | yes | yes |
| no-dropping-the-ra | ら抜き言葉 | yes | yes |
| no-doubled-conjunctive-particle-ga | 逆接の「が」 used twice | yes | yes |
| no-doubled-conjunction | the same conjunction twice in a row | yes | yes |
| no-doubled-joshi | the same particle twice in a row | yes | yes |
| no-nfd | UTF8-MAC combining 濁点 | yes | |
| no-hankaku-kana | half-width katakana | yes | |
| ja-no-weak-phrase | hedging (かも, 思う, 思います, 可能性を示唆している) | yes | yes |

The document validators (`required-headings`, `forbidden`, `terminology`) do nothing until the config names them.

Messages for the ported rules are reproduced from textlint, so the Japanese rules speak Japanese.

Not ported yet:

- `ja-no-redundant-expression`
- `ja-no-abusage`
- `ja-no-successive-word`
- `ja-unnatural-alphabet`
- `no-unmatched-pair`
- `arabic-kanji-numbers`

### Non-Japanese documents

The engine itself is not tied to Japanese, but the defaults are. Twelve rules read Japanese morphology or script, and `sentence-length`'s limit of 100 follows a Japanese writing convention that no English sentence would respect. Turn the Japanese rules off and raise the limit:

```yaml
rules:
  max-ten: false
  max-kanji-continuous-len: false
  no-mix-dearu-desumasu: false
  ja-no-mixed-period: false
  no-double-negative-ja: false
  no-dropping-the-ra: false
  no-doubled-conjunctive-particle-ga: false
  no-doubled-conjunction: false
  no-doubled-joshi: false
  no-nfd: false
  no-hankaku-kana: false
  ja-no-weak-phrase: false
  sentence-length: { max: 200 }
```

What is left still earns its keep: sentence length, comma count, invisible characters, and the three document validators. This is not the project's focus, though, and no fixtures cover it.

## Known differences from textlint

- `no-dropping-the-ra` on 来れる / 見れる: textlint reports a column one off (it passes a 1-based position straight through as an index). issen reports the real position.
- `sentence-length`: textlint reports a paragraph-relative column. issen reports the start of the sentence.
- Text inside links and emphasis is checked by more rules here than in textlint.

## Morphology backend

Rules only read the nine IPADIC (MeCab) feature columns, never the analyzer itself. The default backend is Lindera with the bundled IPADIC, kept behind the `Morphology` trait in `src/tokenizer.rs`. Adding another analyzer (Vibrato, a MeCab binding) means implementing that trait and adding a `backend-*` feature.

Lindera moves through major versions quickly, so `Cargo.toml` pins it exactly. Raise the pin only after `cargo test` passes, `tests/compat.rs` included — that suite compares against textlint's real output.

## Development

```bash
cargo build --release
cargo test
```

`tests/fixtures/*.md` are paired with `*.textlint.json`, the real output of textlint v15 with the preset. `tests/compat.rs` requires an exact match on rule ID, line, and column, with the two divergences above listed individually.

## License

MIT

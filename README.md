# issen

日本語版: [README.ja.md](README.ja.md)

A fast, deterministic Markdown linter for Japanese prose — built as a harness for documents written by AI agents.

Issen (一閃) is a single flash of the blade: one pass, and what the document should not carry is gone.

- Morphological analysis by [Lindera](https://github.com/lindera/lindera) with the embedded IPADIC dictionary. One binary, no external process, no network.
- Rules are ported from `textlint-rule-preset-ja-technical-writing`. Over a corpus of 950 Japanese documents issen reproduces 97.9% of what textlint reports on the rules it implements -- see [COMPATIBILITY.md](COMPATIBILITY.md).
- Output for people (text), for agents (JSON with character offsets and fixes), and for comparison (textlint-compatible JSON).

## What this is, and is not

textlint is a pluggable linter for natural language in general. Nothing about the platform is Japanese; the rules arrive as npm packages you choose, and there are presets for English prose as readily as for Japanese. issen is not a port of that platform. It reimplements the rules of one preset, `textlint-rule-preset-ja-technical-writing`, and emits textlint's JSON so the two can be diffed against each other.

So there is no plugin system here, and a textlint rule from npm will not load. If you want English prose rules, or a rule someone published last week, run textlint. issen is for the case where an agent loop wants one binary, a start-up in milliseconds, and the same answer every run. On a 114 KB document it takes about 0.01 s against textlint's 4.3 s on the same machine, with 17 of the preset's rules implemented.

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

`range` is a pair of UTF-16 code unit offsets from the start of the document, so an agent can splice a replacement in without re-parsing. That is the unit textlint reports and the one LSP uses by default; it equals the character count until the document holds an emoji or a rare kanji outside the BMP. `sentence-length` measures in the same unit, and `countBy: codepoints` switches both it and textlint to characters. Rule IDs are stable and match textlint's.

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

The table lists all 20 rules: the 17 ported from the preset, which are on by default and are what `--list-rules` shows, and 3 document validators. Twelve of them read Japanese morphology or script and only apply to Japanese text. The rest work on any language.

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

Messages come from textlint, so the Japanese rules speak Japanese. They are close but not identical: `ja-no-mixed-period` drops textlint's trailing advice, and `no-doubled-joshi` names the word before each particle where textlint sometimes cannot. The compatibility fixtures check rule IDs and offsets, not wording.

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

### Using it with textlint's own tooling

`--format textlint` emits a real textlint result, `loc` and `fix` included, so textlint's formatters read it. That covers CI annotations without issen having to grow its own:

```bash
issen --format textlint docs/*.md > result.json
node -e '
const { loadFormatter } = require("@textlint/linter-formatter");
loadFormatter({ formatterName: "github" })      // or checkstyle, junit, stylish, ...
  .then(f => console.log(f.format(require("./result.json"))));
'
```

What issen cannot use is a textlint *plugin*. Plugins are the processors that teach textlint new file formats, such as `textlint-plugin-latex` or `textlint-plugin-review`; issen reads Markdown and nothing else. Rules published as npm packages are equally out of reach. When you need either, run textlint over the same files and keep issen for the fast loop.

## Known differences from textlint

[COMPATIBILITY.md](COMPATIBILITY.md) measures these against a 950-document corpus and explains each one. In short:

- Compare the two tools on `index`, not on `column`. textlint's own `sentence-length` column disagrees with its own index.
- `no-dropping-the-ra` on 来れる / 見れる: textlint reports a position one off (it passes a 1-based position straight through as an index). issen reports the real position.
- The Markdown parsers differ on lazy continuation, inline HTML and autolinks, so a few blocks are classified differently.
- `sentence-length` and `max-comma` anchor inside a finding differently. The detection is the same.
- `range` spans the whole match here. textlint reports a single code unit for the rules that hand it only an index, among them `max-kanji-continuous-len`, `no-dropping-the-ra`, and `no-double-negative-ja`.

## Morphology backend

Rules only read the nine IPADIC (MeCab) feature columns, never the analyzer itself. The default backend is Lindera with the bundled IPADIC, kept behind the `Morphology` trait in `src/tokenizer.rs`. Adding another analyzer (Vibrato, a MeCab binding) means implementing that trait and adding a `backend-*` feature.

Lindera moves through major versions quickly, so `Cargo.toml` pins it exactly. Raise the pin only after `cargo test` passes, `tests/compat.rs` included — that suite compares against textlint's real output.

## Development

```bash
cargo build --release
cargo test
cargo fmt --check && cargo clippy --all-targets
```

`tests/fixtures/*.md` are paired with `*.textlint.json`, the real output of textlint. The versions that produced them are pinned in `tests/textlint/package.json`, and `tests/textlint/regenerate.sh` reproduces them. Run it only when you mean to move the compatibility target, and read the diff it makes. `tests/compat.rs` demands an exact match on rule ID and offset, with the one documented divergence listed individually.

CI runs the tests on Linux, macOS and Windows. It also checks formatting and clippy, builds on the declared `rust-version`, and lints this repository's own Japanese documents with issen.

## Versioning

issen follows Semantic Versioning, read for a linter whose output other people's CI depends on. What the version protects is the command line, `issen.yml`, and the JSON output. The Rust library surface is not stable while the version is 0.x.

- A minor release may fail a build that used to pass: a new rule enabled by default, or a raised `rust-version`. While in 0.x, a minor release may also change the command line, configuration, or JSON output incompatibly, and the changelog says so.
- A patch release fixes bugs. When issen was reporting the wrong thing, the fix changes what is reported, which can add findings.
- New fields in the JSON output are not breaking. Removed or renamed ones are.

Pin an exact version in CI if every change in findings should be a deliberate upgrade. Changes are recorded in [CHANGELOG.md](CHANGELOG.md).

## License

MIT for issen itself; see `LICENSE`.

Binaries built with the default feature embed the IPADIC dictionary. Its notice must travel with anything you redistribute. `NOTICE` carries that notice and the attribution for the ported rules.

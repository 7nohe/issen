# textlint compatibility

日本語版: [COMPATIBILITY.ja.md](COMPATIBILITY.ja.md)

issen reimplements rules from `textlint-rule-preset-ja-technical-writing`. This
file says exactly what that buys you, measured rather than asserted.

## What "compatible" covers

Three things, and nothing else:

1. **Rule IDs.** The same names, under the same `ja-technical-writing/` prefix.
2. **Output shape.** `--format textlint` emits the JSON array textlint's own
   formatters read, `loc` and `fix` included. All eleven stock formatters
   consume it.
3. **Findings.** On the rules issen implements, the same rule at the same
   character offset.

It is not a port of the textlint platform. There is no plugin system, no
`.textlintrc`, and a rule from npm will not load.

## Rules

Seventeen of the preset's twenty-three rules are implemented.

| Rule | Implemented |
| --- | --- |
| sentence-length | yes |
| max-comma | yes |
| max-ten | yes |
| max-kanji-continuous-len | yes |
| no-mix-dearu-desumasu | yes |
| ja-no-mixed-period | yes |
| no-doubled-conjunction | yes |
| no-doubled-conjunctive-particle-ga | yes |
| no-doubled-joshi | yes |
| no-dropping-the-ra | yes |
| no-double-negative-ja | yes |
| no-nfd | yes |
| no-invalid-control-character | yes |
| no-zero-width-spaces | yes |
| no-exclamation-question-mark | yes |
| no-hankaku-kana | yes |
| ja-no-weak-phrase | yes |
| arabic-kanji-numbers | no |
| ja-no-successive-word | no |
| ja-no-abusage | no |
| ja-no-redundant-expression | no |
| ja-unnatural-alphabet | no |
| no-unmatched-pair | no |

## Measurement

Both tools were run over the same corpus, with every rule at its preset default.

| | |
| --- | --- |
| textlint | 15.8.0 |
| preset | textlint-rule-preset-ja-technical-writing 12.0.2 |
| issen | 0.1.0 |
| config | `{ "rules": { "preset-ja-technical-writing": true } }` |

| Corpus | Commit | Files | Bytes |
| --- | --- | ---: | ---: |
| kubernetes/website `content/ja` | 49d8e37 | 640 | 5,703,176 |
| reactjs/ja.react.dev `src/content` | 8a0810d | 222 | 4,502,625 |
| asciidwango/js-primer `source` | b4f2bb0 | 88 | 1,429,245 |
| | | **950** | **11,635,046** |

Findings are compared as a multiset of `(rule, character offset)` per file.

| Rule | Same | issen misses | issen adds |
| --- | ---: | ---: | ---: |
| no-mix-dearu-desumasu | 5918 | 20 | 192 |
| sentence-length | 3171 | 244 | 232 |
| ja-no-mixed-period | 2646 | 14 | 47 |
| no-doubled-joshi | 2024 | 26 | 20 |
| no-exclamation-question-mark | 866 | 6 | 1 |
| ja-no-weak-phrase | 371 | 2 | 0 |
| max-ten | 207 | 4 | 2 |
| no-doubled-conjunction | 118 | 0 | 4 |
| max-kanji-continuous-len | 92 | 0 | 0 |
| max-comma | 26 | 17 | 17 |
| no-doubled-conjunctive-particle-ga | 4 | 2 | 2 |
| no-zero-width-spaces | 4 | 0 | 0 |
| no-dropping-the-ra | 2 | 0 | 0 |
| **Total** | **15449** | **335** | **517** |

issen reproduces **97.9%** of what textlint reports on the rules it implements,
and adds findings at a rate of 3.3%. The six unimplemented rules account for a
further 2197 textlint findings, which issen is silent on by design.

Both tools processed all 950 files without error.

## Parity is asserted on the offset, not on line and column

textlint's `sentence-length` reports a `column` that disagrees with its own
`index`. The column it prints is the offset minus the number of preceding
lines, which is not a position in any line:

| line | textlint column | textlint index | issen column | issen index |
| ---: | ---: | ---: | ---: | ---: |
| 23 | 489 | 510 | 1 | 510 |
| 27 | 952 | 977 | 1 | 977 |
| 30 | 1354 | 1382 | 99 | 1382 |
| 35 | 1815 | 1848 | 1 | 1848 |

The offsets agree exactly; only the column differs, and issen's column is the
one that follows from the offset. Comparing on `(rule, line, column)` instead
would report 2497 mismatches for this rule alone, none of them real.

It comes from `sentence-splitter`: the sentence nodes its `splitAST` returns
carry a correct `range` and a `loc` whose column is `range[0] - (line - 1)`.
`sentence-length` reports the node itself, so the column reaches the output
unchanged. Rules that position through `padding` or `locator` recompute from
the offset and are unaffected.

So `tests/compat.rs` compares offsets. If you diff the two tools yourself,
compare `index`, not `column`.

## Known differences

### Positions are in UTF-16 code units, throughout

textlint runs on JavaScript strings, so it measures in UTF-16 code units, and
issen matches that. One textlint rule does not match itself: `ja-no-mixed-period`
takes its index from `check-ends-with-period`, which counts code points on one
branch and UTF-16 units on another. On `…ハグに応じています。🤗🤗🤗`, textlint
reports index 26 with range `[26, 27]` — the high surrogate of the *second*
emoji, half a character — where issen reports `[28, 30]`, the last emoji. Every
issen offset is a UTF-16 offset, with no exceptions.

### The Markdown parsers disagree

issen parses with pulldown-cmark, which follows CommonMark. textlint's parser
does not, in at least these places:

- **Lazy continuation.** For `- foo` followed by `bar` on the next line,
  CommonMark says one list item containing `foo\nbar`. textlint's parser ends
  the list item at `foo` and makes `bar` a separate top-level paragraph. Since
  `no-mix-dearu-desumasu` judges body text and list items against separate
  preferences, the two tools can put the same sentence in different buckets.
  This accounts for 74 of that rule's 192 extra findings.
- **Inline HTML and autolinks.** A paragraph ending in `</sub>` or a bare email
  address ends in an HTML or link node for textlint, which skips it, and in
  plain text for issen, which checks it.

These are parser-level and are not going to be reconciled without replacing one
of the two parsers.

### `no-mix-dearu-desumasu` reads list items as raw Markdown

textlint hands the analyser the raw source of a list item, bullet and link
syntax included, rather than its text. A clause that ends a list item in issen's
view is followed by `](https://…)` there, and the analyser stops at a noun
inside the URL instead of at the clause end, so the sentence is never counted.
On

```markdown
* [Istioなどのサービスメッシュを使用してPodに証明書を提供します](https://istio.io/x)
```

issen reports a ですます in a list, and textlint does not. This and the list
boundaries above are what the remaining 192 extra findings for this rule are.

### Anchors within a finding

Some rules detect the same thing and point at a different character in it.
Offsets differ; the finding does not.

- `sentence-length`: when a sentence opens with inline markup, textlint anchors
  at the markup, issen at the first character of text. 185 of the 244 misses
  and 186 of the 232 extras are this, on the same line as the corresponding
  textlint finding.
- `max-comma`: 14 of 17 in each direction, likewise.

### Messages

Message text is not part of the compatibility claim. `ja-no-mixed-period` in
preset 12.0.2 appends a 理由/修正 explanation that issen does not reproduce.

## Reproducing this

`tests/textlint/` holds a pinned textlint and `regenerate.sh`, which rebuilds
the fixtures that `tests/compat.rs` checks against. Regenerating moves the
compatibility target: read the resulting diff, because every changed line is
either a behaviour change in textlint or one issen has to follow.

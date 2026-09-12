# issen

AI Agent が生成した日本語 Markdown を、高速かつ決定論的に検証するための document harness。名は「一閃」。刃が一度きらめけば、文書に不要なものは落ちている。

- 形態素解析は [Lindera](https://github.com/lindera/lindera) + IPA 辞書（単一バイナリ、外部プロセスなし）
- ルールは `textlint-rule-preset-ja-technical-writing` を仕様として移植。同じ入力に対して同じ rule ID・行・列を返すことを fixture で検証している
- 出力は人向けテキスト、Agent 向け JSON、比較用 textlint 互換 JSON

## 使い方

```bash
issen README.md
cat generated.md | issen --stdin --format json
issen docs/*.md --format textlint   # textlint と diff するとき
issen --fix docs/*.md               # 直せるものは直し、残りを報告
cat generated.md | issen --stdin --fix > fixed.md   # 修正後の文書は stdout、報告は stderr
issen --list-rules
```

`--fix` は修正を適用したあと再 lint し、修正がなくなるまで（最大 10 周）繰り返す。終了コードは `gate` 設定に従い、既定では error が 1 件以上なら 1。

### Agent から使うとき

```text
文書を生成 → issen --stdin --fix --format json → 残った診断を読んで書き直す → 再実行 → exit 0
```

JSON の `range` は文書先頭からの文字オフセットなので、Agent はそのまま該当箇所を差し替えられる。rule ID は textlint と同じ名前で固定。

JSON 出力の各診断は `rule` / `severity` / `message` / `line` / `column` / `endLine` / `endColumn` / `range`（文書先頭からの文字オフセット）と、ある場合は `fix`（`range` と置換文字列）を持つ。

## 設定

`./issen.yml` があれば自動で読む。書かなかったルールは preset の既定値のまま。

```yaml
rules:
  sentence-length: { max: 90 }
  no-exclamation-question-mark: false        # 無効化
  ja-no-weak-phrase: { severity: warning }   # 既定は error
  no-doubled-joshi: { min_interval: 1, allow: ["も"] }
  no-mix-dearu-desumasu:
    preferInBody: ですます
    preferInList: である

  # 文書バリデータ。書いたときだけ有効。
  required-headings: { headings: [概要, 前提条件] }
  forbidden: { patterns: ["絶対に成功します", "/必ず.*します/"] }   # /…/ は正規表現
  terminology:
    terms:
      - { preferred: GitHub, avoid: [Github, github] }               # --fix で置換

gate:
  fail-on: error      # error（既定）| warning | never
  max-warnings: 3     # 超えたら exit 1。--max-warnings で上書き可
```

## ルール

| rule | 概要 | NLP |
| --- | --- | --- |
| sentence-length | 1 文 100 文字以下 | |
| max-comma | 1 文にコンマ 3 つまで | |
| max-ten | 1 文に読点 3 つまで（名詞に挟まれた読点は数えない） | 要 |
| max-kanji-continuous-len | 漢字の連続 6 文字まで | |
| no-mix-dearu-desumasu | 見出し・本文・箇条書きごとに文体を統一 | 要 |
| ja-no-mixed-period | 段落末は「。」 | |
| no-double-negative-ja | 二重否定 | 要 |
| no-dropping-the-ra | ら抜き言葉 | 要 |
| no-doubled-conjunctive-particle-ga | 逆接の「が」の重複 | 要 |
| no-doubled-conjunction | 同じ接続詞の連続 | 要 |
| no-doubled-joshi | 同じ助詞の連続 | 要 |
| no-nfd | UTF8-MAC 濁点 | |
| no-invalid-control-character | 制御文字 | |
| no-zero-width-spaces | ゼロ幅スペース | |
| no-exclamation-question-mark | 感嘆符・疑問符 | |
| no-hankaku-kana | 半角カナ | |
| ja-no-weak-phrase | 弱い表現（かも、思う、思います、可能性を示唆している） | 要 |

文書バリデータ（設定したときだけ動く）: required-headings、forbidden、terminology。

未移植: ja-no-redundant-expression, ja-no-abusage, ja-no-successive-word, ja-unnatural-alphabet, no-unmatched-pair, arabic-kanji-numbers。

## textlint との既知の差

- `no-dropping-the-ra` の 来れる/見れる は、textlint が 1 文字ずれた列を返す（1 始まりの位置をそのまま index に使っている）。issen は実際の位置を返す。
- `sentence-length` の列は、textlint が段落ノード基準の値を返す。issen は文の先頭を返す。
- Link / Emphasis 内のテキストも検査対象にしている（textlint の多くのルールは除外する）。

## 形態素解析バックエンド

ルールが見るのは IPADIC（MeCab）形式の 9 素性だけで、解析器そのものには依存しない。既定のバックエンドは Lindera + 同梱 IPADIC で、`src/tokenizer.rs` の `Morphology` trait の実装として閉じ込めてある。別の解析器（Vibrato、MeCab バインディングなど）を足すときは、同 trait を実装して `backend-*` feature を追加する。

Lindera はメジャーバージョンの更新が速いので `Cargo.toml` で完全に固定している。上げるときは `cargo test` で `tests/compat.rs`（textlint 実出力との一致）を通してから。

## 開発

```bash
cargo build --release
cargo test
```

`tests/fixtures/*.md` と対になる `*.textlint.json` は textlint v15 + preset の実出力で、`tests/compat.rs` が rule ID・行で全件一致することを確認する。

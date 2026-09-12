# issen

English: [README.md](README.md)

AI エージェントが書いた日本語 Markdown を、高速かつ決定論的に検証するための document harness です。

名は「一閃」。刃が一度きらめけば、文書に不要なものは落ちています。

- 形態素解析は [Lindera](https://github.com/lindera/lindera) と同梱の IPA 辞書による。単一バイナリで、外部プロセスやネットワークは不要
- ルールは `textlint-rule-preset-ja-technical-writing` からの移植。同じ入力に対して textlint と同じ rule ID・行・列を返すことを fixture で確認している
- 出力は 3 種類。人向けのテキスト、エージェント向けの JSON、比較用の textlint 互換 JSON

## インストール

```bash
cargo install --path .
```

IPA 辞書はバイナリに埋め込まれます。そのため約 50 MB になりますが、実行時に必要なものは他にありません。

## 使い方

```bash
issen README.md
cat generated.md | issen --stdin --format json
issen docs/*.md --format textlint   # textlint と diff を取るとき
issen --fix docs/*.md               # 直せるものを直し、残りを報告
cat generated.md | issen --stdin --fix > fixed.md   # 文書は stdout、報告は stderr
issen --list-rules
```

`--fix` は修正を適用したあとに再 lint し、直すものがなくなるまで繰り返します。上限は 10 周です。終了コードは `gate` 設定に従い、既定ではエラーが 1 件でもあれば 1 を返します。

### エージェントから使う

```text
文書を生成 → issen --stdin --fix --format json → 残った診断を読む → 書き直す → 再実行 → exit 0
```

診断は次の形です。文書 `# T\n\nﾃｽﾄです。\n` に対する出力です。

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

`range` は文書先頭からの文字オフセットの組です。エージェントは構文解析をやり直さずに、その範囲を差し替えられます。rule ID は textlint と同じ名前で固定です。

## 設定

`./issen.yml` があれば自動で読みます。オプションはプリセットにキー単位で重ねるため、severity だけを指定しても閾値は保たれます。

```yaml
rules:
  sentence-length: { max: 90 }
  no-exclamation-question-mark: false        # 無効化
  ja-no-weak-phrase: { severity: warning }   # 既定は error
  no-doubled-joshi: { min_interval: 1, allow: ["も"] }
  no-mix-dearu-desumasu:
    preferInBody: ですます
    preferInList: である

  # 文書バリデータ。設定したときだけ動く。
  required-headings: { headings: [概要, 前提条件] }
  forbidden: { patterns: ["絶対に成功します", "/必ず.*します/"] }   # /…/ は正規表現
  terminology:
    terms:
      - { preferred: GitHub, avoid: [Github, github] }             # --fix で置換

gate:
  fail-on: error      # error（既定）| warning | never
  max-warnings: 3     # 超えたら exit 1。--max-warnings で上書き
```

## ルール

12 のルールは日本語の形態素や表記を見るため、日本語の文書にだけ働きます。残りは言語を選びません。

| rule | 対象 | 日本語専用 | 形態素解析 |
| --- | --- | --- | --- |
| sentence-length | 1 文 100 文字以下 | | |
| max-comma | 1 文にコンマ 3 つまで | | |
| no-invalid-control-character | 制御文字 | | |
| no-zero-width-spaces | ゼロ幅スペース | | |
| no-exclamation-question-mark | 感嘆符と疑問符（全角と半角） | | |
| required-headings | 必須の見出し | | |
| forbidden | 禁止する表現 | | |
| terminology | 用語の表記ゆれ | | |
| max-ten | 1 文に読点 3 つまで（名詞に挟まれた読点は数えない） | あり | あり |
| max-kanji-continuous-len | 漢字の連続は 6 文字まで | あり | |
| no-mix-dearu-desumasu | 見出し・本文・箇条書きごとの文体統一 | あり | あり |
| ja-no-mixed-period | 段落末の句点 | あり | |
| no-double-negative-ja | 二重否定 | あり | あり |
| no-dropping-the-ra | ら抜き言葉 | あり | あり |
| no-doubled-conjunctive-particle-ga | 逆接の「が」の重複 | あり | あり |
| no-doubled-conjunction | 同じ接続詞の連続 | あり | あり |
| no-doubled-joshi | 同じ助詞の連続 | あり | あり |
| no-nfd | UTF8-MAC 濁点 | あり | |
| no-hankaku-kana | 半角カナ | あり | |
| ja-no-weak-phrase | 弱い表現（`かも`、`思う`、`思います`、`可能性を示唆している`） | あり | あり |

文書バリデータの `required-headings`、`forbidden`、`terminology` は、設定に名前を書くまで何もしません。

移植したルールのメッセージは textlint のものをそのまま使っています。

未移植は次の 6 つです。

- `ja-no-redundant-expression`
- `ja-no-abusage`
- `ja-no-successive-word`
- `ja-unnatural-alphabet`
- `no-unmatched-pair`
- `arabic-kanji-numbers`

### 日本語以外の文書

エンジン自体は日本語に縛られていませんが、既定値は日本語に合わせてあります。12 のルールは日本語の形態素や表記を読みます。`sentence-length` の 100 文字という上限も、日本語の書き方の慣習です。英語の文がこの上限を守ることはまずありません。日本語のルールを切り、上限を上げてください。

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

残るルールも役には立ちます。文の長さ、コンマの数、不可視文字、3 つの文書バリデータです。ただしプロジェクトの主眼ではなく、fixture も用意していません。

## textlint との既知の差

- `no-dropping-the-ra` の `来れる`・`見れる` では、textlint の列が 1 文字ずれる。1 始まりの位置をそのまま index に使っているため。issen は実際の位置を返す
- `sentence-length` の列は、textlint が段落を基準にした値を返す。issen は文の先頭を返す
- リンクと強調の中のテキストは、textlint より多くのルールが検査の対象にしている

## 形態素解析バックエンド

ルールが見るのは IPADIC（MeCab）形式の 9 素性だけで、解析器そのものには依存しません。既定のバックエンドは Lindera と同梱の IPA 辞書で、`src/tokenizer.rs` の `Morphology` trait の実装として閉じ込めてあります。別の解析器を足すときは、同じ trait を実装して `backend-*` feature を追加します。Vibrato や MeCab バインディングが候補です。

Lindera はメジャーバージョンの更新が速いため、`Cargo.toml` で完全に固定しています。上げるときは `cargo test` を通してください。`tests/compat.rs` が textlint の実出力と比較します。

## 開発

```bash
cargo build --release
cargo test
```

`tests/fixtures/*.md` と対になる `*.textlint.json` は、textlint v15 とプリセットの実出力です。`tests/compat.rs` が rule ID・行・列の完全一致を要求します。上に挙げた 2 つの差だけは個別に列挙してあります。

## ライセンス

MIT

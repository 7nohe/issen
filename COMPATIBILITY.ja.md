# textlint 互換性

English: [COMPATIBILITY.md](COMPATIBILITY.md)

issen は `textlint-rule-preset-ja-technical-writing` のルールを再実装したものです。
「互換」が具体的に何を指すのかを、主張ではなく実測で示します。

## 互換の範囲

次の 3 点だけです。

1. **ルール ID**。`ja-technical-writing/` 接頭辞つきの同じ名前。
2. **出力の形**。`--format textlint` は textlint 自身のフォーマッタが読める JSON 配列を
   出力します（`loc` と `fix` を含む）。標準の 11 フォーマッタすべてで確認済み。
3. **検出結果**。実装済みのルールについて、同じルールが同じ文字オフセットで報告されること。

textlint というプラットフォームの移植ではありません。プラグイン機構はなく、
`.textlintrc` も読まず、npm のルールも読み込めません。

## ルール

プリセットの 23 ルールのうち 17 を実装しています。

| ルール | 実装 |
| --- | --- |
| sentence-length | あり |
| max-comma | あり |
| max-ten | あり |
| max-kanji-continuous-len | あり |
| no-mix-dearu-desumasu | あり |
| ja-no-mixed-period | あり |
| no-doubled-conjunction | あり |
| no-doubled-conjunctive-particle-ga | あり |
| no-doubled-joshi | あり |
| no-dropping-the-ra | あり |
| no-double-negative-ja | あり |
| no-nfd | あり |
| no-invalid-control-character | あり |
| no-zero-width-spaces | あり |
| no-exclamation-question-mark | あり |
| no-hankaku-kana | あり |
| ja-no-weak-phrase | あり |
| arabic-kanji-numbers | なし |
| ja-no-successive-word | なし |
| ja-no-abusage | なし |
| ja-no-redundant-expression | なし |
| ja-unnatural-alphabet | なし |
| no-unmatched-pair | なし |

## 測定

両ツールを同じコーパスに、すべてプリセットの既定値で実行しました。

| | |
| --- | --- |
| textlint | 15.8.0 |
| preset | textlint-rule-preset-ja-technical-writing 12.0.2 |
| issen | 0.1.0 |
| 設定 | `{ "rules": { "preset-ja-technical-writing": true } }` |

| コーパス | commit | ファイル | バイト |
| --- | --- | ---: | ---: |
| kubernetes/website `content/ja` | 49d8e37 | 640 | 5,703,176 |
| reactjs/ja.react.dev `src/content` | 8a0810d | 222 | 4,502,625 |
| asciidwango/js-primer `source` | b4f2bb0 | 88 | 1,429,245 |
| | | **950** | **11,635,046** |

検出結果はファイルごとに `(ルール, 文字オフセット)` の多重集合として比較しています。

| ルール | 一致 | issen の取りこぼし | issen の追加 |
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
| **合計** | **15449** | **335** | **517** |

実装済みルールについて、issen は textlint の報告の **97.9%** を再現し、
追加で報告する割合は 3.3% です。未実装の 6 ルールぶんとして textlint はさらに
2197 件を報告しますが、これは設計どおり issen が黙る部分です。

950 ファイルすべてを両ツールともエラーなく処理しました。

## 一致の判定は行・列ではなくオフセットで行う

textlint の `sentence-length` が返す `column` は、textlint 自身の `index` と
食い違っています。出力される列はオフセットから先行行数を引いた値であり、
どの行の位置でもありません。

| 行 | textlint の列 | textlint の index | issen の列 | issen の index |
| ---: | ---: | ---: | ---: | ---: |
| 23 | 489 | 510 | 1 | 510 |
| 27 | 952 | 977 | 1 | 977 |
| 30 | 1354 | 1382 | 99 | 1382 |
| 35 | 1815 | 1848 | 1 | 1848 |

オフセットは完全に一致していて、違うのは列だけです。そしてオフセットから導けるのは
issen の列のほうです。`(ルール, 行, 列)` で比較すると、このルールだけで 2497 件の
不一致が出ますが、そのいずれも実体がありません。

発生源は `sentence-splitter` です。`splitAST` が返す文ノードは `range` が正しく、
`loc` の列だけが `range[0] - (line - 1)` になっています。`sentence-length` は
ノードをそのまま報告するので、この列が出力まで届きます。`padding` や `locator` で
位置を指定するルールはオフセットから再計算されるため影響を受けません。

そのため `tests/compat.rs` はオフセットで比較しています。自分で 2 つのツールを
突き合わせる場合も、`column` ではなく `index` を見てください。

## 既知の差分

### 位置はすべて UTF-16 コードユニット

textlint は JavaScript の文字列上で動くため、UTF-16 コードユニットで数えます。
issen もそれに合わせています。textlint 側で 1 つだけ自己矛盾しているのが
`ja-no-mixed-period` です。index を `check-ends-with-period` から取っていますが、
この関数は分岐によって、コードポイントを返すことも UTF-16 コードユニットを
返すこともあります。
`…ハグに応じています。🤗🤗🤗` に対して textlint は index 26、範囲 `[26, 27]` を返します。
これは **2 つ目**の絵文字の上位サロゲート、つまり文字の半分です。issen は `[28, 30]`
（最後の絵文字）を返します。issen のオフセットは例外なく UTF-16 です。

### Markdown パーサの挙動が違う

issen は CommonMark 準拠の pulldown-cmark を使っています。textlint のパーサは
少なくとも次の点で CommonMark と異なります。

- **lazy continuation**。`- foo` の次の行に `bar` があるとき、CommonMark では
  `foo\nbar` を含む 1 つのリスト項目である。textlint のパーサはリスト項目を
  `foo` で終え、`bar` を独立した段落とする。`no-mix-dearu-desumasu` は本文と
  箇条書きを別々の基準で判定するため、同じ文が別の分類に入る。
  このルールの追加報告 192 件のうち 74 件がこれにあたる
- **インライン HTML と自動リンク**。`</sub>` で終わる段落や裸のメールアドレスは、
  textlint では HTML ノード・リンクノードで終わるため対象外となるが、
  issen では通常のテキストなので検査される

これはパーサの層の違いなので、どちらかのパーサを入れ替えない限り解消しません。

### `no-mix-dearu-desumasu` はリスト項目を生の Markdown として読む

textlint は解析器にリスト項目の**生のソース**を渡します。行頭記号やリンク記法を
含んだままです。issen から見て節の末尾にある語が、textlint 側では
`](https://…)` に続かれており、解析器は節の末尾ではなく URL 内の名詞で止まります。
その結果その文は数えられません。次の例で issen は箇条書きの ですます を報告し、
textlint は報告しません。

```markdown
* [Istioなどのサービスメッシュを使用してPodに証明書を提供します](https://istio.io/x)
```

このルールに残る追加報告 192 件は、これと上のリスト境界の差で説明がつきます。

### 同じ検出の中でのアンカー位置

検出内容は同じで、その中のどの文字を指すかだけが違うものがあります。

- `sentence-length`: 文がインライン記法で始まるとき、textlint は記法の位置を、
  issen は最初のテキスト文字を指す。取りこぼし 244 件のうち 185 件と
  追加 232 件のうち 186 件は、対応する textlint の報告と同じ行で見つかる
- `max-comma`: 同様に、各方向 17 件のうち 14 件

### メッセージ

メッセージ本文は互換の対象外です。preset 12.0.2 の `ja-no-mixed-period` は
理由と修正方針の説明を付けますが、issen はこれを再現しません。

## 再現方法

`tests/textlint/` にバージョン固定した textlint と `regenerate.sh` があります。
これは `tests/compat.rs` が参照する fixture を再生成します。再生成は互換性の
基準そのものを動かす操作なので、出た差分は必ず読んでください。変化した行は
textlint の挙動変更か、issen が追従すべき挙動のどちらかです。

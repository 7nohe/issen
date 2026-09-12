//! Regressions for the textlint-parity and position bugs found in review.
//! Each case names the textlint behaviour it pins.

use issen::{Config, Linter};

fn rules_at(src: &str) -> Vec<(String, usize, usize)> {
    let linter = Linter::preset().unwrap();
    linter.lint("t.md", src).diagnostics.into_iter().map(|d| (d.rule, d.line, d.column)).collect()
}

fn has(src: &str, rule: &str) -> bool {
    rules_at(src).iter().any(|(r, _, _)| r == rule)
}

#[test]
fn non_ascii_inline_code_never_panics() {
    for src in ["これは`日本`\n", "`あ`ﾃｽﾄ。\n", "テスト`あ`\n", "- `日本語`です。\n"] {
        let _ = rules_at(src);
    }
}

#[test]
fn positions_after_breaks_and_entities() {
    assert!(rules_at("これは  \nﾃｽﾄです。\n").contains(&("no-hankaku-kana".into(), 2, 1)));
    assert!(rules_at("これは\r\nﾃｽﾄです。\r\n").contains(&("no-hankaku-kana".into(), 2, 1)));
}

#[test]
fn hard_wrapped_and_quoted_text_is_one_sentence() {
    // sentence-splitter: a line break does not end a sentence.
    assert!(has("この設定を有効にすると、起動時に、キャッシュを、\nすべて破棄してから、インデックスを再構築します。\n", "max-ten"));
    // ...and neither does 。 inside 「」.
    let long = format!("「{}。」{}。\n", "あ".repeat(60), "あ".repeat(60));
    assert!(has(&long, "sentence-length"));
}

#[test]
fn table_cells_are_not_paragraphs() {
    let src = "| 名前 | 説明 |\n|---|---|\n| save | ファイルを保存する |\n| 状態 | 実行中である |\n";
    assert!(!has(src, "ja-no-mixed-period"));
    assert!(!has(src, "no-mix-dearu-desumasu"));
    // Character-level rules still see cells.
    assert!(has("| ﾃｽﾄ |\n|---|\n| a |\n", "no-hankaku-kana"));
}

#[test]
fn nested_tight_list_items_are_separate_blocks() {
    assert!(!has("- 私は学生です\n  - 彼は先生です\n", "no-doubled-joshi"));
}

#[test]
fn paragraph_ending_in_code_or_link_is_not_checked_for_period() {
    assert!(!has("ビルドに使うコマンドは `npm run build`\n", "ja-no-mixed-period"));
    assert!(!has("詳細は[公式ドキュメント](https://example.com/docs)\n", "ja-no-mixed-period"));
    assert!(has("文末に句点がない\n", "ja-no-mixed-period"));
}

#[test]
fn sentence_length_skips_url_links_and_link_only_paragraphs() {
    let url = format!("https://example.com/{}", "a".repeat(150));
    assert!(!has(&format!("リポジトリは <{url}> にあります。\n"), "sentence-length"));
    assert!(!has(&format!("[{}]({url})\n", "あ".repeat(120)), "sentence-length"));
    assert!(has(&format!("{}。\n", "あ".repeat(120)), "sentence-length"));
}

#[test]
fn inline_node_exclusions_match_textlint() {
    assert!(!has("[利用規約同意確認画面](https://x)を参照。\n", "max-kanji-continuous-len"));
    assert!(has("利用規約同意確認画面を参照。\n", "max-kanji-continuous-len"));
    assert!(!has("Yahoo!を使います。\n", "no-exclamation-question-mark"));
    assert!(has("すごい!です。\n", "no-exclamation-question-mark"));
    // Block quotes: weak-phrase runs, sentence rules do not.
    assert!(has("> これはかもしれない。\n", "ja-no-weak-phrase"));
    assert!(!has("> 私は昨日は東京に行きました。\n", "no-doubled-joshi"));
    // A tight list item starting with inline code keeps the code mask.
    let code = "a".repeat(101);
    assert!(has(&format!("- `{code}`です。\n"), "sentence-length"));
}

#[test]
fn exclamation_options_are_honoured() {
    let linter = Linter::new(Config::parse("rules:\n  no-exclamation-question-mark: { allowFullWidthExclamation: true }\n").unwrap()).unwrap();
    let r = linter.lint("t.md", "保存してください！\n\n本当ですか？\n");
    let marks: Vec<usize> = r.diagnostics.iter().filter(|d| d.rule == "no-exclamation-question-mark").map(|d| d.line).collect();
    assert_eq!(marks, vec![3]);
}

#[test]
fn conjunction_after_whitespace_is_ignored() {
    // textlint skips a 接続詞 that follows a whitespace token; Lindera emits no
    // whitespace tokens, so the rule must look at the text instead.
    assert!(!has("速いので `Cargo.toml` で固定する。上げるときは `cargo test` で確認する。\n", "no-doubled-conjunction"));
    assert!(has("しかし、遅い。しかし、安い。\n", "no-doubled-conjunction"));
}

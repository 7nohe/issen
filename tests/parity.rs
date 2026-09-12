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
    let linter =
        Linter::new(Config::parse("rules:\n  no-exclamation-question-mark: { allowFullWidthExclamation: true }\n").unwrap()).unwrap();
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

/// textlint runs on JavaScript strings, so every position and length it
/// reports is in UTF-16 code units. issen matches that, which only shows up
/// once a document holds something outside the BMP.
mod utf16 {
    use super::*;

    const EMOJI: &str = "😀"; // one character, two UTF-16 code units

    #[test]
    fn offsets_and_columns_count_utf16() {
        let src = format!("# T\n\n{EMOJI} ﾃｽﾄです。\n");
        let linter = Linter::preset().unwrap();
        let d = linter.lint("t.md", &src).diagnostics.into_iter().find(|d| d.rule == "no-hankaku-kana").unwrap();
        // The emoji plus a space is 3 code units, so the kana starts at column 4.
        assert_eq!((d.line, d.column), (3, 4));
        assert_eq!(d.range, [8, 11]);
        assert_eq!(d.fix.unwrap().range, [8, 11]);
    }

    #[test]
    fn fixes_apply_across_a_surrogate_pair() {
        let linter = Linter::preset().unwrap();
        let fixed = linter.fix("t.md", &format!("{EMOJI} ﾃｽﾄです。\n"));
        assert_eq!(fixed.source, format!("{EMOJI} テストです。\n"));
        assert_eq!(fixed.applied, 1);
    }

    #[test]
    fn sentence_length_counts_utf16_by_default() {
        // 90 characters, 150 UTF-16 code units: over the limit only in textlint's unit.
        let src = format!("{}{}。\n", EMOJI.repeat(60), "あ".repeat(29));
        assert!(has(&src, "sentence-length"));

        let linter = Linter::new(Config::parse("rules:\n  sentence-length: { countBy: codepoints }\n").unwrap()).unwrap();
        assert!(linter.lint("t.md", &src).diagnostics.iter().all(|d| d.rule != "sentence-length"));
    }

    #[test]
    fn kanji_runs_count_utf16() {
        // Four SIP kanji: 4 characters, 8 UTF-16 code units, so over the limit of 6.
        assert!(has("𠮟𠮟𠮟𠮟と言う。\n", "max-kanji-continuous-len"));
        assert!(!has("漢字漢字と言う。\n", "max-kanji-continuous-len"));
    }
}

#[test]
fn textlint_format_carries_every_field_its_formatters_read() {
    use issen::format::{render, Format, Summary};

    let linter = Linter::preset().unwrap();
    let results = vec![linter.lint("t.md", "# T\n\nﾃｽﾄです。\n")];
    let out = render(Format::Textlint, &results, Summary::of(&results, 0));
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let m = &v[0]["messages"][0];

    assert_eq!(m["type"], "lint");
    assert_eq!(m["ruleId"], "ja-technical-writing/no-hankaku-kana");
    assert_eq!(m["severity"], 2);
    assert_eq!(m["index"], m["range"][0]);
    // `loc` duplicates the span; the github and junit formatters read only it.
    assert_eq!(m["loc"]["start"]["line"], m["line"]);
    assert_eq!(m["loc"]["start"]["column"], m["column"]);
    assert_eq!(m["loc"]["end"], serde_json::json!({ "line": 3, "column": 4 }));
    assert!(m["fix"]["text"].is_string());
}

/// Everything below was reported by a pre-publication review. Each case is a
/// way issen could quietly damage a document or quietly check nothing.
mod review {
    use super::*;

    #[test]
    fn a_fix_never_swallows_the_markup_around_it() {
        let linter = Linter::preset().unwrap();
        // The match sits inside one emphasis run: rewritable, markup intact.
        assert_eq!(linter.fix("t.md", "**ﾃｽﾄ**です。\n").source, "**テスト**です。\n");
        assert_eq!(linter.fix("t.md", "`x`ﾃｽﾄです。\n").source, "`x`テストです。\n");
    }

    #[test]
    fn a_fix_spanning_markup_is_dropped_but_still_reported() {
        let linter = Linter::preset().unwrap();
        let fixed = linter.fix("t.md", "ﾃ*ｽ*ﾄです。\n");
        // Rewriting would delete the `*`, so the fix is withheld.
        assert_eq!(fixed.source, "ﾃ*ｽ*ﾄです。\n");
        assert_eq!(fixed.applied, 0);
        let d = fixed.result.diagnostics.iter().find(|d| d.rule == "no-hankaku-kana").unwrap();
        assert!(d.fix.is_none(), "the diagnostic stands, only its fix is withheld");
    }

    #[test]
    fn only_u200b_counts_as_a_zero_width_space() {
        // U+200D joins the parts of an emoji; deleting it splits the family up.
        assert!(!has("👨‍👩‍👧‍👦です。\n", "no-zero-width-spaces"));
        assert!(has("あ\u{200B}い。\n", "no-zero-width-spaces"));
        let linter = Linter::preset().unwrap();
        assert_eq!(linter.fix("t.md", "👨‍👩‍👧‍👦です。\n").applied, 0);
    }

    #[test]
    fn a_config_that_would_check_nothing_is_rejected() {
        // A typo in the gate would let every error through with exit code 0.
        assert!(Config::parse("gate: { fail-on: erorr }\n").is_err());
        // A textlint-style preset key disables nothing here.
        assert!(Config::parse("rules:\n  preset-ja-technical-writing: { max-ten: false }\n").is_err());
        // A near-miss gets a suggestion.
        let e = Config::parse("rules:\n  max-ten-x: true\n").unwrap_err();
        assert!(e.contains("max-ten"), "{e}");
        // An unusable regex means the rule silently matches nothing.
        let bad = Config::parse("rules:\n  forbidden: { patterns: [\"/[/\"] }\n").unwrap();
        assert!(Linter::new(bad).is_err());
        assert!(Config::parse("rules:\n  sentence-length: { countBy: chars }\n").and_then(Linter::new).is_err());
    }

    #[test]
    fn regex_options_honour_textlints_flags() {
        let linter = Linter::new(Config::parse("rules:\n  sentence-length: { max: 3, skipPatterns: [\"/abc/i\"] }\n").unwrap()).unwrap();
        assert!(linter.lint("t.md", "ABC。\n").diagnostics.is_empty());

        let linter = Linter::new(Config::parse("rules:\n  forbidden: { patterns: [\"/NG/i\"] }\n").unwrap()).unwrap();
        assert_eq!(linter.lint("t.md", "これは ng です。\n").diagnostics.iter().filter(|d| d.rule == "forbidden").count(), 1);
    }

    #[test]
    fn inline_code_keeps_its_utf16_length_for_counting() {
        // textlint measures the code's own text: `😀😀`。 is 5 code units.
        let linter = Linter::new(Config::parse("rules:\n  sentence-length: { max: 3 }\n").unwrap()).unwrap();
        let d = linter.lint("t.md", "`😀😀`。\n").diagnostics;
        assert!(d.iter().any(|d| d.rule == "sentence-length" && d.message.contains("length(5)")), "{d:?}");
    }

    #[test]
    fn control_characters_are_named_as_textlint_names_them() {
        let linter = Linter::preset().unwrap();
        let d = linter.lint("t.md", "制御\u{1}文字。\n").diagnostics;
        assert!(d.iter().any(|d| d.message == "Found invalid control character(START OF HEADING \\u0001)"), "{d:?}");
    }
}

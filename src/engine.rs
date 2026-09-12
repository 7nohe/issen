use crate::config::{opt_str, Config};
use crate::diagnostic::{Diagnostic, Fix, Severity};
use crate::document::Document;
use crate::rules::{self, Anchor, BlockData, Ctx, Report};
use crate::sentence;
use crate::tokenizer::{Token, Tokenizer};
use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Serialize)]
pub struct FileResult {
    pub path: String,
    pub diagnostics: Vec<Diagnostic>,
}

pub struct FixResult {
    pub source: String,
    pub applied: usize,
    pub result: FileResult,
}

pub struct Linter {
    tokenizer: Tokenizer,
    config: Config,
}

/// Tokens by block text, so `--fix` rounds do not re-analyze unchanged blocks.
type TokenCache = HashMap<String, Vec<Token>>;

impl Linter {
    pub fn new(config: Config) -> Result<Linter, String> {
        Ok(Linter { tokenizer: Tokenizer::new()?, config })
    }

    pub fn preset() -> Result<Linter, String> {
        Linter::new(Config::preset())
    }

    pub fn lint(&self, path: &str, source: &str) -> FileResult {
        self.lint_cached(path, source, &mut TokenCache::new())
    }

    /// Lint, apply every non-overlapping fix, and lint again -- repeated until
    /// nothing is left to fix (bounded, so two fixes can never ping-pong).
    pub fn fix(&self, path: &str, source: &str) -> FixResult {
        let mut cache = TokenCache::new();
        let mut text = source.to_string();
        let mut applied = 0usize;
        let mut result = self.lint_cached(path, &text, &mut cache);
        for _ in 0..10 {
            let (next, n) = apply_fixes(&text, &result.diagnostics);
            if n == 0 {
                break;
            }
            applied += n;
            text = next;
            result = self.lint_cached(path, &text, &mut cache);
        }
        FixResult { source: text, applied, result }
    }

    fn lint_cached(&self, path: &str, source: &str, cache: &mut TokenCache) -> FileResult {
        let doc = Document::parse(source);
        let mut rules = rules::build(&self.config);

        let blocks: Vec<BlockData> = doc
            .blocks
            .iter()
            .enumerate()
            .map(|(index, block)| {
                let tokens = cache.entry(block.text.clone()).or_insert_with(|| self.tokenizer.tokenize(&block.text)).clone();
                let sentences = sentence::split(&block.text, &tokens);
                BlockData { index, block, tokens, sentences }
            })
            .collect();

        let mut diagnostics = Vec::new();
        for rule in rules.iter_mut() {
            let id = rule.id();
            let options = self.config.options(id);
            let severity = Severity::parse(&opt_str(&options, "severity", "error")).unwrap_or(Severity::Error);
            let ctx = Ctx { doc: &doc, options };
            let scope = rule.scope();
            for blk in blocks.iter().filter(|b| scope.admits(b.block)) {
                for r in rule.check(blk, &ctx) {
                    diagnostics.push(to_diagnostic(&doc, Anchor::Block(blk.index), id, severity, r));
                }
            }
            for (anchor, r) in rule.finish(&ctx) {
                diagnostics.push(to_diagnostic(&doc, anchor, id, severity, r));
            }
        }
        diagnostics.sort_by(|a, b| (a.line, a.column, &a.rule).cmp(&(b.line, b.column, &b.rule)));
        FileResult { path: path.to_string(), diagnostics }
    }
}

/// Apply the fixes carried by `diagnostics` to `source`. Fix ranges are
/// UTF-16 offsets, the same unit the diagnostics report; overlapping ones are
/// applied first-come and the rest skipped, so the caller re-lints to pick
/// them up on the next round.
pub fn apply_fixes(source: &str, diagnostics: &[Diagnostic]) -> (String, usize) {
    let mut fixes: Vec<&Fix> = diagnostics.iter().filter_map(|d| d.fix.as_ref()).collect();
    if fixes.is_empty() {
        return (source.to_string(), 0);
    }
    fixes.sort_by_key(|f| (f.range[0], f.range[1]));

    // UTF-16 offset -> byte offset. Both halves of a surrogate pair map to the
    // start of their character, so a range can never split one.
    let mut to_byte: Vec<usize> = Vec::with_capacity(source.len() + 1);
    for (b, ch) in source.char_indices() {
        to_byte.extend(std::iter::repeat_n(b, ch.len_utf16()));
    }
    to_byte.push(source.len());

    let mut out = String::with_capacity(source.len());
    let mut cursor = 0usize; // in UTF-16 offsets
    let mut applied = 0usize;
    for f in fixes {
        let [start, end] = f.range;
        if start < cursor || end < start || end >= to_byte.len() {
            continue;
        }
        out.push_str(&source[to_byte[cursor]..to_byte[start]]);
        out.push_str(&f.text);
        cursor = end;
        applied += 1;
    }
    out.push_str(&source[to_byte[cursor]..]);
    (out, applied)
}

fn to_diagnostic(doc: &Document, anchor: Anchor, rule: &str, severity: Severity, r: Report) -> Diagnostic {
    let to_src = |b: usize| match anchor {
        Anchor::Block(i) => doc.blocks[i].to_source(b),
        Anchor::Source => b.min(doc.source.len()),
    };
    let start = to_src(r.start);
    let end = to_src(r.end).max(start);
    let (line, column) = doc.position(start);
    let (end_line, end_column) = doc.position(end);
    let fix = r.fix.map(|f| {
        let fs = to_src(f.start);
        let fe = to_src(f.end).max(fs);
        Fix { range: [doc.utf16_offset(fs), doc.utf16_offset(fe)], text: f.text }
    });
    Diagnostic {
        rule: rule.to_string(),
        severity,
        message: r.message,
        line,
        column,
        end_line,
        end_column,
        range: [doc.utf16_offset(start), doc.utf16_offset(end)],
        fix,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn diag(range: [usize; 2], fix: Option<(&str, [usize; 2])>) -> Diagnostic {
        Diagnostic {
            rule: "t".into(),
            severity: Severity::Error,
            message: String::new(),
            line: 1,
            column: 1,
            end_line: 1,
            end_column: 1,
            range,
            fix: fix.map(|(text, range)| Fix { range, text: text.into() }),
        }
    }

    #[test]
    fn applies_char_range_fixes_and_skips_overlaps() {
        let src = "ﾃｽﾄを実行する.";
        let fixes = vec![
            diag([0, 3], Some(("テスト", [0, 3]))),
            diag([1, 2], Some(("X", [1, 2]))), // overlaps the first: skipped
            diag([8, 9], Some(("。", [8, 9]))),
        ];
        let (out, n) = apply_fixes(src, &fixes);
        assert_eq!(n, 2);
        assert_eq!(out, "テストを実行する。");
    }

    #[test]
    fn fix_loop_converges() {
        let linter = Linter::preset().unwrap();
        let r = linter.fix("t.md", "ﾃｽﾄを実行する.\n");
        assert_eq!(r.source, "テストを実行する。\n");
        assert_eq!(r.applied, 2);
        assert!(r.result.diagnostics.is_empty(), "{:?}", r.result.diagnostics);
    }

    #[test]
    fn fixes_after_breaks_and_entities_keep_the_source_intact() {
        let linter = Linter::preset().unwrap();
        assert_eq!(linter.fix("t.md", "これは  \nﾃｽﾄです。\n").source, "これは  \nテストです。\n");
        assert_eq!(linter.fix("t.md", "これは\r\nﾃｽﾄです。\r\n").source, "これは\r\nテストです。\r\n");
        let r = linter.lint("t.md", "これは`日本`\n");
        assert!(r.diagnostics.iter().all(|d| d.rule != "ja-no-mixed-period"), "{:?}", r.diagnostics);
    }
}

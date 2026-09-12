//! Output renderers: human text, issen's JSON, and textlint-compatible JSON.

use crate::diagnostic::{Fix, Severity};
use crate::engine::FileResult;
use crate::rules::TEXTLINT_RULE_PREFIX;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// `path:line:col: message [rule]`, one per line.
    Text,
    /// issen's own JSON: files, diagnostics with character ranges and fixes.
    Json,
    /// textlint-compatible JSON array, for diffing against textlint.
    Textlint,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Summary {
    pub errors: usize,
    pub warnings: usize,
    pub fixed: usize,
}

impl Summary {
    pub fn of(results: &[FileResult], fixed: usize) -> Summary {
        let count = |s| results.iter().flat_map(|f| &f.diagnostics).filter(|d| d.severity == s).count();
        Summary { errors: count(Severity::Error), warnings: count(Severity::Warning), fixed }
    }
}

#[derive(Serialize)]
struct Output<'a> {
    version: &'static str,
    files: &'a [FileResult],
    summary: Summary,
}

#[derive(Serialize)]
struct TextlintFile<'a> {
    #[serde(rename = "filePath")]
    file_path: &'a str,
    messages: Vec<TextlintMessage<'a>>,
}

#[derive(Serialize)]
struct Position {
    line: usize,
    column: usize,
}

/// textlint carries the span twice: `line`/`column` for the start, and `loc`
/// for both ends. Formatters use whichever they were written against, so a
/// result missing `loc` breaks the github and junit formatters.
#[derive(Serialize)]
struct Loc {
    start: Position,
    end: Position,
}

#[derive(Serialize)]
struct TextlintMessage<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    #[serde(rename = "ruleId")]
    rule_id: String,
    message: &'a str,
    line: usize,
    column: usize,
    index: usize,
    range: [usize; 2],
    loc: Loc,
    severity: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    fix: Option<&'a Fix>,
}

pub fn render(format: Format, results: &[FileResult], summary: Summary) -> String {
    match format {
        Format::Text => results
            .iter()
            .flat_map(|f| f.diagnostics.iter().map(move |d| (f, d)))
            .map(|(f, d)| format!("{}:{}:{}: {} [{}]\n", f.path, d.line, d.column, d.message.lines().next().unwrap_or(""), d.rule))
            .collect(),
        Format::Json => {
            let out = Output { version: env!("CARGO_PKG_VERSION"), files: results, summary };
            serde_json::to_string_pretty(&out).unwrap() + "\n"
        }
        Format::Textlint => {
            let out: Vec<TextlintFile> = results
                .iter()
                .map(|f| TextlintFile {
                    file_path: &f.path,
                    messages: f
                        .diagnostics
                        .iter()
                        .map(|d| TextlintMessage {
                            kind: "lint",
                            rule_id: format!("{TEXTLINT_RULE_PREFIX}{}", d.rule),
                            message: &d.message,
                            line: d.line,
                            column: d.column,
                            index: d.range[0],
                            range: d.range,
                            loc: Loc {
                                start: Position { line: d.line, column: d.column },
                                end: Position { line: d.end_line, column: d.end_column },
                            },
                            severity: if d.severity == Severity::Error { 2 } else { 1 },
                            fix: d.fix.as_ref(),
                        })
                        .collect(),
                })
                .collect();
            serde_json::to_string(&out).unwrap() + "\n"
        }
    }
}

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

impl Severity {
    pub fn parse(s: &str) -> Option<Severity> {
        match s {
            "error" => Some(Severity::Error),
            "warning" | "warn" => Some(Severity::Warning),
            _ => None,
        }
    }
}

/// A replacement over a UTF-16 offset range of the source document.
#[derive(Debug, Clone, Serialize)]
pub struct Fix {
    pub range: [usize; 2],
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    pub rule: String,
    pub severity: Severity,
    pub message: String,
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub end_column: usize,
    /// UTF-16 code unit offsets `[start, end)` from the start of the
    /// document. textlint reports the same unit, and so does LSP by default.
    pub range: [usize; 2],
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<Fix>,
}

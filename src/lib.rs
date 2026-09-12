pub mod config;
pub mod diagnostic;
pub mod document;
pub mod engine;
pub mod format;
pub mod rules;
pub mod sentence;
pub mod tokenizer;

pub use config::Config;
pub use diagnostic::{Diagnostic, Fix, Severity};
pub use engine::{apply_fixes, FileResult, FixResult, Linter};
pub use rules::TEXTLINT_RULE_PREFIX;

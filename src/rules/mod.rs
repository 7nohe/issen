//! Rule interface and registry.
//!
//! Rules see one block at a time (tokens and sentences precomputed) and report
//! byte ranges within the block text. Rules that need the whole document
//! (style consistency, required headings) collect during `check` and emit
//! from `finish`. Which blocks a rule sees is declared by `scope`, and the
//! engine enforces it, so a rule never re-checks block kinds itself.

pub mod document;
pub mod joshi;
pub mod style;
pub mod text;
pub mod verb;

use crate::config::Config;
use crate::document::{Block, BlockKind, Document};
use crate::sentence::Sentence;
use crate::tokenizer::Token;
use serde_yaml::Value;

/// A finding, in byte offsets of the block text (or of the source, for
/// `Anchor::Source` reports).
#[derive(Debug, Clone)]
pub struct Report {
    pub start: usize,
    pub end: usize,
    pub message: String,
    pub fix: Option<TextFix>,
}

#[derive(Debug, Clone)]
pub struct TextFix {
    pub start: usize,
    pub end: usize,
    pub text: String,
}

impl Report {
    pub fn at(start: usize, end: usize, message: impl Into<String>) -> Report {
        Report { start, end, message: message.into(), fix: None }
    }
    pub fn token(tok: &Token, message: impl Into<String>) -> Report {
        Report::at(tok.byte_start, tok.byte_end, message)
    }
    pub fn with_fix(mut self, start: usize, end: usize, text: impl Into<String>) -> Report {
        self.fix = Some(TextFix { start, end, text: text.into() });
        self
    }
    /// A fix that replaces exactly the reported range.
    pub fn replace(self, text: impl Into<String>) -> Report {
        let (s, e) = (self.start, self.end);
        self.with_fix(s, e, text)
    }
}

/// Which offset space a `finish` report uses.
#[derive(Debug, Clone, Copy)]
pub enum Anchor {
    /// Offsets are within the text of this block.
    Block(usize),
    /// Offsets are source bytes already (a document-level finding).
    Source,
}

/// The blocks a rule wants to see.
#[derive(Debug, Clone, Copy)]
pub struct Scope {
    pub kinds: &'static [BlockKind],
    /// textlint's ja rules mostly skip quoted text; the character-level ones do not.
    pub blockquote: bool,
}

impl Scope {
    /// Blocks textlint represents with a `Paragraph` node, where its
    /// sentence-scoped rules run.
    pub const PROSE: Scope = Scope { kinds: &[BlockKind::Paragraph, BlockKind::ListItem], blockquote: false };
    pub const PARAGRAPH: Scope = Scope { kinds: &[BlockKind::Paragraph], blockquote: false };
    pub const TEXT: Scope = Scope { kinds: BlockKind::ALL, blockquote: false };
    pub const EVERYWHERE: Scope = Scope { kinds: BlockKind::ALL, blockquote: true };

    pub fn admits(&self, block: &Block) -> bool {
        self.kinds.contains(&block.kind) && (self.blockquote || !block.in_blockquote)
    }
}

pub struct BlockData<'a> {
    pub index: usize,
    pub block: &'a Block,
    pub tokens: Vec<Token>,
    pub sentences: Vec<Sentence>,
}

impl<'a> BlockData<'a> {
    pub fn text(&self) -> &str {
        &self.block.text
    }
    pub fn sentence_text(&self, s: &Sentence) -> &str {
        &self.block.text[s.byte_range.clone()]
    }
    pub fn sentence_tokens(&self, s: &Sentence) -> &[Token] {
        &self.tokens[s.tokens.clone()]
    }
    /// Whether the text at `byte` sits inside a link or emphasis, which most
    /// textlint rules skip.
    pub fn in_link_or_emphasis(&self, byte: usize) -> bool {
        self.block.segment_at(byte).map(|s| s.in_link || s.in_emphasis).unwrap_or(false)
    }
}

pub struct Ctx<'a> {
    pub doc: &'a Document,
    pub options: Value,
}

impl<'a> Ctx<'a> {
    pub fn line_of(&self, blk: &BlockData, text_byte: usize) -> usize {
        self.doc.position(blk.block.to_source(text_byte)).0
    }
}

pub trait Rule {
    fn id(&self) -> &'static str;
    fn check(&mut self, blk: &BlockData, ctx: &Ctx) -> Vec<Report>;
    fn finish(&mut self, _ctx: &Ctx) -> Vec<(Anchor, Report)> {
        Vec::new()
    }
    fn scope(&self) -> Scope {
        Scope::TEXT
    }
    /// Whether the rule runs when the config does not mention it.
    fn default_enabled(&self) -> bool {
        true
    }
}

/// Prefix textlint uses for this preset's rule IDs.
pub const TEXTLINT_RULE_PREFIX: &str = "ja-technical-writing/";

fn registry() -> Vec<Box<dyn Rule>> {
    vec![
        Box::new(text::SentenceLength::default()),
        Box::new(text::MaxComma),
        Box::new(text::MaxKanjiContinuousLen),
        Box::new(text::NoExclamationQuestionMark),
        Box::new(text::NoZeroWidthSpaces),
        Box::new(text::NoInvalidControlCharacter),
        Box::new(text::NoNfd),
        Box::new(text::NoHankakuKana),
        Box::new(text::JaNoMixedPeriod),
        Box::new(joshi::MaxTen),
        Box::new(joshi::NoDoubledJoshi),
        Box::new(joshi::NoDoubledConjunctiveParticleGa),
        Box::new(joshi::NoDoubledConjunction),
        Box::new(verb::NoDroppingTheRa),
        Box::new(verb::NoDoubleNegativeJa),
        Box::new(verb::JaNoWeakPhrase),
        Box::new(style::NoMixDearuDesumasu::default()),
        Box::new(document::RequiredHeadings::default()),
        Box::new(document::Forbidden::default()),
        Box::new(document::Terminology::default()),
    ]
}

pub fn build(config: &Config) -> Vec<Box<dyn Rule>> {
    registry()
        .into_iter()
        .filter(|r| match config.rules.get(r.id()) {
            Some(Value::Bool(false)) => false,
            Some(_) => true,
            None => r.default_enabled(),
        })
        .collect()
}

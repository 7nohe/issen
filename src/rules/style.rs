//! no-mix-dearu-desumasu: keep 敬体 (ですます) and 常体 (である) consistent,
//! judged separately for headings, body paragraphs, and list items.
//! Port of textlint-rule-no-mix-dearu-desumasu + analyze-desumasu-dearu.

use super::{Anchor, BlockData, Ctx, Report, Rule, Scope};
use crate::config::{opt_bool, opt_str};
use crate::document::BlockKind;
use crate::tokenizer::Token;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Style {
    Desumasu,
    Dearu,
}

impl Style {
    fn name(self) -> &'static str {
        match self {
            Style::Desumasu => "ですます",
            Style::Dearu => "である",
        }
    }
    fn other(self) -> Style {
        match self {
            Style::Desumasu => Style::Dearu,
            Style::Dearu => Style::Desumasu,
        }
    }
    fn parse(s: &str) -> Option<Style> {
        match s {
            "ですます" => Some(Style::Desumasu),
            "である" => Some(Style::Dearu),
            _ => None,
        }
    }
}

struct Hit {
    block: usize,
    kind: BlockKind,
    style: Style,
    byte_start: usize,
    byte_end: usize,
    value: String,
}

/// The token after `idx` that ends the current clause: punctuation, a 特殊
/// conjugation (です/ます/だ family), or a noun.
fn next_puncture(tokens: &[Token], idx: usize) -> Option<usize> {
    (idx + 1..tokens.len()).find(|&i| {
        let t = &tokens[i];
        t.surface.contains(['、', '。']) || t.conjugated_type().contains("特殊") || t.pos() == "名詞"
    })
}

fn is_last_token(tokens: &[Token], idx: usize) -> bool {
    match next_puncture(tokens, idx) {
        None => true,
        // textlint tests the surface for any of these. Testing every character
        // instead would miss a clause end whenever the analyser merges a bracket
        // and a 句点 into one unknown token, as it does for ")。".
        Some(p) => tokens[p].surface.chars().any(|c| matches!(c, '!' | '?' | '！' | '？' | '。')),
    }
}

fn analyze(tokens: &[Token], ignore_conjunction: bool) -> Vec<(usize, Style)> {
    let mut out = Vec::new();
    for (i, t) in tokens.iter().enumerate() {
        let ct = t.conjugated_type();
        let style = if ct == "特殊・ダ" {
            let dearu = t.pos() == "助動詞"
                && t.conjugated_form() == "連用形"
                && tokens.get(i + 1).map(|n| n.conjugated_type() == "五段・ラ行アル").unwrap_or(false);
            if !dearu {
                continue;
            }
            Style::Dearu
        } else if (ct == "特殊・デス" || ct == "特殊・マス") && t.basic_form() != "やす" && t.conjugated_form() == "基本形" {
            Style::Desumasu
        } else {
            continue;
        };
        if !ignore_conjunction || is_last_token(tokens, i) {
            out.push((i, style));
        }
    }
    out
}

fn value_of(tokens: &[Token], idx: usize) -> String {
    let end = next_puncture(tokens, idx).unwrap_or(tokens.len() - 1);
    tokens[idx..=end].iter().map(|t| t.surface.as_str()).collect()
}

#[derive(Default)]
pub struct NoMixDearuDesumasu {
    hits: Vec<Hit>,
}

impl NoMixDearuDesumasu {
    /// Findings for one block kind: which style must yield, and every hit of the other.
    fn reports(&self, kind: BlockKind, label: &str, prefer: Option<Style>) -> Vec<(Anchor, Report)> {
        let hits: Vec<&Hit> = self.hits.iter().filter(|h| h.kind == kind).collect();
        let dearu = hits.iter().filter(|h| h.style == Style::Dearu).count();
        let desumasu = hits.len() - dearu;
        let over = match prefer {
            Some(Style::Desumasu) if dearu != 0 => Style::Desumasu,
            Some(Style::Dearu) if desumasu != 0 => Style::Dearu,
            None if dearu != 0 && desumasu != 0 => {
                if dearu > desumasu {
                    Style::Dearu
                } else {
                    Style::Desumasu
                }
            }
            _ => return Vec::new(),
        };
        let (want, other) = (over.name(), over.other().name());
        let top = if prefer.is_some() {
            format!("\"{want}\"調 でなければなりません\n=> \"{want}\"調 であるべき箇所に、次の \"{other}\"調 の箇所があります")
        } else {
            format!("\"である\"調 と \"ですます\"調 が混在\n=> \"{want}\"調 の文体に、次の \"{other}\"調 の箇所があります")
        };
        hits.iter()
            .filter(|h| h.style != over)
            .map(|h| {
                let msg = format!("{label}: {top}: \"{}\"\nTotal:\nである  : {dearu}\nですます: {desumasu}\n", h.value);
                (Anchor::Block(h.block), Report::at(h.byte_start, h.byte_end, msg))
            })
            .collect()
    }
}

impl Rule for NoMixDearuDesumasu {
    fn id(&self) -> &'static str {
        "no-mix-dearu-desumasu"
    }
    fn scope(&self) -> Scope {
        Scope { kinds: &[BlockKind::Paragraph, BlockKind::Heading, BlockKind::ListItem], blockquote: false }
    }
    fn check(&mut self, blk: &BlockData, ctx: &Ctx) -> Vec<Report> {
        let strict = opt_bool(&ctx.options, "strict", false);
        for (i, style) in analyze(&blk.tokens, !strict) {
            let t = &blk.tokens[i];
            self.hits.push(Hit {
                block: blk.index,
                kind: blk.block.kind,
                style,
                byte_start: t.byte_start,
                byte_end: t.byte_end,
                value: value_of(&blk.tokens, i),
            });
        }
        Vec::new()
    }
    fn finish(&mut self, ctx: &Ctx) -> Vec<(Anchor, Report)> {
        [
            (BlockKind::Paragraph, "本文", "preferInBody"),
            (BlockKind::Heading, "見出し", "preferInHeader"),
            (BlockKind::ListItem, "箇条書き", "preferInList"),
        ]
        .into_iter()
        .flat_map(|(kind, label, key)| self.reports(kind, label, Style::parse(&opt_str(&ctx.options, key, ""))))
        .collect()
    }
}

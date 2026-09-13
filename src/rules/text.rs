//! Rules that work on characters alone (no morphology needed).

use super::{BlockData, Ctx, Report, Rule, Scope};
use crate::config::{matchers, opt_bool, opt_str, opt_strs, opt_usize, Matcher};
use crate::document::{utf16_len, SegmentKind};
use regex::Regex;
use std::sync::OnceLock;
use unicode_normalization::UnicodeNormalization;

/// Han characters as textlint's kanji rules define them.
pub fn is_kanji(c: char) -> bool {
    matches!(c, '々' | '〇' | '〻' | '\u{3400}'..='\u{9FFF}' | '\u{F900}'..='\u{FAFF}' | '\u{20000}'..='\u{2FFFF}')
}

fn is_japanese(text: &str) -> bool {
    text.chars().any(|c| is_kanji(c) || matches!(c, 'ぁ'..='ん' | 'ァ'..='ヶ'))
}

/// sentence-length: at most `max` per sentence, counted in UTF-16 code units
/// (`countBy: codepoints` counts characters instead). Like textlint, a link
/// whose text is its own URL is not counted (`skipUrlStringLink`), a paragraph
/// that is only a link is skipped, and `skipPatterns` are removed before
/// counting.
#[derive(Default)]
pub struct SentenceLength {
    skip_patterns: Option<Vec<Matcher>>,
}
impl Rule for SentenceLength {
    fn id(&self) -> &'static str {
        "sentence-length"
    }
    fn scope(&self) -> Scope {
        Scope::PROSE
    }
    fn validate(&self, options: &yaml_serde::Value) -> Result<(), String> {
        matchers(options, "skipPatterns").map(|_| ())?;
        match opt_str(options, "countBy", "codeunits").as_str() {
            "codeunits" | "codepoints" => Ok(()),
            other => Err(format!("sentence-length.countBy: unknown value \"{other}\". Use codeunits or codepoints.")),
        }
    }
    fn check(&mut self, blk: &BlockData, ctx: &Ctx) -> Vec<Report> {
        let max = opt_usize(&ctx.options, "max", 100);
        let skip_url_links = opt_bool(&ctx.options, "skipUrlStringLink", true);
        let skip_patterns = self.skip_patterns.get_or_insert_with(|| matchers(&ctx.options, "skipPatterns").unwrap_or_default());
        if blk.block.segments.iter().all(|s| s.in_link || s.kind == SegmentKind::Break) {
            return Vec::new();
        }

        let mut out = Vec::new();
        for s in &blk.sentences {
            let mut text = blk.block.text_where(s.byte_range.clone(), |seg| !(skip_url_links && seg.url_link));
            for m in skip_patterns.iter() {
                text = m.remove_from(&text);
            }
            let len = if opt_str(&ctx.options, "countBy", "codeunits") == "codepoints" { text.chars().count() } else { utf16_len(&text) };
            if len > max {
                let line = ctx.line_of(blk, s.byte_range.start);
                out.push(Report::at(
                    s.byte_range.start,
                    s.byte_range.end,
                    format!(
                        "Line {line} sentence length({len}) exceeds the maximum sentence length of {max}.\nOver {} characters.",
                        len - max
                    ),
                ));
            }
        }
        out
    }
}

pub struct MaxComma;
impl Rule for MaxComma {
    fn id(&self) -> &'static str {
        "max-comma"
    }
    fn scope(&self) -> Scope {
        Scope::PROSE
    }
    fn check(&mut self, blk: &BlockData, ctx: &Ctx) -> Vec<Report> {
        let max = opt_usize(&ctx.options, "max", 4);
        let mut out = Vec::new();
        for s in &blk.sentences {
            let text = blk.sentence_text(s);
            if text.matches(',').count() > max {
                let at = s.byte_range.start + text.rfind(',').unwrap();
                out.push(Report::at(at, at + 1, format!("This sentence exceeds the maximum count of comma. Maximum is {max}.")));
            }
        }
        out
    }
}

pub struct MaxKanjiContinuousLen;
impl Rule for MaxKanjiContinuousLen {
    fn id(&self) -> &'static str {
        "max-kanji-continuous-len"
    }
    fn check(&mut self, blk: &BlockData, ctx: &Ctx) -> Vec<Report> {
        let max = opt_usize(&ctx.options, "max", 5);
        let allow = opt_strs(&ctx.options, "allow");
        let mut out = Vec::new();
        let text = blk.text();
        let mut run_start: Option<usize> = None;
        for (i, c) in text.char_indices().chain(std::iter::once((text.len(), ' '))) {
            match (is_kanji(c), run_start) {
                (true, None) => run_start = Some(i),
                (false, Some(start)) => {
                    let run = &text[start..i];
                    if utf16_len(run) > max && !allow.iter().any(|a| a == run) && !blk.in_link_or_emphasis(start) {
                        out.push(Report::at(start, i, format!("漢字が{}つ以上連続しています: {run}", max + 1)));
                    }
                    run_start = None;
                }
                _ => {}
            }
        }
        out
    }
}

/// no-exclamation-question-mark, with textlint's options: `allow` phrases
/// (plus the built-in "Yahoo!") and per-width allow flags; link text is skipped.
pub struct NoExclamationQuestionMark;
impl Rule for NoExclamationQuestionMark {
    fn id(&self) -> &'static str {
        "no-exclamation-question-mark"
    }
    fn check(&mut self, blk: &BlockData, ctx: &Ctx) -> Vec<Report> {
        let allowed = |c: char| match c {
            '!' => opt_bool(&ctx.options, "allowHalfWidthExclamation", false),
            '！' => opt_bool(&ctx.options, "allowFullWidthExclamation", false),
            '?' => opt_bool(&ctx.options, "allowHalfWidthQuestion", false),
            '？' => opt_bool(&ctx.options, "allowFullWidthQuestion", false),
            _ => true,
        };
        let mut allow = opt_strs(&ctx.options, "allow");
        allow.push("Yahoo!".to_string());
        let text = blk.text();
        // An allowed phrase covers the mark when the phrase ends with it ("Yahoo!").
        let in_allowed_phrase = |end: usize| allow.iter().any(|p| text[..end].ends_with(p.as_str()));

        text.char_indices()
            .filter(|&(i, c)| !allowed(c) && !blk.in_link_or_emphasis(i) && !in_allowed_phrase(i + c.len_utf8()))
            .map(|(i, c)| Report::at(i, i + c.len_utf8(), format!("Disallow to use \"{c}\".")))
            .collect()
    }
}

/// no-zero-width-spaces. Only U+200B, as textlint has it. The neighbouring
/// invisibles are load-bearing: U+200D joins the parts of an emoji, and
/// deleting it rewrites 👨‍👩‍👧‍👦 into four separate people.
pub struct NoZeroWidthSpaces;
impl Rule for NoZeroWidthSpaces {
    fn id(&self) -> &'static str {
        "no-zero-width-spaces"
    }
    fn scope(&self) -> Scope {
        Scope::EVERYWHERE
    }
    fn check(&mut self, blk: &BlockData, _ctx: &Ctx) -> Vec<Report> {
        blk.text()
            .char_indices()
            .filter(|(_, c)| *c == '\u{200B}')
            .map(|(i, c)| Report::at(i, i + c.len_utf8(), "Zero width space is disallowed.").replace(""))
            .collect()
    }
}

/// Names for the control characters, so the message reads like textlint's.
const CONTROL_NAMES: [&str; 33] = [
    "NULL",
    "START OF HEADING",
    "START OF TEXT",
    "END OF TEXT",
    "END OF TRANSMISSION",
    "ENQUIRY",
    "ACKNOWLEDGE",
    "BELL",
    "BACKSPACE",
    "CHARACTER TABULATION",
    "LINE FEED (LF)",
    "LINE TABULATION",
    "FORM FEED (FF)",
    "CARRIAGE RETURN (CR)",
    "SHIFT OUT",
    "SHIFT IN",
    "DATA LINK ESCAPE",
    "DEVICE CONTROL ONE",
    "DEVICE CONTROL TWO",
    "DEVICE CONTROL THREE",
    "DEVICE CONTROL FOUR",
    "NEGATIVE ACKNOWLEDGE",
    "SYNCHRONOUS IDLE",
    "END OF TRANSMISSION BLOCK",
    "CANCEL",
    "END OF MEDIUM",
    "SUBSTITUTE",
    "ESCAPE",
    "INFORMATION SEPARATOR FOUR",
    "INFORMATION SEPARATOR THREE",
    "INFORMATION SEPARATOR TWO",
    "INFORMATION SEPARATOR ONE",
    "DELETE",
];

fn control_name(c: char) -> &'static str {
    match c as u32 {
        n @ 0..=0x1F => CONTROL_NAMES[n as usize],
        0x7F => CONTROL_NAMES[32],
        _ => "CONTROL",
    }
}

pub struct NoInvalidControlCharacter;
impl Rule for NoInvalidControlCharacter {
    fn id(&self) -> &'static str {
        "no-invalid-control-character"
    }
    fn scope(&self) -> Scope {
        Scope::EVERYWHERE
    }
    fn check(&mut self, blk: &BlockData, _ctx: &Ctx) -> Vec<Report> {
        blk.text()
            .char_indices()
            .filter(|(_, c)| (c.is_control() && !matches!(c, '\t' | '\n' | '\r')) || *c == '\u{7F}')
            .map(|(i, c)| {
                let msg = format!("Found invalid control character({} \\u{:04X})", control_name(c), c as u32);
                Report::at(i, i + c.len_utf8(), msg).replace("")
            })
            .collect()
    }
}

pub struct NoNfd;
impl Rule for NoNfd {
    fn id(&self) -> &'static str {
        "no-nfd"
    }
    fn check(&mut self, blk: &BlockData, _ctx: &Ctx) -> Vec<Report> {
        let text = blk.text();
        let mut out = Vec::new();
        let mut prev: Option<(usize, char)> = None;
        for (i, c) in text.char_indices() {
            if let (true, Some((pi, pc))) = (matches!(c, '\u{3099}' | '\u{309A}' | '\u{309B}' | '\u{309C}'), prev) {
                let combining = match c {
                    '\u{309B}' => '\u{3099}',
                    '\u{309C}' => '\u{309A}',
                    other => other,
                };
                let pair: String = [pc, c].iter().collect();
                let expected: String = [pc, combining].iter().collect::<String>().nfc().collect();
                out.push(
                    Report::at(
                        i,
                        i + c.len_utf8(),
                        format!("Disallow to use NFD(well-known as UTF8-MAC 濁点): \"{pair}\" => \"{expected}\""),
                    )
                    .with_fix(pi, i + c.len_utf8(), expected),
                );
            }
            prev = Some((i, c));
        }
        out
    }
}

pub struct NoHankakuKana;
impl Rule for NoHankakuKana {
    fn id(&self) -> &'static str {
        "no-hankaku-kana"
    }
    fn check(&mut self, blk: &BlockData, _ctx: &Ctx) -> Vec<Report> {
        static RE: OnceLock<Regex> = OnceLock::new();
        let re = RE.get_or_init(|| Regex::new(r"[\u{FF61}-\u{FF9F}]+").unwrap());
        re.find_iter(blk.text())
            .filter(|m| !blk.in_link_or_emphasis(m.start()))
            .map(|m| {
                let fixed: String = m.as_str().nfkc().collect();
                Report::at(m.start(), m.end(), format!("Disallow to use 半角カタカナ: \"{}\"", m.as_str())).replace(fixed)
            })
            .collect()
    }
}

/// ja-no-mixed-period: a paragraph must end with the configured period mark.
/// Like textlint, only a paragraph whose last node is plain text is checked;
/// one that ends in inline code or a link is left alone.
pub struct JaNoMixedPeriod;
impl Rule for JaNoMixedPeriod {
    fn id(&self) -> &'static str {
        "ja-no-mixed-period"
    }
    fn scope(&self) -> Scope {
        Scope::PARAGRAPH
    }
    fn check(&mut self, blk: &BlockData, ctx: &Ctx) -> Vec<Report> {
        let Some(last) = blk.block.last_content_segment() else { return Vec::new() };
        // textlint only checks a paragraph whose last child is a plain text node,
        // so inline code, a link and emphasis all end the check.
        if last.kind != SegmentKind::Text || last.in_link || last.in_emphasis || last.in_strong {
            return Vec::new();
        }
        let prefer = opt_str(&ctx.options, "periodMark", "。");
        let mut allowed = opt_strs(&ctx.options, "allowPeriodMarks");
        allowed.push(prefer.clone());

        let text = blk.text()[last.text_start..last.text_end].trim_end();
        // The "is this Japanese at all" test runs over the whole text node, which
        // is wider than this segment whenever a soft break splits one.
        if text.is_empty() || !blk.block.last_text_node().map(is_japanese).unwrap_or(false) {
            return Vec::new();
        }
        let (rel, ch) = text.char_indices().last().unwrap();
        let idx = last.text_start + rel;
        // check-ends-with-period's own exception set, kept verbatim for parity.
        if matches!(ch, '!' | '?' | '！' | '？' | ')' | '）' | '」' | '』') || allowed.iter().any(|m| m == &ch.to_string()) {
            return Vec::new();
        }
        let report = Report::at(idx, idx + ch.len_utf8(), format!("文末が\"{prefer}\"で終わっていません。"));
        if matches!(ch, '.' | '．') {
            vec![report.replace(prefer)]
        } else {
            vec![report]
        }
    }
}

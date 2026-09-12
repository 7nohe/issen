//! Sentence splitting over block text, following textlint's sentence-splitter:
//! a sentence ends at 。！？ (or their ASCII forms), never inside an open
//! bracket or quote pair, and never at a line break. An ASCII period only
//! counts when followed by whitespace or the end of the block, so
//! `GitHub.com` stays whole.

use crate::tokenizer::Token;
use std::ops::Range;

#[derive(Debug, Clone)]
pub struct Sentence {
    /// Byte range within the block text, separators included, whitespace trimmed.
    pub byte_range: Range<usize>,
    /// Index range into the block's token list.
    pub tokens: Range<usize>,
}

/// Characters that end a sentence. Rules that count within a sentence share
/// this list so they never disagree with the splitter about boundaries.
pub const TERMINATORS: [char; 6] = ['。', '！', '？', '!', '?', '．'];

const PAIRS: [(char, char); 10] = [
    ('「', '」'),
    ('『', '』'),
    ('（', '）'),
    ('(', ')'),
    ('[', ']'),
    ('{', '}'),
    ('【', '】'),
    ('《', '》'),
    ('〈', '〉'),
    ('“', '”'),
];

pub fn is_terminator(c: char) -> bool {
    TERMINATORS.contains(&c)
}

fn ends_here(text: &str, idx: usize, ch: char) -> bool {
    match ch {
        '.' => {
            let rest = &text[idx + 1..];
            rest.is_empty() || rest.chars().next().map(|c| c.is_whitespace()).unwrap_or(true)
        }
        c => is_terminator(c),
    }
}

pub fn split(text: &str, tokens: &[Token]) -> Vec<Sentence> {
    let mut sentences = Vec::new();
    let mut start = 0usize;
    let mut open: Vec<char> = Vec::new();
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut i = 0;
    while i < chars.len() {
        let (idx, ch) = chars[i];
        if let Some(&(_, close)) = PAIRS.iter().find(|(o, _)| *o == ch) {
            open.push(close);
        } else if open.last() == Some(&ch) {
            open.pop();
        } else if ch == '"' {
            if open.last() == Some(&'"') {
                open.pop();
            } else {
                open.push('"');
            }
        } else if open.is_empty() && ends_here(text, idx, ch) {
            // Absorb a run of terminators such as "!?" or "。。".
            let mut j = i;
            while j + 1 < chars.len() && ends_here(text, chars[j + 1].0, chars[j + 1].1) {
                j += 1;
            }
            let end = chars.get(j + 1).map(|c| c.0).unwrap_or(text.len());
            push(text, tokens, &mut sentences, start, end);
            start = end;
            i = j + 1;
            continue;
        }
        i += 1;
    }
    push(text, tokens, &mut sentences, start, text.len());
    sentences
}

fn push(text: &str, tokens: &[Token], out: &mut Vec<Sentence>, start: usize, end: usize) {
    let slice = &text[start..end];
    let trimmed_start = start + (slice.len() - slice.trim_start().len());
    let trimmed_end = trimmed_start + slice.trim().len();
    if trimmed_start >= trimmed_end {
        return;
    }
    let first = tokens.partition_point(|t| t.byte_start < trimmed_start);
    let last = tokens.partition_point(|t| t.byte_start < trimmed_end);
    out.push(Sentence { byte_range: trimmed_start..trimmed_end, tokens: first..last });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(s: &str) -> Vec<&str> {
        split(s, &[]).into_iter().map(|x| &s[x.byte_range]).collect()
    }

    #[test]
    fn splits_on_japanese_terminators() {
        assert_eq!(texts("一つ目。二つ目！三つ目"), vec!["一つ目。", "二つ目！", "三つ目"]);
    }

    #[test]
    fn ascii_period_needs_trailing_space() {
        assert_eq!(texts("GitHub.com を使う。").len(), 1);
    }

    #[test]
    fn line_breaks_and_quotes_do_not_end_a_sentence() {
        assert_eq!(texts("起動時に、\nキャッシュを破棄します。").len(), 1);
        assert_eq!(texts("「保存します。」と表示されます。次の文。"), vec!["「保存します。」と表示されます。", "次の文。"]);
    }
}

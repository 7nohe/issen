//! Particle and conjunction rules (token-based, sentence-scoped).

use super::{BlockData, Ctx, Report, Rule, Scope};
use crate::config::{opt_bool, opt_str, opt_strs, opt_strs_or, opt_usize};
use crate::tokenizer::{is_joshi_pos, Token};
use std::borrow::Cow;
use std::collections::BTreeMap;

/// Sentence terminators as token surfaces (sentence::TERMINATORS plus ASCII ".").
const SEPARATORS: &[&str] = &["。", "！", "？", "!", "?", "．", "."];
const COMMAS: &[&str] = &["、", "，"];

fn is_bracket(pos: &str, pd1: &str) -> bool {
    pos == "記号" && (pd1 == "括弧開" || pd1 == "括弧閉")
}

/// max-ten: at most `max` 読点 per sentence. A 読点 sandwiched between two
/// nouns (enumeration) is not counted unless `strict`.
pub struct MaxTen;
impl Rule for MaxTen {
    fn id(&self) -> &'static str {
        "max-ten"
    }
    fn scope(&self) -> Scope {
        Scope::PROSE
    }
    fn check(&mut self, blk: &BlockData, ctx: &Ctx) -> Vec<Report> {
        let max = opt_usize(&ctx.options, "max", 3);
        let strict = opt_bool(&ctx.options, "strict", false);
        let touten = opt_str(&ctx.options, "touten", "、");
        let kuten = opt_str(&ctx.options, "kuten", "。");

        // The nearest non-bracket token before/after idx.
        fn sibling(tokens: &[Token], idx: usize, back: bool) -> Option<&Token> {
            let mut i = idx;
            loop {
                i = if back { i.checked_sub(1)? } else { i + 1 };
                let t = tokens.get(i)?;
                if !(is_bracket(t.pos(), t.pos_detail_1()) || t.surface == "(" || t.surface == ")") {
                    return Some(t);
                }
            }
        }

        let mut out = Vec::new();
        for s in &blk.sentences {
            let tokens = blk.sentence_tokens(s);
            let mut count = 0usize;
            let mut last: Option<&Token> = None;
            for (i, t) in tokens.iter().enumerate() {
                if t.surface == touten {
                    let sandwiched = matches!((sibling(tokens, i, true), sibling(tokens, i, false)), (Some(b), Some(a)) if b.pos() == "名詞" && a.pos() == "名詞");
                    if sandwiched && !strict {
                        continue;
                    }
                    count += 1;
                    last = Some(t);
                }
                if SEPARATORS.contains(&t.surface.as_str()) || t.surface == kuten {
                    count = 0;
                }
                if count > max {
                    if let Some(l) = last {
                        out.push(Report::token(l, format!("一つの文で\"{touten}\"を{}つ以上使用しています", max + 1)));
                    }
                    count = 0;
                }
            }
        }
        out
    }
}

/// A particle after merging adjacent particles (連語) into one, as textlint
/// does. Fields borrow from the token unless a merge made them longer.
struct Joshi<'a> {
    surface: Cow<'a, str>,
    pos: Cow<'a, str>,
    pd1: Cow<'a, str>,
    pd2: &'a str,
    pd3: &'a str,
    byte_start: usize,
    byte_end: usize,
    /// Index of the merged token before this one.
    prev: Option<usize>,
}

impl Joshi<'_> {
    fn is_joshi(&self) -> bool {
        is_joshi_pos(&self.pos)
    }
}

fn merge_joshi(tokens: &[Token]) -> Vec<Joshi<'_>> {
    let mut out: Vec<Joshi> = Vec::new();
    for t in tokens.iter().filter(|t| !t.is_whitespace()) {
        if let Some(prev) = out.last_mut().filter(|p| t.is_joshi() && p.is_joshi()) {
            prev.surface.to_mut().push_str(&t.surface);
            prev.pos.to_mut().push_str(t.pos());
            prev.pd1.to_mut().push_str(&t.surface);
            prev.byte_end = t.byte_end;
            continue;
        }
        out.push(Joshi {
            surface: Cow::Borrowed(&t.surface),
            pos: Cow::Borrowed(t.pos()),
            pd1: Cow::Borrowed(t.pos_detail_1()),
            pd2: t.pos_detail_2(),
            pd3: t.pos_detail_3(),
            byte_start: t.byte_start,
            byte_end: t.byte_end,
            prev: out.len().checked_sub(1),
        });
    }
    out
}

/// no-doubled-joshi: the same particle used twice within `min_interval`
/// countable tokens of one sentence. Port of textlint-rule-no-doubled-joshi.
pub struct NoDoubledJoshi;
impl Rule for NoDoubledJoshi {
    fn id(&self) -> &'static str {
        "no-doubled-joshi"
    }
    fn scope(&self) -> Scope {
        Scope::PROSE
    }
    fn check(&mut self, blk: &BlockData, ctx: &Ctx) -> Vec<Report> {
        let min_interval = opt_usize(&ctx.options, "min_interval", 1).max(1);
        let strict = opt_bool(&ctx.options, "strict", false);
        let allow = opt_strs(&ctx.options, "allow");
        let separators = opt_strs_or(&ctx.options, "separatorCharacters", SEPARATORS);
        let commas = opt_strs_or(&ctx.options, "commaCharacters", COMMAS);

        let mut out = Vec::new();
        for s in &blk.sentences {
            let all = merge_joshi(blk.sentence_tokens(s));
            let countable: Vec<&Joshi> = all
                .iter()
                .filter(|j| {
                    let comma = commas.iter().any(|c| c == &j.surface) && (j.pos == "名詞" || j.pos == "記号");
                    j.is_joshi() || (!strict && (is_bracket(&j.pos, &j.pd1) || separators.iter().any(|c| c == &j.surface) || comma))
                })
                .collect();

            let mut by_key: BTreeMap<(&str, &str, &str, &str, &str), Vec<usize>> = BTreeMap::new();
            for (i, j) in countable.iter().enumerate() {
                if j.is_joshi() {
                    by_key.entry((&j.surface, &j.pos, &j.pd1, j.pd2, j.pd3)).or_default().push(i);
                }
            }

            for ((name, _, _, _, _), positions) in by_key {
                if positions.len() <= 1 || allow.iter().any(|a| a == name) {
                    continue;
                }
                let first = countable[positions[0]];
                let second = countable[positions[1]];
                let pair = positions.len() == 2;
                let prev_of = |j: &Joshi| j.prev.map(|i| all[i].surface.as_ref());
                let exception = !strict
                    && (first.pd1 == "連体化"
                        || (first.pd1 == "格助詞" && first.surface == "を")
                        || (first.pd1 == "接続助詞" && first.surface == "て")
                        || (pair && first.pd1 == "並立助詞" && second.pd1 == "並立助詞")
                        // The 〜かどうか construction, where the repeat is idiomatic.
                        || (pair && first.surface == "か" && second.surface == "か" && prev_of(second) == Some("どう")));
                if exception {
                    continue;
                }
                for w in positions.windows(2) {
                    let (a, b) = (countable[w[0]], countable[w[1]]);
                    if w[1] - w[0] <= min_interval {
                        let word = |j: &Joshi| match prev_of(j) {
                            Some(p) => format!("{p}\"{}\"", j.surface),
                            None => format!("\"{}\"", j.surface),
                        };
                        out.push(Report::at(
                            b.byte_start,
                            b.byte_end,
                            format!(
                                "一文に二回以上利用されている助詞 \"{name}\" がみつかりました。\n\n次の助詞が連続しているため、文を読みにくくしています。\n\n- {}\n- {}\n\n同じ助詞を連続して利用しない、文の中で順番を入れ替える、文を分割するなどを検討してください。",
                                word(a),
                                word(b)
                            ),
                        ));
                    }
                }
            }
        }
        out
    }
}

/// no-doubled-conjunctive-particle-ga: 逆接の「が」 twice in one sentence.
pub struct NoDoubledConjunctiveParticleGa;
impl Rule for NoDoubledConjunctiveParticleGa {
    fn id(&self) -> &'static str {
        "no-doubled-conjunctive-particle-ga"
    }
    fn scope(&self) -> Scope {
        Scope::PROSE
    }
    fn check(&mut self, blk: &BlockData, ctx: &Ctx) -> Vec<Report> {
        let separators = opt_strs_or(&ctx.options, "separatorChars", SEPARATORS);
        let mut out = Vec::new();
        for s in &blk.sentences {
            for group in blk.sentence_tokens(s).split(|t| separators.iter().any(|c| c == &t.surface)) {
                let ga: Vec<&Token> = group.iter().filter(|t| t.pos_detail_1() == "接続助詞" && t.surface == "が").collect();
                if ga.len() > 1 {
                    out.push(Report::token(ga[0], "文中に逆接の接続助詞 \"が\" が二回以上使われています。"));
                }
            }
        }
        out
    }
}

/// no-doubled-conjunction: the same sentence-initial conjunction in two
/// consecutive sentences of a paragraph.
pub struct NoDoubledConjunction;
impl Rule for NoDoubledConjunction {
    fn id(&self) -> &'static str {
        "no-doubled-conjunction"
    }
    fn scope(&self) -> Scope {
        Scope::PROSE
    }
    fn check(&mut self, blk: &BlockData, _ctx: &Ctx) -> Vec<Report> {
        // textlint skips a conjunction right after a whitespace token; Lindera
        // emits no whitespace tokens, so look at the character instead. It
        // tokenizes one sentence at a time, so the lookback stops at the sentence
        // start: the line break before a sentence belongs to no sentence at all.
        let per_sentence: Vec<Vec<&Token>> = blk
            .sentences
            .iter()
            .map(|s| {
                let start = s.byte_range.start;
                let after_space = |t: &Token| {
                    t.byte_start > start && blk.text()[start..t.byte_start].chars().next_back().map(char::is_whitespace).unwrap_or(false)
                };
                blk.sentence_tokens(s).iter().filter(|t| t.pos() == "接続詞" && !after_space(t)).collect()
            })
            .collect();

        let mut out = Vec::new();
        let mut carried: Option<&Token> = None;
        for pair in per_sentence.windows(2) {
            let (prev, cur) = (&pair[0], &pair[1]);
            let token = prev.first().copied().or(carried);
            if let (Some(t), Some(c)) = (token, cur.first()) {
                if c.surface == t.surface {
                    out.push(Report::token(c, format!("同じ接続詞（{}）が連続して使われています。", c.surface)));
                }
            }
            carried = token;
        }
        out
    }
}

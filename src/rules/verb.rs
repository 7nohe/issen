//! Verb-form and phrase rules: ら抜き, double negatives, weak phrases.
//! All three are token-sequence patterns over IPADIC features.

use super::{BlockData, Ctx, Report, Rule, Scope};
use crate::tokenizer::Token;
use std::sync::OnceLock;

/// One position in a token pattern. Every set field must match; a field with
/// several values matches any of them.
#[derive(Default, Clone, Copy)]
pub struct Expect {
    pub surface: &'static [&'static str],
    pub pos: &'static [&'static str],
    pub pos_detail_1: &'static [&'static str],
    pub conjugated_type: &'static [&'static str],
    pub conjugated_form: &'static [&'static str],
    pub basic_form: &'static [&'static str],
    pub reading: &'static [&'static str],
}

impl Expect {
    fn matches(&self, t: &Token) -> bool {
        let ok = |vals: &[&str], actual: &str| vals.is_empty() || vals.contains(&actual);
        ok(self.surface, &t.surface)
            && ok(self.pos, t.pos())
            && ok(self.pos_detail_1, t.pos_detail_1())
            && ok(self.conjugated_type, t.conjugated_type())
            && ok(self.conjugated_form, t.conjugated_form())
            && ok(self.basic_form, t.basic_form())
            && ok(self.reading, t.reading())
    }
}

/// A pattern is a sequence of alternatives per position.
pub struct Pattern {
    pub label: &'static str,
    pub steps: Vec<Vec<Expect>>,
}

impl Pattern {
    fn new(label: &'static str, steps: &[&[Expect]]) -> Pattern {
        Pattern { label, steps: steps.iter().map(|s| s.to_vec()).collect() }
    }
}

/// Run every pattern over the block's non-whitespace tokens. `gate` is a cheap
/// per-token test that every pattern needs somewhere; a block with no token
/// passing it is skipped without matching. `report` builds the finding from
/// the pattern and the first/last matched token indices.
fn scan(
    blk: &BlockData,
    patterns: &[Pattern],
    gate: impl Fn(&Token) -> bool,
    report: impl Fn(&Pattern, usize, usize) -> Report,
) -> Vec<Report> {
    if !blk.tokens.iter().any(gate) {
        return Vec::new();
    }
    let idx: Vec<usize> = (0..blk.tokens.len()).filter(|&i| !blk.tokens[i].is_whitespace()).collect();
    let mut out = Vec::new();
    for p in patterns {
        let n = p.steps.len();
        let mut i = 0;
        while n > 0 && i + n <= idx.len() {
            if (0..n).all(|k| p.steps[k].iter().any(|e| e.matches(&blk.tokens[idx[i + k]]))) {
                out.push(report(p, idx[i], idx[i + n - 1]));
                i += n;
            } else {
                i += 1;
            }
        }
    }
    out.sort_by_key(|r| r.start);
    out
}

macro_rules! ex {
    ($($field:ident : [$($v:expr),*]),* $(,)?) => {
        Expect { $($field: &[$($v),*],)* ..Default::default() }
    };
}

const NAI: &[&str] = &["ない", "無い"];
const NAI_ANY: Expect = Expect { basic_form: NAI, ..EMPTY };
const NAI_ADJ: Expect = Expect { basic_form: NAI, pos: &["形容詞"], ..EMPTY };
const EMPTY: Expect =
    Expect { surface: &[], pos: &[], pos_detail_1: &[], conjugated_type: &[], conjugated_form: &[], basic_form: &[], reading: &[] };

fn joshi(s: &'static [&'static str]) -> Expect {
    Expect { surface: s, pos: &["助詞"], ..EMPTY }
}

/// no-dropping-the-ra: an 一段 verb in 未然形 followed by the suffix れる, as in
/// 食べれる, plus the fixed forms 来れる and 見れる.
fn ra_patterns() -> &'static [Pattern] {
    static P: OnceLock<Vec<Pattern>> = OnceLock::new();
    P.get_or_init(|| {
        vec![
            Pattern::new("ら抜き", &[&[ex!(pos: ["動詞"], basic_form: ["来れる", "見れる"])]]),
            Pattern::new(
                "ら抜き",
                &[
                    &[ex!(pos: ["動詞"], pos_detail_1: ["自立"], conjugated_type: ["一段"], conjugated_form: ["未然形"])],
                    &[ex!(pos: ["動詞"], pos_detail_1: ["接尾"], basic_form: ["れる"])],
                ],
            ),
        ]
    })
}

pub struct NoDroppingTheRa;
impl Rule for NoDroppingTheRa {
    fn id(&self) -> &'static str {
        "no-dropping-the-ra"
    }
    fn check(&mut self, blk: &BlockData, _ctx: &Ctx) -> Vec<Report> {
        scan(blk, ra_patterns(), |t| t.pos() == "動詞", |_, _, last| Report::token(&blk.tokens[last], "ら抜き言葉を使用しています。"))
    }
}

/// no-double-negative-ja. Every construction comes in a は / も pair.
fn double_negative_patterns() -> &'static [Pattern] {
    static P: OnceLock<Vec<Pattern>> = OnceLock::new();
    P.get_or_init(|| {
        let da_renyou = ex!(basic_form: ["だ"], conjugated_form: ["連用形"]);
        let koto = ex!(reading: ["コト"], pos: ["名詞"]);
        let mono = ex!(reading: ["モノ"], pos: ["名詞"]);
        let wake = ex!(reading: ["ワケ"], pos: ["名詞"]);
        let naku = ex!(surface: ["なく", "無く"], basic_form: ["ない", "無い"]);
        let de = ex!(surface: ["で"]);
        // (label with は, label with も, steps before the particle, step after it)
        let pair = |ha: &'static str, mo: &'static str, head: &[&[Expect]], last: &[Expect]| {
            [(ha, [joshi(&["は"])]), (mo, [joshi(&["も"])])].map(|(label, particle)| {
                let steps: Vec<&[Expect]> = head.iter().copied().chain([&particle[..], last]).collect();
                Pattern::new(label, &steps)
            })
        };
        let mut out: Vec<Pattern> = [
            pair(
                "ないことはない",
                "ないこともない",
                &[&[NAI_ANY], &[koto]],
                &[Expect { conjugated_type: &["特殊・ナイ"], ..NAI_ANY }, NAI_ADJ],
            ),
            pair("なくはない", "なくもない", &[&[naku]], &[NAI_ANY]),
            pair("ないではない", "ないでもない", &[&[NAI_ANY], &[de]], &[NAI_ADJ]),
            pair("ないものではない", "ないものでもない", &[&[NAI_ANY], &[mono], &[da_renyou]], &[NAI_ADJ]),
            pair("ないわけではない", "ないわけでもない", &[&[NAI_ANY], &[wake], &[da_renyou]], &[NAI_ADJ]),
        ]
        .into_iter()
        .flatten()
        .collect();
        out.push(Pattern::new(
            "ないとはいいきれない",
            &[
                &[NAI_ANY],
                &[joshi(&["と"])],
                &[joshi(&["は"])],
                &[ex!(basic_form: ["言い切れる", "いい切れる", "言いきれる", "いいきれる"])],
                &[NAI_ANY],
            ],
        ));
        out.push(Pattern::new(
            "ないとはかぎらない",
            &[&[NAI_ANY], &[joshi(&["と"])], &[joshi(&["は"])], &[ex!(basic_form: ["限る", "かぎる"])], &[NAI_ANY]],
        ));
        out
    })
}

pub struct NoDoubleNegativeJa;
impl Rule for NoDoubleNegativeJa {
    fn id(&self) -> &'static str {
        "no-double-negative-ja"
    }
    fn scope(&self) -> Scope {
        Scope::EVERYWHERE
    }
    fn check(&mut self, blk: &BlockData, _ctx: &Ctx) -> Vec<Report> {
        // textlint reports at the token that completes the pattern.
        scan(
            blk,
            double_negative_patterns(),
            |t| NAI.contains(&t.basic_form()),
            |p, _, last| Report::token(&blk.tokens[last], format!("二重否定: 〜{}", p.label)),
        )
    }
}

fn weak_phrase_patterns() -> &'static [Pattern] {
    static P: OnceLock<Vec<Pattern>> = OnceLock::new();
    P.get_or_init(|| {
        let kamo = ex!(surface: ["かも"], pos: ["助詞"], pos_detail_1: ["副助詞"]);
        vec![
            Pattern::new("かも", &[&[kamo], &[ex!(surface: ["。"], pos: ["記号"])]]),
            Pattern::new(
                "かも",
                &[&[kamo], &[ex!(surface: ["しれ"], pos: ["動詞"], basic_form: ["しれる"], conjugated_form: ["連用形", "未然形"])]],
            ),
            Pattern::new("思う", &[&[ex!(surface: ["思う"], pos: ["動詞"], conjugated_form: ["基本形"], basic_form: ["思う"])]]),
            Pattern::new(
                "思います",
                &[
                    &[ex!(surface: ["思い"], pos: ["動詞"], basic_form: ["思う"], conjugated_form: ["連用形"])],
                    &[ex!(surface: ["ます"], pos: ["助動詞"], conjugated_type: ["特殊・マス"], conjugated_form: ["基本形"])],
                ],
            ),
            Pattern::new(
                "可能性を示唆している",
                &[
                    &[ex!(surface: ["可能"], pos: ["名詞"])],
                    &[ex!(surface: ["性"], pos: ["名詞"])],
                    &[ex!(surface: ["を"], pos: ["助詞"])],
                    &[ex!(surface: ["示唆"], pos: ["名詞"])],
                    &[ex!(surface: ["し"], pos: ["動詞"], basic_form: ["する"])],
                    &[ex!(surface: ["て"], pos: ["助詞"])],
                    &[ex!(surface: ["いる"], pos: ["動詞"], basic_form: ["いる"])],
                ],
            ),
        ]
    })
}

pub struct JaNoWeakPhrase;
impl Rule for JaNoWeakPhrase {
    fn id(&self) -> &'static str {
        "ja-no-weak-phrase"
    }
    fn scope(&self) -> Scope {
        Scope::EVERYWHERE
    }
    fn check(&mut self, blk: &BlockData, _ctx: &Ctx) -> Vec<Report> {
        scan(
            blk,
            weak_phrase_patterns(),
            |t| matches!(t.surface.as_str(), "かも" | "思う" | "思い" | "可能"),
            |p, first, last| {
                Report::at(blk.tokens[first].byte_start, blk.tokens[last].byte_end, format!("弱い表現: \"{}\" が使われています。", p.label))
            },
        )
    }
}

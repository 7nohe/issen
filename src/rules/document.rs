//! Document-level validation for agent output: the headings a document must
//! carry, phrases it must not, and terms it must spell one way. These rules
//! are off until configured -- unlike the prose rules they have no sensible
//! default.

use super::{Anchor, BlockData, Ctx, Report, Rule};
use crate::config::opt_strs;
use crate::document::BlockKind;
use regex::Regex;

/// required-headings: every configured heading text must appear as a heading.
///
/// ```yaml
/// required-headings: { headings: [概要, 前提条件] }
/// ```
#[derive(Default)]
pub struct RequiredHeadings {
    seen: Vec<String>,
}

impl Rule for RequiredHeadings {
    fn id(&self) -> &'static str {
        "required-headings"
    }
    fn default_enabled(&self) -> bool {
        false
    }
    fn check(&mut self, blk: &BlockData, _ctx: &Ctx) -> Vec<Report> {
        if blk.block.kind == BlockKind::Heading {
            self.seen.push(blk.text().trim().to_string());
        }
        Vec::new()
    }
    fn finish(&mut self, ctx: &Ctx) -> Vec<(Anchor, Report)> {
        opt_strs(&ctx.options, "headings")
            .into_iter()
            .filter(|h| !self.seen.iter().any(|s| s == h))
            .map(|h| (Anchor::Source, Report::at(0, 0, format!("必須の見出し \"{h}\" がありません。"))))
            .collect()
    }
}

/// A configured pattern: a literal, or `/regex/`. Compiled once per lint.
enum Needle {
    Literal(String),
    Regex(Regex),
}

impl Needle {
    fn parse(s: &str) -> Option<Needle> {
        if s.len() >= 2 && s.starts_with('/') && s.ends_with('/') {
            Regex::new(&s[1..s.len() - 1]).ok().map(Needle::Regex)
        } else if s.is_empty() {
            None
        } else {
            Some(Needle::Literal(s.to_string()))
        }
    }

    fn find_all<'a>(&self, text: &'a str) -> Vec<(usize, usize, &'a str)> {
        match self {
            Needle::Literal(lit) => text.match_indices(lit.as_str()).map(|(i, m)| (i, i + m.len(), m)).collect(),
            Needle::Regex(re) => re.find_iter(text).map(|m| (m.start(), m.end(), m.as_str())).collect(),
        }
    }
}

/// forbidden: phrases that must not appear.
///
/// ```yaml
/// forbidden: { patterns: ["絶対に成功します", "/必ず.*します/"] }
/// ```
#[derive(Default)]
pub struct Forbidden {
    needles: Option<Vec<Needle>>,
}

impl Rule for Forbidden {
    fn id(&self) -> &'static str {
        "forbidden"
    }
    fn default_enabled(&self) -> bool {
        false
    }
    fn check(&mut self, blk: &BlockData, ctx: &Ctx) -> Vec<Report> {
        let needles = self
            .needles
            .get_or_insert_with(|| opt_strs(&ctx.options, "patterns").iter().filter_map(|p| Needle::parse(p)).collect());
        let mut out: Vec<Report> = needles
            .iter()
            .flat_map(|n| n.find_all(blk.text()))
            .map(|(s, e, m)| Report::at(s, e, format!("禁止されている表現 \"{m}\" が使われています。")))
            .collect();
        out.sort_by_key(|r| r.start);
        out
    }
}

/// terminology: spell a term one way; every variant is fixable.
///
/// ```yaml
/// terminology:
///   terms:
///     - { preferred: GitHub, avoid: [Github, github] }
/// ```
#[derive(Default)]
pub struct Terminology {
    terms: Option<Vec<(String, Vec<Needle>)>>,
}

impl Rule for Terminology {
    fn id(&self) -> &'static str {
        "terminology"
    }
    fn default_enabled(&self) -> bool {
        false
    }
    fn check(&mut self, blk: &BlockData, ctx: &Ctx) -> Vec<Report> {
        let terms = self.terms.get_or_insert_with(|| {
            ctx.options
                .get("terms")
                .and_then(|t| t.as_sequence())
                .map(|terms| {
                    terms
                        .iter()
                        .filter_map(|term| {
                            let preferred = term.get("preferred")?.as_str()?.to_string();
                            let avoid = opt_strs(term, "avoid").iter().filter_map(|a| Needle::parse(a)).collect();
                            Some((preferred, avoid))
                        })
                        .collect()
                })
                .unwrap_or_default()
        });
        let mut out = Vec::new();
        for (preferred, needles) in terms.iter() {
            for (s, e, m) in needles.iter().flat_map(|n| n.find_all(blk.text())) {
                if m != preferred {
                    out.push(Report::at(s, e, format!("\"{m}\" ではなく \"{preferred}\" を使ってください。")).replace(preferred.clone()));
                }
            }
        }
        out.sort_by_key(|r| r.start);
        out
    }
}

//! Document-level validation for agent output: the headings a document must
//! carry, phrases it must not, and terms it must spell one way. These rules
//! are off until configured -- unlike the prose rules they have no sensible
//! default.

use super::{Anchor, BlockData, Ctx, Report, Rule};
use crate::config::{matchers, opt_strs, Matcher};
use crate::document::BlockKind;

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

/// forbidden: phrases that must not appear.
///
/// ```yaml
/// forbidden: { patterns: ["絶対に成功します", "/必ず.*します/"] }
/// ```
#[derive(Default)]
pub struct Forbidden {
    needles: Option<Vec<Matcher>>,
}

impl Rule for Forbidden {
    fn id(&self) -> &'static str {
        "forbidden"
    }
    fn default_enabled(&self) -> bool {
        false
    }
    fn validate(&self, options: &yaml_serde::Value) -> Result<(), String> {
        matchers(options, "patterns").map(|_| ())
    }
    fn check(&mut self, blk: &BlockData, ctx: &Ctx) -> Vec<Report> {
        let needles = self.needles.get_or_insert_with(|| matchers(&ctx.options, "patterns").unwrap_or_default());
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
    terms: Option<Vec<(String, Vec<Matcher>)>>,
}

impl Rule for Terminology {
    fn id(&self) -> &'static str {
        "terminology"
    }
    fn default_enabled(&self) -> bool {
        false
    }
    fn validate(&self, options: &yaml_serde::Value) -> Result<(), String> {
        for term in options.get("terms").and_then(|t| t.as_sequence()).map(|s| s.as_slice()).unwrap_or_default() {
            if term.get("preferred").and_then(|p| p.as_str()).is_none() {
                return Err("terminology.terms: every entry needs a `preferred` string".to_string());
            }
            matchers(term, "avoid")?;
        }
        Ok(())
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
                            Some((preferred, matchers(term, "avoid").unwrap_or_default()))
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

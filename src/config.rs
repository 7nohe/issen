//! `issen.yml` configuration.
//!
//! ```yaml
//! rules:
//!   sentence-length: { max: 100 }
//!   no-exclamation-question-mark: false   # disable
//!   ja-no-weak-phrase: { severity: warning }
//! gate:
//!   fail-on: error
//! ```
//!
//! A rule's options are merged key by key over the ja-technical-writing preset,
//! so `{ severity: warning }` keeps the preset's thresholds.

use regex::Regex;
use serde::Deserialize;
use serde_yaml::Value;
use std::collections::BTreeMap;

const PRESET: &str = r#"
rules:
  sentence-length: { max: 100 }
  max-comma: { max: 3 }
  max-ten: { max: 3 }
  max-kanji-continuous-len: { max: 6 }
  no-mix-dearu-desumasu: { preferInHeader: "", preferInBody: "ですます", preferInList: "である", strict: false }
  ja-no-mixed-period: { periodMark: "。" }
  no-double-negative-ja: true
  no-dropping-the-ra: true
  no-doubled-conjunctive-particle-ga: true
  no-doubled-conjunction: true
  no-doubled-joshi: { min_interval: 1 }
  no-nfd: true
  no-invalid-control-character: true
  no-zero-width-spaces: true
  no-exclamation-question-mark: true
  no-hankaku-kana: true
  ja-no-weak-phrase: true
"#;

/// When the exit code should be non-zero.
///
/// ```yaml
/// gate:
///   fail-on: error      # error (default) | warning | never
///   max-warnings: 3     # also fail when warnings exceed this
/// ```
#[derive(Debug, Clone, Deserialize, Default)]
pub struct Gate {
    #[serde(default, rename = "fail-on")]
    pub fail_on: Option<String>,
    #[serde(default, rename = "max-warnings")]
    pub max_warnings: Option<usize>,
}

const FAIL_ON: [&str; 5] = ["error", "warning", "warn", "never", "none"];

impl Gate {
    pub fn fails(&self, errors: usize, warnings: usize) -> bool {
        let by_level = match self.fail_on.as_deref() {
            Some("never") | Some("none") => false,
            Some("warning") | Some("warn") => errors + warnings > 0,
            _ => errors > 0,
        };
        by_level || self.max_warnings.map(|m| warnings > m).unwrap_or(false)
    }

    fn validate(&self) -> Result<(), String> {
        match &self.fail_on {
            Some(v) if !FAIL_ON.contains(&v.as_str()) => {
                Err(format!("gate.fail-on: unknown value \"{v}\". Use one of: {}", FAIL_ON.join(", ")))
            }
            _ => Ok(()),
        }
    }
}

/// A configured pattern: a literal, or textlint's `/pattern/flags` form.
#[derive(Debug)]
pub enum Matcher {
    Literal(String),
    Regex(Regex),
}

impl Matcher {
    /// `None` for an empty spec, which matches nothing.
    pub fn parse(spec: &str) -> Result<Option<Matcher>, String> {
        if spec.is_empty() {
            return Ok(None);
        }
        let Some((pattern, flags)) = spec.strip_prefix('/').and_then(|b| b.rsplit_once('/')) else {
            return Ok(Some(Matcher::Literal(spec.to_string())));
        };
        let mut inline = String::new();
        for f in flags.chars() {
            match f {
                // Rust's regex is Unicode-aware and every search is global,
                // so textlint's `u` and `g` are already how this behaves.
                'i' | 'm' | 's' => inline.push(f),
                'g' | 'u' => {}
                other => return Err(format!("unknown regex flag \"{other}\" in \"{spec}\"")),
            }
        }
        let source = if inline.is_empty() { pattern.to_string() } else { format!("(?{inline}){pattern}") };
        Regex::new(&source).map(|re| Some(Matcher::Regex(re))).map_err(|e| format!("bad regex \"{spec}\": {e}"))
    }

    pub fn find_all<'a>(&self, text: &'a str) -> Vec<(usize, usize, &'a str)> {
        match self {
            Matcher::Literal(lit) => text.match_indices(lit.as_str()).map(|(i, m)| (i, i + m.len(), m)).collect(),
            Matcher::Regex(re) => re.find_iter(text).map(|m| (m.start(), m.end(), m.as_str())).collect(),
        }
    }

    /// The text with every match removed, as textlint's `skipPatterns` does.
    pub fn remove_from(&self, text: &str) -> String {
        match self {
            Matcher::Literal(lit) => text.replace(lit.as_str(), ""),
            Matcher::Regex(re) => re.replace_all(text, "").into_owned(),
        }
    }
}

/// Compile every entry of a string-list option into matchers.
pub fn matchers(v: &Value, key: &str) -> Result<Vec<Matcher>, String> {
    opt_strs(v, key).iter().filter_map(|p| Matcher::parse(p).transpose()).collect()
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub rules: BTreeMap<String, Value>,
    #[serde(default)]
    pub gate: Gate,
}

impl Config {
    pub fn preset() -> Config {
        serde_yaml::from_str(PRESET).expect("preset config is valid")
    }

    pub fn load(path: &str) -> Result<Config, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
        Config::parse(&text).map_err(|e| format!("{path}: {e}"))
    }

    /// Parse user YAML and layer it over the preset.
    pub fn parse(yaml: &str) -> Result<Config, String> {
        let user: Config = serde_yaml::from_str(yaml).map_err(|e| e.to_string())?;
        let mut base = Config::preset();
        for (k, v) in user.rules {
            let merged = match (base.rules.remove(&k), v) {
                // `true` re-enables a rule without touching its options.
                (Some(preset), Value::Bool(true)) => preset,
                (Some(Value::Mapping(mut p)), Value::Mapping(u)) => {
                    p.extend(u);
                    Value::Mapping(p)
                }
                (_, v) => v,
            };
            base.rules.insert(k, merged);
        }
        base.gate = user.gate;
        base.validate()?;
        Ok(base)
    }

    /// Reject a config that would otherwise fail quietly: a misspelled rule
    /// name that disables nothing, or a gate value that lets errors through.
    fn validate(&self) -> Result<(), String> {
        self.gate.validate()?;
        let known = crate::rules::all_ids();
        for name in self.rules.keys() {
            if !known.contains(&name.as_str()) {
                let hint = known
                    .iter()
                    .find(|k| k.contains(name.as_str()) || name.contains(*k))
                    .map(|k| format!(" Did you mean \"{k}\"?"))
                    .unwrap_or_default();
                return Err(format!("rules.{name}: unknown rule.{hint} Run `issen --list-rules` to see them all."));
            }
        }
        Ok(())
    }

    pub fn options(&self, id: &str) -> Value {
        match self.rules.get(id) {
            Some(Value::Mapping(m)) => Value::Mapping(m.clone()),
            _ => Value::Null,
        }
    }
}

pub fn opt_usize(v: &Value, key: &str, default: usize) -> usize {
    v.get(key).and_then(|x| x.as_u64()).map(|x| x as usize).unwrap_or(default)
}

pub fn opt_bool(v: &Value, key: &str, default: bool) -> bool {
    v.get(key).and_then(|x| x.as_bool()).unwrap_or(default)
}

pub fn opt_str(v: &Value, key: &str, default: &str) -> String {
    v.get(key).and_then(|x| x.as_str()).unwrap_or(default).to_string()
}

/// A string list option; `default` when absent or empty.
pub fn opt_strs_or(v: &Value, key: &str, default: &[&str]) -> Vec<String> {
    let given: Vec<String> = v
        .get(key)
        .and_then(|x| x.as_sequence())
        .map(|s| s.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();
    if given.is_empty() {
        default.iter().map(|s| s.to_string()).collect()
    } else {
        given
    }
}

pub fn opt_strs(v: &Value, key: &str) -> Vec<String> {
    opt_strs_or(v, key, &[])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_options_merge_over_preset() {
        let c = Config::parse("rules:\n  max-comma: { severity: warning }\n  no-mix-dearu-desumasu: { strict: true }\n").unwrap();
        assert_eq!(opt_usize(&c.options("max-comma"), "max", 99), 3);
        assert_eq!(opt_str(&c.options("max-comma"), "severity", ""), "warning");
        assert_eq!(opt_str(&c.options("no-mix-dearu-desumasu"), "preferInBody", ""), "ですます");
        assert!(opt_bool(&c.options("no-mix-dearu-desumasu"), "strict", false));
        let c = Config::parse("rules:\n  max-comma: true\n").unwrap();
        assert_eq!(opt_usize(&c.options("max-comma"), "max", 99), 3);
    }
}

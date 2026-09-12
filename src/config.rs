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

use crate::diagnostic::Severity;
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

impl Gate {
    pub fn fails(&self, errors: usize, warnings: usize) -> bool {
        let by_level = match self.fail_on.as_deref().map(Severity::parse) {
            Some(Some(Severity::Warning)) => errors + warnings > 0,
            Some(None) => false, // never / none / anything unknown
            _ => errors > 0,
        };
        by_level || self.max_warnings.map(|m| warnings > m).unwrap_or(false)
    }
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
        Ok(base)
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

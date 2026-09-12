//! textlint compatibility: every diagnostic textlint reports on a fixture must
//! be reported by issen with the same rule ID, line, and column -- except for
//! the individually listed, documented divergences.

use issen::Linter;
use serde_json::Value;
use std::collections::HashMap;

/// (rule, line, textlint column) -> issen column, for the known differences
/// described in the README.
const COLUMN_DIVERGENCES: &[(&str, usize, usize, usize)] = &[
    // 来れる/見れる: textlint reports word_position (1-based) as a 0-based index.
    ("no-dropping-the-ra", 7, 9, 8),
    // sentence-length: textlint reports a paragraph-relative padding; issen the sentence start.
    ("sentence-length", 15, 151, 1),
];

fn textlint_expectations(path: &str) -> Vec<(String, usize, usize)> {
    let json: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    json[0]["messages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| {
            let rule = m["ruleId"].as_str().unwrap().trim_start_matches(issen::TEXTLINT_RULE_PREFIX).to_string();
            (rule, m["line"].as_u64().unwrap() as usize, m["column"].as_u64().unwrap() as usize)
        })
        .collect()
}

fn check_fixture(name: &str) {
    // Resolved at run time: a compile-time env!() bakes in the path the test
    // binary was built at, which goes stale when the checkout moves.
    let dir = format!("{}/tests/fixtures/", std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let source = std::fs::read_to_string(format!("{dir}{name}.md")).unwrap();
    let mut expected = textlint_expectations(&format!("{dir}{name}.textlint.json"));
    for (rule, line, tl_col, issen_col) in COLUMN_DIVERGENCES {
        for e in expected.iter_mut() {
            if e.0 == *rule && e.1 == *line && e.2 == *tl_col {
                e.2 = *issen_col;
            }
        }
    }

    let linter = Linter::preset().unwrap();
    let result = linter.lint(name, &source);

    // Exact multiset comparison on (rule, line, column).
    let mut got: HashMap<(String, usize, usize), usize> = HashMap::new();
    for d in &result.diagnostics {
        *got.entry((d.rule.clone(), d.line, d.column)).or_default() += 1;
    }
    let mut want: HashMap<(String, usize, usize), usize> = HashMap::new();
    for e in &expected {
        *want.entry(e.clone()).or_default() += 1;
    }
    let mut failures = Vec::new();
    for (k, n) in &want {
        let g = got.get(k).copied().unwrap_or(0);
        if g < *n {
            failures.push(format!("missing {} at {}:{} (want {n}, got {g})", k.0, k.1, k.2));
        }
    }
    for (k, n) in &got {
        let w = want.get(k).copied().unwrap_or(0);
        if *n > w {
            failures.push(format!("extra {} at {}:{} (want {w}, got {n})", k.0, k.1, k.2));
        }
    }
    assert!(failures.is_empty(), "{name}: {} mismatch(es)\n{}", failures.len(), failures.join("\n"));
}

#[test]
fn preset_sample2_matches_textlint() {
    check_fixture("sample2");
}

#[test]
fn preset_sample3_matches_textlint() {
    check_fixture("sample3");
}

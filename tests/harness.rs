//! Agent-harness behaviour: document validators, fixes, and the gate.

use issen::{Config, Linter};

fn config(yaml: &str) -> Config {
    Config::parse(yaml).unwrap()
}

#[test]
fn validators_are_inactive_until_configured() {
    let linter = Linter::preset().unwrap();
    let r = linter.lint("t.md", "# 概要\n\nGithubに置く。\n");
    assert!(r.diagnostics.iter().all(|d| d.rule != "terminology" && d.rule != "required-headings"));
}

#[test]
fn required_headings_forbidden_and_terminology() {
    let linter = Linter::new(config(
        r#"
rules:
  required-headings: { headings: [概要, 前提条件] }
  forbidden: { patterns: ["絶対に成功します", "/必ず.*します/"] }
  terminology:
    terms:
      - { preferred: GitHub, avoid: [Github, github] }
"#,
    ))
    .unwrap();
    let r = linter.lint("t.md", "# 概要\n\nGithubに置くと必ず動作します。絶対に成功します。\n");
    let rules: Vec<&str> = r.diagnostics.iter().map(|d| d.rule.as_str()).collect();
    assert_eq!(rules.iter().filter(|r| **r == "required-headings").count(), 1);
    assert_eq!(rules.iter().filter(|r| **r == "forbidden").count(), 2);
    assert_eq!(rules.iter().filter(|r| **r == "terminology").count(), 1);

    let missing = r.diagnostics.iter().find(|d| d.rule == "required-headings").unwrap();
    assert_eq!((missing.line, missing.column), (1, 1));
    assert!(missing.message.contains("前提条件"));

    let term = r.diagnostics.iter().find(|d| d.rule == "terminology").unwrap();
    assert_eq!(term.fix.as_ref().unwrap().text, "GitHub");

    let fixed = linter.fix("t.md", "Githubとgithub。\n");
    assert_eq!(fixed.source, "GitHubとGitHub。\n");
    assert_eq!(fixed.applied, 2);
}

#[test]
fn gate_levels() {
    let g = config("gate: { fail-on: never }\n").gate;
    assert!(!g.fails(3, 3));
    let g = config("gate: { fail-on: warning }\n").gate;
    assert!(g.fails(0, 1));
    let g = config("gate: { fail-on: error, max-warnings: 2 }\n").gate;
    assert!(!g.fails(0, 2));
    assert!(g.fails(0, 3));
    assert!(g.fails(1, 0));
    assert!(!Config::preset().gate.fails(0, 10));
}

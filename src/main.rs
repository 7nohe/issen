use clap::{Parser, ValueEnum};
use issen::format::{self, Summary};
use issen::{Config, Linter};
use std::io::Read;

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Format {
    /// `path:line:col: message [rule]`, one per line.
    Text,
    /// issen's own JSON: files, diagnostics with character ranges and fixes.
    Json,
    /// textlint-compatible JSON array, for diffing against textlint.
    Textlint,
}

impl From<Format> for format::Format {
    fn from(f: Format) -> Self {
        match f {
            Format::Text => format::Format::Text,
            Format::Json => format::Format::Json,
            Format::Textlint => format::Format::Textlint,
        }
    }
}

/// Japanese document harness for AI agents.
#[derive(Parser, Debug)]
#[command(name = "issen", version, about)]
struct Cli {
    /// Markdown files to lint.
    files: Vec<String>,
    /// Read one document from standard input.
    #[arg(long)]
    stdin: bool,
    /// File name to report for standard input.
    #[arg(long, default_value = "<stdin>")]
    stdin_filename: String,
    /// Output format.
    #[arg(long, value_enum, default_value_t = Format::Text)]
    format: Format,
    /// Config file (YAML). Defaults to ./issen.yml when present.
    #[arg(long)]
    config: Option<String>,
    /// Exit with code 1 when warnings exceed this number (overrides gate.max-warnings).
    #[arg(long)]
    max_warnings: Option<usize>,
    /// List enabled rule IDs and exit.
    #[arg(long)]
    list_rules: bool,
    /// Apply every available fix, re-lint, and report what remains. Files are
    /// rewritten in place; with --stdin the fixed document goes to stdout and
    /// diagnostics to stderr.
    #[arg(long)]
    fix: bool,
}

fn die(msg: impl std::fmt::Display) -> ! {
    eprintln!("issen: {msg}");
    std::process::exit(2)
}

fn main() {
    let cli = Cli::parse();

    let config = match &cli.config {
        Some(p) => Config::load(p),
        None if std::path::Path::new("issen.yml").exists() => Config::load("issen.yml"),
        None => Ok(Config::preset()),
    }
    .unwrap_or_else(|e| die(e));

    if cli.list_rules {
        for rule in issen::rules::build(&config) {
            println!("{}", rule.id());
        }
        return;
    }
    if cli.files.is_empty() && !cli.stdin {
        die("no input. Pass Markdown files or --stdin.");
    }

    let mut gate = config.gate.clone();
    if cli.max_warnings.is_some() {
        gate.max_warnings = cli.max_warnings;
    }
    let linter = Linter::new(config).unwrap_or_else(|e| die(e));

    let mut results = Vec::new();
    let mut fixed = 0usize;
    // With --fix, the fixed stdin document owns stdout and reports go to stderr.
    let mut fixed_stdin: Option<String> = None;

    if cli.stdin {
        let mut src = String::new();
        std::io::stdin().read_to_string(&mut src).unwrap_or_else(|e| die(format!("stdin: {e}")));
        if cli.fix {
            let r = linter.fix(&cli.stdin_filename, &src);
            fixed += r.applied;
            fixed_stdin = Some(r.source);
            results.push(r.result);
        } else {
            results.push(linter.lint(&cli.stdin_filename, &src));
        }
    }
    for path in &cli.files {
        let src = std::fs::read_to_string(path).unwrap_or_else(|e| die(format!("{path}: {e}")));
        if cli.fix {
            let r = linter.fix(path, &src);
            if r.source != src {
                std::fs::write(path, &r.source).unwrap_or_else(|e| die(format!("{path}: {e}")));
            }
            fixed += r.applied;
            results.push(r.result);
        } else {
            results.push(linter.lint(path, &src));
        }
    }

    let summary = Summary::of(&results, fixed);
    let report = format::render(cli.format.into(), &results, summary);
    match &fixed_stdin {
        Some(text) => {
            print!("{text}");
            eprint!("{report}");
        }
        None => print!("{report}"),
    }
    if matches!(cli.format, Format::Text) {
        if cli.fix {
            eprintln!("{fixed} fix(es) applied");
        }
        if summary.errors + summary.warnings > 0 {
            eprintln!("{} error(s), {} warning(s)", summary.errors, summary.warnings);
        }
    }

    if gate.fails(summary.errors, summary.warnings) {
        std::process::exit(1);
    }
}

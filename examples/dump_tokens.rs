//! Debug aid: dump tokens line by line as JSON, for comparing issen's
//! morphology against kuromoji.js. Run with `cargo run --example dump-tokens -- <file>`.
//! Each entry is
//! `surface|pos|pd1|pd2|pd3|conj_type|conj_form|base|char_start`.

use issen::tokenizer::Tokenizer;

fn main() {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: cargo run --example dump-tokens -- <file>");
        std::process::exit(2);
    };
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("dump-tokens: {path}: {e}");
            std::process::exit(2);
        }
    };
    let tokenizer = Tokenizer::new().expect("tokenizer");
    let mut out: Vec<Vec<String>> = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() || line.starts_with('#') || line.starts_with('-') {
            continue;
        }
        let row = tokenizer
            .tokenize(line)
            .into_iter()
            .map(|t| {
                format!(
                    "{}|{}|{}|{}|{}|{}|{}|{}|{}",
                    t.surface,
                    t.pos(),
                    t.pos_detail_1(),
                    t.pos_detail_2(),
                    t.pos_detail_3(),
                    t.conjugated_type(),
                    t.conjugated_form(),
                    t.basic_form(),
                    line[..t.byte_start].chars().count()
                )
            })
            .collect();
        out.push(row);
    }
    println!("{}", serde_json::to_string(&out).unwrap());
}

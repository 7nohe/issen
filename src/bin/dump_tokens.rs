//! Dump tokens line by line as JSON, in the shape `tests/compare-textlint.js`
//! and the kuromoji.js parity check expect:
//! `surface|pos|pd1|pd2|pd3|conj_type|conj_form|base|char_start`.

use issen::tokenizer::Tokenizer;

fn main() {
    let path = std::env::args().nth(1).expect("usage: dump-tokens <file>");
    let text = std::fs::read_to_string(&path).expect("read file");
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

//! Morphological analysis behind a backend-neutral interface.
//!
//! Rules only ever see [`Token`], whose feature columns follow the IPADIC
//! (MeCab) layout: pos, pos_detail_1..3, conjugated_type, conjugated_form,
//! basic_form, reading, pronunciation. That layout is what kuromoji.js
//! (textlint-ja) uses too, so any analyzer that ships IPADIC -- Lindera,
//! Vibrato, Kagome, MeCab itself -- can sit behind [`Morphology`] without
//! touching a rule. The default backend is selected by Cargo feature.

const COLUMNS: usize = 9;

#[derive(Debug, Clone)]
pub struct Token {
    pub surface: String,
    /// Byte offsets within the analyzed text.
    pub byte_start: usize,
    pub byte_end: usize,
    /// The feature columns joined with ',' and the byte offset where each
    /// starts (plus the end), so a column is one slice rather than one String.
    features: String,
    cols: [u32; COLUMNS + 1],
}

impl Token {
    pub fn new(surface: impl Into<String>, features: &[&str], byte_start: usize, byte_end: usize) -> Token {
        let mut joined = String::new();
        let mut cols = [0u32; COLUMNS + 1];
        for (i, col) in cols.iter_mut().take(COLUMNS).enumerate() {
            *col = joined.len() as u32;
            joined.push_str(features.get(i).copied().unwrap_or("*"));
            joined.push(',');
        }
        cols[COLUMNS] = joined.len() as u32;
        Token { surface: surface.into(), byte_start, byte_end, features: joined, cols }
    }

    fn feat(&self, i: usize) -> &str {
        // Each column is followed by the ',' separator.
        &self.features[self.cols[i] as usize..self.cols[i + 1] as usize - 1]
    }
    pub fn pos(&self) -> &str {
        self.feat(0)
    }
    pub fn pos_detail_1(&self) -> &str {
        self.feat(1)
    }
    pub fn pos_detail_2(&self) -> &str {
        self.feat(2)
    }
    pub fn pos_detail_3(&self) -> &str {
        self.feat(3)
    }
    pub fn conjugated_type(&self) -> &str {
        self.feat(4)
    }
    pub fn conjugated_form(&self) -> &str {
        self.feat(5)
    }
    pub fn basic_form(&self) -> &str {
        self.feat(6)
    }
    pub fn reading(&self) -> &str {
        self.feat(7)
    }
    pub fn is_joshi(&self) -> bool {
        is_joshi_pos(self.pos())
    }
    pub fn is_whitespace(&self) -> bool {
        self.surface.trim().is_empty()
    }
}

/// Whether an IPADIC pos column is a particle (also true for merged 連語
/// whose pos reads "助詞助詞").
pub fn is_joshi_pos(pos: &str) -> bool {
    pos.starts_with("助詞")
}

/// A morphological analyzer producing IPADIC-style tokens, in order, with
/// byte spans inside the given text.
pub trait Morphology: Send + Sync {
    fn segment(&self, text: &str) -> Vec<Token>;
}

pub struct Tokenizer {
    backend: Box<dyn Morphology>,
}

impl Tokenizer {
    /// The default backend for this build (selected by Cargo feature).
    pub fn new() -> Result<Self, String> {
        Ok(Self::with_backend(Box::new(default_backend()?)))
    }

    pub fn with_backend(backend: Box<dyn Morphology>) -> Self {
        Tokenizer { backend }
    }

    /// Tokens of `text`, dropping anything a backend reports out of order or
    /// outside the text so rules can rely on monotonic spans.
    pub fn tokenize(&self, text: &str) -> Vec<Token> {
        let mut end = 0usize;
        self.backend
            .segment(text)
            .into_iter()
            .filter(|t| {
                let ok = t.byte_start >= end && t.byte_end >= t.byte_start && t.byte_end <= text.len();
                if ok {
                    end = t.byte_end;
                }
                ok
            })
            .collect()
    }
}

#[cfg(feature = "backend-lindera")]
pub use lindera_backend::LinderaBackend;

#[cfg(feature = "backend-lindera")]
fn default_backend() -> Result<LinderaBackend, String> {
    LinderaBackend::new()
}

#[cfg(not(any(feature = "backend-lindera")))]
compile_error!("issen needs a morphology backend: enable the `backend-lindera` feature");

#[cfg(feature = "backend-lindera")]
mod lindera_backend {
    use super::{Morphology, Token};
    use lindera::dictionary::load_dictionary;
    use lindera::mode::Mode;
    use lindera::segmenter::Segmenter;
    use std::borrow::Cow;

    /// Lindera with the embedded IPADIC dictionary.
    pub struct LinderaBackend {
        segmenter: Segmenter,
    }

    impl LinderaBackend {
        pub fn new() -> Result<Self, String> {
            let dictionary = load_dictionary("embedded://ipadic").map_err(|e| e.to_string())?;
            Ok(Self { segmenter: Segmenter::new(Mode::Normal, dictionary, None) })
        }
    }

    impl Morphology for LinderaBackend {
        fn segment(&self, text: &str) -> Vec<Token> {
            let mut raw = match self.segmenter.segment(Cow::Borrowed(text)) {
                Ok(t) => t,
                Err(_) => return Vec::new(),
            };
            raw.iter_mut()
                .map(|t| {
                    let (surface, start, end) = (t.surface.to_string(), t.byte_start, t.byte_end);
                    Token::new(surface, &t.details(), start, end)
                })
                .collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A backend that splits on ASCII spaces, to prove rules never touch Lindera.
    struct SpaceBackend;
    impl Morphology for SpaceBackend {
        fn segment(&self, text: &str) -> Vec<Token> {
            text.split(' ')
                .scan(0usize, |pos, w| {
                    let start = *pos;
                    *pos += w.len() + 1;
                    Some(Token::new(w, &["名詞"], start, start + w.len()))
                })
                .filter(|t| !t.surface.is_empty())
                .collect()
        }
    }

    #[test]
    fn any_backend_yields_ipadic_columns() {
        let tk = Tokenizer::with_backend(Box::new(SpaceBackend));
        let toks = tk.tokenize("日本語 テキスト");
        assert_eq!(toks.len(), 2);
        assert_eq!((toks[1].byte_start, toks[1].byte_end), (10, 22));
        assert_eq!(toks[1].pos(), "名詞");
        assert_eq!(toks[1].basic_form(), "*");
    }

    #[cfg(feature = "backend-lindera")]
    #[test]
    fn lindera_backend_matches_ipadic_layout() {
        let tk = Tokenizer::new().unwrap();
        let toks = tk.tokenize("食べれる");
        assert_eq!(toks[0].surface, "食べ");
        assert_eq!(toks[0].conjugated_type(), "一段");
        assert_eq!(toks[0].conjugated_form(), "未然形");
        assert_eq!(toks[1].basic_form(), "れる");
        assert_eq!(toks[1].reading(), "レル");
    }
}

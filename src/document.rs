//! Markdown -> linting blocks.
//!
//! A block is one unit of prose (paragraph, heading, list item, table cell)
//! with inline markup stripped. Every byte of block text maps back to a byte
//! in the source through `segments`, so diagnostics point at the original
//! Markdown, not at the stripped text. Segments also remember what inline
//! construct they came from, so rules can honour textlint's node exclusions
//! (a kanji run inside a link, a paragraph that ends in inline code).

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    Paragraph,
    Heading,
    ListItem,
    TableCell,
}

impl BlockKind {
    pub const ALL: &'static [BlockKind] = &[BlockKind::Paragraph, BlockKind::Heading, BlockKind::ListItem, BlockKind::TableCell];
}

/// What a run of block text came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentKind {
    Text,
    /// Inline code, masked in the block text.
    Code,
    /// A soft or hard line break, "\n" in the block text.
    Break,
}

/// A run of block text that came from one run of source text.
#[derive(Debug, Clone)]
pub struct Segment {
    pub text_start: usize,
    pub text_end: usize,
    pub src_start: usize,
    pub src_end: usize,
    pub kind: SegmentKind,
    /// Inside `[...](url)`; `url_link` when the link text is its own URL.
    pub in_link: bool,
    pub url_link: bool,
    pub in_emphasis: bool,
}

impl Segment {
    /// Text and source have the same byte length, so offsets inside map 1:1.
    fn verbatim(&self) -> bool {
        self.text_end - self.text_start == self.src_end - self.src_start
    }
}

#[derive(Debug, Clone)]
pub struct Block {
    pub kind: BlockKind,
    pub text: String,
    pub segments: Vec<Segment>,
    pub in_blockquote: bool,
}

impl Block {
    fn new(kind: BlockKind, in_blockquote: bool) -> Block {
        Block { kind, text: String::new(), segments: Vec::new(), in_blockquote }
    }

    fn push(&mut self, s: &str, src: std::ops::Range<usize>, kind: SegmentKind, inline: &Inline) {
        let start = self.text.len();
        self.text.push_str(s);
        self.segments.push(Segment {
            text_start: start,
            text_end: self.text.len(),
            src_start: src.start,
            src_end: src.end,
            kind,
            in_link: inline.link > 0,
            url_link: inline.url_link,
            in_emphasis: inline.emphasis > 0,
        });
    }

    /// The segment a block-text byte offset falls in. An offset on the
    /// boundary between two segments belongs to the later one.
    pub fn segment_at(&self, text_byte: usize) -> Option<&Segment> {
        let i = self.segments.partition_point(|s| s.text_start <= text_byte);
        i.checked_sub(1).map(|i| &self.segments[i])
    }

    /// Map a byte offset in `text` to a byte offset in the source.
    ///
    /// Inside a verbatim segment the mapping is 1:1. Inside a segment whose
    /// source differs in length (masked code, an entity, a CRLF break) any
    /// interior offset resolves to the segment's source end, so a report can
    /// only ever cover the whole construct -- never land mid-character.
    pub fn to_source(&self, text_byte: usize) -> usize {
        let Some(seg) = self.segment_at(text_byte) else {
            return self.segments.first().map(|s| s.src_start).unwrap_or(0);
        };
        if text_byte >= seg.text_end {
            return seg.src_end;
        }
        let delta = text_byte - seg.text_start;
        if delta == 0 {
            seg.src_start
        } else if seg.verbatim() {
            seg.src_start + delta
        } else {
            seg.src_end
        }
    }

    /// The block text within `range`, keeping only segments `keep` accepts.
    pub fn text_where(&self, range: std::ops::Range<usize>, keep: impl Fn(&Segment) -> bool) -> String {
        self.segments
            .iter()
            .filter(|s| keep(s))
            .filter_map(|s| {
                let (lo, hi) = (s.text_start.max(range.start), s.text_end.min(range.end));
                (lo < hi).then(|| &self.text[lo..hi])
            })
            .collect()
    }

    /// The last segment holding non-whitespace text.
    pub fn last_content_segment(&self) -> Option<&Segment> {
        self.segments.iter().rev().find(|s| !self.text[s.text_start..s.text_end].trim().is_empty())
    }
}

#[derive(Default)]
struct Inline {
    link: usize,
    url_link: bool,
    emphasis: usize,
    image: usize,
}

#[derive(Debug)]
pub struct Document {
    pub source: String,
    pub blocks: Vec<Block>,
    line_starts: Vec<usize>,
    /// Character offset of every byte offset (len = source.len() + 1).
    byte_to_char: Vec<u32>,
}

impl Document {
    pub fn parse(source: &str) -> Document {
        let mut line_starts = vec![0];
        let mut byte_to_char = vec![0u32; source.len() + 1];
        let mut chars = 0u32;
        for (i, ch) in source.char_indices() {
            byte_to_char[i..i + ch.len_utf8()].fill(chars);
            chars += 1;
            if ch == '\n' {
                line_starts.push(i + 1);
            }
        }
        byte_to_char[source.len()] = chars;

        let mut opts = Options::empty();
        opts.insert(Options::ENABLE_TABLES);
        opts.insert(Options::ENABLE_STRIKETHROUGH);
        opts.insert(Options::ENABLE_TASKLISTS);
        opts.insert(Options::ENABLE_FOOTNOTES);
        opts.insert(Options::ENABLE_YAML_STYLE_METADATA_BLOCKS);

        let mut blocks = Vec::new();
        let mut open: Option<Block> = None;
        let mut bq_depth = 0usize;
        let mut item_depth = 0usize;
        let mut in_code_block = false;
        let mut in_metadata = false;
        let mut inline = Inline::default();
        let mut link_url: Option<String> = None;

        fn close(open: &mut Option<Block>, blocks: &mut Vec<Block>) {
            if let Some(b) = open.take() {
                if !b.text.trim().is_empty() {
                    blocks.push(b);
                }
            }
        }

        for (event, range) in Parser::new_ext(source, opts).into_offset_iter() {
            match event {
                Event::Start(tag) => match tag {
                    Tag::BlockQuote(_) => bq_depth += 1,
                    Tag::CodeBlock(_) => in_code_block = true,
                    Tag::MetadataBlock(_) => in_metadata = true,
                    Tag::Image { .. } => inline.image += 1,
                    Tag::Emphasis => inline.emphasis += 1,
                    Tag::Link { dest_url, .. } => {
                        inline.link += 1;
                        link_url = Some(dest_url.to_string());
                    }
                    Tag::List(_) => close(&mut open, &mut blocks),
                    Tag::Item => {
                        // A tight list carries item text directly; a nested item
                        // must not continue its parent's block.
                        close(&mut open, &mut blocks);
                        item_depth += 1;
                    }
                    Tag::Paragraph => {
                        close(&mut open, &mut blocks);
                        let kind = if item_depth > 0 { BlockKind::ListItem } else { BlockKind::Paragraph };
                        open = Some(Block::new(kind, bq_depth > 0));
                    }
                    Tag::Heading { .. } => {
                        close(&mut open, &mut blocks);
                        open = Some(Block::new(BlockKind::Heading, bq_depth > 0));
                    }
                    Tag::TableCell => {
                        close(&mut open, &mut blocks);
                        open = Some(Block::new(BlockKind::TableCell, bq_depth > 0));
                    }
                    _ => {}
                },
                Event::End(tag) => match tag {
                    TagEnd::BlockQuote(_) => bq_depth = bq_depth.saturating_sub(1),
                    TagEnd::CodeBlock => in_code_block = false,
                    TagEnd::MetadataBlock(_) => in_metadata = false,
                    TagEnd::Image => inline.image = inline.image.saturating_sub(1),
                    TagEnd::Emphasis => inline.emphasis = inline.emphasis.saturating_sub(1),
                    TagEnd::Link => {
                        inline.link = inline.link.saturating_sub(1);
                        inline.url_link = false;
                        link_url = None;
                    }
                    TagEnd::Item => {
                        close(&mut open, &mut blocks);
                        item_depth = item_depth.saturating_sub(1);
                    }
                    TagEnd::Paragraph | TagEnd::Heading(_) | TagEnd::TableCell => close(&mut open, &mut blocks),
                    _ => {}
                },
                Event::Text(_) | Event::Code(_) if in_code_block || in_metadata || inline.image > 0 => {}
                Event::Text(_) | Event::Code(_) => {
                    let (content, kind) = match &event {
                        Event::Text(s) => {
                            // textlint's skipUrlStringLink: a link whose text is its URL.
                            inline.url_link = link_url.as_deref() == Some(s.as_ref());
                            (s.to_string(), SegmentKind::Text)
                        }
                        // Inline code is masked with a same-length run of "ー" so it
                        // tokenizes as one opaque word and never contributes particles.
                        Event::Code(s) => ("ー".repeat(s.chars().count().max(1)), SegmentKind::Code),
                        _ => unreachable!(),
                    };
                    // A tight list carries item text directly, with no paragraph event.
                    if open.is_none() && item_depth > 0 {
                        open = Some(Block::new(BlockKind::ListItem, bq_depth > 0));
                    }
                    if let Some(b) = open.as_mut() {
                        b.push(&content, range, kind, &inline);
                    }
                }
                Event::SoftBreak | Event::HardBreak => {
                    if let Some(b) = open.as_mut() {
                        b.push("\n", range, SegmentKind::Break, &inline);
                    }
                }
                _ => {}
            }
        }
        close(&mut open, &mut blocks);

        Document { source: source.to_string(), blocks, line_starts, byte_to_char }
    }

    /// 1-based line and column (column counted in characters) of a source byte offset.
    pub fn position(&self, src_byte: usize) -> (usize, usize) {
        let src_byte = src_byte.min(self.source.len());
        let line = self.line_starts.partition_point(|&s| s <= src_byte).saturating_sub(1);
        let column = self.char_offset(src_byte) - self.char_offset(self.line_starts[line]) + 1;
        (line + 1, column)
    }

    /// Character offset from the start of the source.
    pub fn char_offset(&self, src_byte: usize) -> usize {
        self.byte_to_char[src_byte.min(self.source.len())] as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_paragraph_text_back_to_source() {
        let doc = Document::parse("# 見出し\n\nこれは`code`です。\n");
        assert_eq!(doc.blocks.len(), 2);
        let p = &doc.blocks[1];
        assert_eq!(p.text, "これはーーーーです。");
        let src = p.to_source(p.text.find("です").unwrap());
        assert_eq!(&doc.source[src..src + 6], "です");
        assert_eq!(doc.position(src), (3, 10));
    }

    #[test]
    fn masked_code_maps_to_whole_span_never_mid_char() {
        let doc = Document::parse("これは`日本`\n");
        let p = &doc.blocks[0];
        let code = p.segments.iter().find(|s| s.kind == SegmentKind::Code).unwrap();
        for b in code.text_start..=code.text_end {
            let src = p.to_source(b);
            assert!(doc.source.is_char_boundary(src));
            assert!(src == code.src_start || src == code.src_end);
        }
    }

    #[test]
    fn boundary_offset_belongs_to_the_next_segment() {
        let doc = Document::parse("これは  \nﾃｽﾄです。\n");
        let p = &doc.blocks[0];
        let at = p.text.find('ﾃ').unwrap();
        assert_eq!(doc.position(p.to_source(at)), (2, 1));
        let doc = Document::parse("これは\r\nﾃｽﾄです。\r\n");
        let p = &doc.blocks[0];
        assert_eq!(doc.position(p.to_source(p.text.find('ﾃ').unwrap())), (2, 1));
        let doc = Document::parse("&amp;Githubです。\n");
        let p = &doc.blocks[0];
        assert_eq!(doc.position(p.to_source(p.text.find('G').unwrap())), (1, 6));
    }

    #[test]
    fn tight_list_items_become_blocks() {
        let doc = Document::parse("- 一つ目である\n- 二つ目です\n");
        assert_eq!(doc.blocks.len(), 2);
        assert!(doc.blocks.iter().all(|b| b.kind == BlockKind::ListItem));
    }

    #[test]
    fn nested_tight_items_do_not_merge() {
        let doc = Document::parse("- 私は学生です\n  - 彼は先生です\n");
        let texts: Vec<&str> = doc.blocks.iter().map(|b| b.text.as_str()).collect();
        assert_eq!(texts, vec!["私は学生です", "彼は先生です"]);
        let doc = Document::parse("- `foo`は便利です。\n");
        assert_eq!(doc.blocks[0].text, "ーーーは便利です。");
    }

    #[test]
    fn table_cells_and_links_are_tagged() {
        let doc = Document::parse("| 名前 |\n|---|\n| はい |\n\n[https://x](https://x) と [名前](https://y)\n");
        assert!(doc.blocks.iter().take(2).all(|b| b.kind == BlockKind::TableCell));
        let p = doc.blocks.last().unwrap();
        let segs: Vec<(bool, bool)> = p.segments.iter().map(|s| (s.in_link, s.url_link)).collect();
        assert_eq!(segs, vec![(true, true), (false, false), (true, false)]);
    }
}

//! Markdown -> linting blocks.
//!
//! A block is one unit of prose (paragraph, heading, list item, table cell)
//! with inline markup stripped. Every byte of block text maps back to a byte
//! in the source through `segments`, so diagnostics point at the original
//! Markdown, not at the stripped text. Segments also remember what inline
//! construct they came from, so rules can honour textlint's node exclusions
//! (a kanji run inside a link, a paragraph that ends in inline code).

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

/// Length in UTF-16 code units, the unit textlint measures in because its
/// rules run on JavaScript strings.
pub fn utf16_len(s: &str) -> usize {
    s.chars().map(char::len_utf16).sum()
}

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
    /// Inside `*em*`. Kept apart from `in_strong` because textlint's rules
    /// exclude the two node types separately.
    pub in_emphasis: bool,
    /// Inside `**strong**`.
    pub in_strong: bool,
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
            in_strong: inline.strong > 0,
        });
    }

    /// The segment a block-text byte offset falls in. An offset on the
    /// boundary between two segments belongs to the later one.
    pub fn segment_at(&self, text_byte: usize) -> Option<&Segment> {
        let i = self.segments.partition_point(|s| s.text_start <= text_byte);
        i.checked_sub(1).map(|i| &self.segments[i])
    }

    /// Index of the segment an offset that *starts* a span falls in. On a
    /// boundary the span starts in the later segment.
    fn start_index(&self, text_byte: usize) -> Option<usize> {
        self.segments.partition_point(|s| s.text_start <= text_byte).checked_sub(1)
    }

    /// Index of the segment an offset that *ends* a span falls in. On a
    /// boundary the span ends in the earlier segment.
    fn end_index(&self, text_byte: usize) -> Option<usize> {
        self.segments.partition_point(|s| s.text_start < text_byte).checked_sub(1)
    }

    /// Map a byte offset in `text` that starts a span to a byte offset in the source.
    ///
    /// Inside a verbatim segment the mapping is 1:1. Inside a segment whose
    /// source differs in length (masked code, an entity, a CRLF break) any
    /// interior offset resolves to the segment's source end, so a report can
    /// only ever cover the whole construct -- never land mid-character.
    pub fn to_source(&self, text_byte: usize) -> usize {
        let Some(seg) = self.start_index(text_byte).map(|i| &self.segments[i]) else {
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

    /// Map a byte offset in `text` that ends a span to a byte offset in the
    /// source.
    ///
    /// A span ending exactly on a segment boundary ends at the previous
    /// segment's source end, not at the next segment's source start. Those two
    /// differ by whatever markup sits between them, so taking the latter would
    /// stretch the span over a closing `**` or `` ` ``.
    pub fn to_source_end(&self, text_byte: usize) -> usize {
        let Some(seg) = self.end_index(text_byte).map(|i| &self.segments[i]) else {
            return self.segments.first().map(|s| s.src_start).unwrap_or(0);
        };
        if text_byte >= seg.text_end || !seg.verbatim() {
            seg.src_end
        } else {
            seg.src_start + (text_byte - seg.text_start)
        }
    }

    /// The source range a fix may rewrite, or `None` when it must not.
    ///
    /// A fix is only safe while it stays inside one verbatim segment. A span
    /// reaching across two of them covers source the block text never held --
    /// the `*` around an emphasis, the backticks around code -- and rewriting
    /// it would delete that markup.
    pub fn fix_range(&self, start: usize, end: usize) -> Option<(usize, usize)> {
        let i = self.start_index(start)?;
        let j = self.end_index(end)?;
        let seg = &self.segments[i];
        if i != j || !seg.verbatim() || start < seg.text_start || end > seg.text_end {
            return None;
        }
        Some((seg.src_start + (start - seg.text_start), seg.src_start + (end - seg.text_start)))
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

    /// The block text of the markdown text node the last content segment belongs
    /// to. A soft break does not start a new text node, so issen's segments are
    /// finer than the nodes textlint rules see; this walks back over breaks and
    /// adjacent text in the same inline context to recover the node.
    pub fn last_text_node(&self) -> Option<&str> {
        let end = self.segments.iter().rposition(|s| !self.text[s.text_start..s.text_end].trim().is_empty())?;
        let last = &self.segments[end];
        let same = |s: &Segment| {
            s.kind != SegmentKind::Code
                && s.in_link == last.in_link
                && s.url_link == last.url_link
                && s.in_emphasis == last.in_emphasis
                && s.in_strong == last.in_strong
        };
        let mut start = end;
        while start > 0 && same(&self.segments[start - 1]) {
            start -= 1;
        }
        Some(&self.text[self.segments[start].text_start..last.text_end])
    }
}

#[derive(Default)]
struct Inline {
    link: usize,
    url_link: bool,
    emphasis: usize,
    strong: usize,
    image: usize,
}

#[derive(Debug)]
pub struct Document {
    pub source: String,
    pub blocks: Vec<Block>,
    line_starts: Vec<usize>,
    /// UTF-16 offset of every byte offset (len = source.len() + 1).
    byte_to_utf16: Vec<u32>,
}

impl Document {
    pub fn parse(source: &str) -> Document {
        let mut line_starts = vec![0];
        let mut byte_to_utf16 = vec![0u32; source.len() + 1];
        let mut units = 0u32;
        for (i, ch) in source.char_indices() {
            byte_to_utf16[i..i + ch.len_utf8()].fill(units);
            units += ch.len_utf16() as u32;
            if ch == '\n' {
                line_starts.push(i + 1);
            }
        }
        byte_to_utf16[source.len()] = units;

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
                    Tag::Strong => inline.strong += 1,
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
                    TagEnd::Strong => inline.strong = inline.strong.saturating_sub(1),
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
                        // Inline code is masked with a run of "ー" so it tokenizes as
                        // one opaque word and never contributes particles. The run is
                        // as long as the code itself in UTF-16, because textlint
                        // measures the code's own text when it counts a sentence.
                        Event::Code(s) => ("ー".repeat(utf16_len(s).max(1)), SegmentKind::Code),
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

        Document { source: source.to_string(), blocks, line_starts, byte_to_utf16 }
    }

    /// 1-based line and column of a source byte offset. The column counts
    /// UTF-16 code units, which is what textlint reports and what LSP uses by
    /// default; it equals the character count unless the line holds something
    /// outside the BMP.
    pub fn position(&self, src_byte: usize) -> (usize, usize) {
        let src_byte = src_byte.min(self.source.len());
        let line = self.line_starts.partition_point(|&s| s <= src_byte).saturating_sub(1);
        let column = self.utf16_offset(src_byte) - self.utf16_offset(self.line_starts[line]) + 1;
        (line + 1, column)
    }

    /// UTF-16 code unit offset from the start of the source.
    pub fn utf16_offset(&self, src_byte: usize) -> usize {
        self.byte_to_utf16[src_byte.min(self.source.len())] as usize
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

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use std::ops::Range;

pub(crate) struct MaskedText {
    text: String,
    hidden: Vec<bool>,
}

impl MaskedText {
    pub(crate) fn as_str(&self) -> &str {
        &self.text
    }

    fn into_text(self) -> String {
        self.text
    }

    pub(crate) fn visible_source_range(
        &self,
        source: &str,
        mut range: Range<usize>,
    ) -> Result<Option<Range<usize>>, SourceRangeError> {
        if self.hidden.len() != source.len() || range.start > range.end || range.end > source.len()
        {
            return Err(SourceRangeError::OutOfBounds);
        }

        while range.start < range.end && !source.is_char_boundary(range.start) {
            if !self.hidden[range.start] {
                return Err(SourceRangeError::NotCharBoundary);
            }
            range.start += 1;
        }
        while range.start < range.end && !source.is_char_boundary(range.end) {
            if !self.hidden[range.end - 1] {
                return Err(SourceRangeError::NotCharBoundary);
            }
            range.end -= 1;
        }

        while range.start < range.end {
            let character = source[range.start..]
                .chars()
                .next()
                .expect("non-empty source range starts on a character boundary");
            let character_end = range.start + character.len_utf8();
            let character_hidden = &self.hidden[range.start..character_end];
            if character_hidden.iter().all(|byte| *byte) {
                range.start = character_end;
            } else if character_hidden.iter().any(|byte| *byte) {
                return Err(SourceRangeError::NotCharBoundary);
            } else {
                break;
            }
        }

        while range.start < range.end {
            let (relative_start, _) = source[range.start..range.end]
                .char_indices()
                .next_back()
                .expect("non-empty source range ends on a character boundary");
            let character_start = range.start + relative_start;
            let character_hidden = &self.hidden[character_start..range.end];
            if character_hidden.iter().all(|byte| *byte) {
                range.end = character_start;
            } else if character_hidden.iter().any(|byte| *byte) {
                return Err(SourceRangeError::NotCharBoundary);
            } else {
                break;
            }
        }

        Ok((range.start < range.end).then_some(range))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceRangeError {
    OutOfBounds,
    NotCharBoundary,
}

pub(crate) fn mask_hidden_contexts(text: &str) -> String {
    masked_hidden_contexts(text).into_text()
}

pub(crate) fn masked_hidden_contexts(text: &str) -> MaskedText {
    let mut hidden = vec![false; text.len()];
    let mut code_block_start = None;
    let mut html_block_start = None;

    for (event, range) in Parser::new_ext(text, Options::empty()).into_offset_iter() {
        match event {
            Event::Start(Tag::CodeBlock(_)) => code_block_start = Some(range.start),
            Event::End(TagEnd::CodeBlock) => {
                if let Some(start) = code_block_start.take() {
                    mark(&mut hidden, start, range.end);
                }
            }
            Event::Start(Tag::HtmlBlock) => html_block_start = Some(range.start),
            Event::End(TagEnd::HtmlBlock) => {
                if let Some(start) = html_block_start.take() {
                    mark(&mut hidden, start, range.end);
                }
            }
            Event::Code(_) | Event::InlineMath(_) | Event::DisplayMath(_) => {
                mark(&mut hidden, range.start, range.end);
            }
            Event::Html(_) | Event::InlineHtml(_)
                if text[range.clone()].trim_start().starts_with("<!--") =>
            {
                mark(&mut hidden, range.start, range.end);
            }
            _ => {}
        }
    }
    if let Some(start) = code_block_start {
        mark(&mut hidden, start, text.len());
    }
    if let Some(start) = html_block_start {
        mark(&mut hidden, start, text.len());
    }
    mark_raw_html_elements(text, &mut hidden);
    mark_escapes(text.as_bytes(), &mut hidden);

    let mut masked = text.as_bytes().to_vec();
    for (index, byte) in masked.iter_mut().enumerate() {
        if hidden[index] && !matches!(*byte, b'\r' | b'\n') {
            *byte = b' ';
        }
    }
    MaskedText {
        text: String::from_utf8(masked).expect("masked byte positions remain valid UTF-8"),
        hidden,
    }
}

fn mark_raw_html_elements(text: &str, hidden: &mut [bool]) {
    let lower = text.to_ascii_lowercase();
    for tag in ["pre", "script", "style", "textarea"] {
        let opening = format!("<{tag}");
        let closing = format!("</{tag}>");
        let mut cursor = 0;
        while let Some(relative_start) = lower[cursor..].find(&opening) {
            let start = cursor + relative_start;
            let boundary = lower.as_bytes().get(start + opening.len()).copied();
            if !boundary.is_some_and(|byte| byte.is_ascii_whitespace() || byte == b'>') {
                cursor = start + opening.len();
                continue;
            }
            let end = lower[start..]
                .find(&closing)
                .map_or(text.len(), |offset| start + offset + closing.len());
            mark(hidden, start, end);
            cursor = end;
        }
    }
}

fn mark_escapes(bytes: &[u8], hidden: &mut [bool]) {
    let mut cursor = 0;
    while cursor + 1 < bytes.len() {
        if !hidden[cursor] && bytes[cursor] == b'\\' && bytes[cursor + 1].is_ascii_punctuation() {
            hidden[cursor + 1] = true;
            cursor += 2;
        } else {
            cursor += 1;
        }
    }
}

fn mark(hidden: &mut [bool], start: usize, end: usize) {
    hidden[start..end].fill(true);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masking_preserves_bytes_and_hides_semantic_dead_zones() {
        let source = "# Visible\n    ## Indented code\n```md\n## Fenced\n```\n<script>[Security](SECURITY.md)</script>\n";
        let masked = mask_hidden_contexts(source);
        assert_eq!(masked.len(), source.len());
        assert_eq!(masked.matches('\n').count(), source.matches('\n').count());
        assert!(masked.contains("# Visible"));
        assert!(!masked.contains("Indented code"));
        assert!(!masked.contains("Fenced"));
        assert!(!masked.contains("Security"));
    }
}

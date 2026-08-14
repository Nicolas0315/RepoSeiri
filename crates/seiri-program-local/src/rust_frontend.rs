use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RustFrontendLimits {
    pub max_source_bytes: usize,
    pub max_items: usize,
    pub max_unknowns: usize,
    pub max_item_bytes: usize,
    pub max_cfg_conditions_per_item: usize,
    pub max_cfg_condition_bytes: usize,
}

impl Default for RustFrontendLimits {
    fn default() -> Self {
        Self {
            max_source_bytes: 1024 * 1024,
            max_items: 65_536,
            max_unknowns: 4_096,
            max_item_bytes: 64 * 1024,
            max_cfg_conditions_per_item: 16,
            max_cfg_condition_bytes: 2_048,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct RustByteSpan {
    pub byte_start: usize,
    pub byte_end: usize,
}

impl RustByteSpan {
    fn from_range(range: Range<usize>) -> Self {
        Self {
            byte_start: range.start,
            byte_end: range.end,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum RustLogicalItemKind {
    Function,
    TestFunction,
    Reexport,
    Struct,
    Enum,
    Trait,
    TypeAlias,
    Module,
    Const,
    Static,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RustCfgCondition {
    pub expression: String,
    pub span: RustByteSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RustLogicalItem {
    pub kind: RustLogicalItemKind,
    pub name: String,
    pub span: RustByteSpan,
    pub name_span: RustByteSpan,
    pub cfg_conditions: Vec<RustCfgCondition>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum RustFrontendUnknownKind {
    SourceLimitExceeded,
    ItemLimitExceeded,
    ItemTooLarge,
    CfgLimitExceeded,
    UnsupportedItem,
    UnsupportedMacro,
    IncludeMacro,
    UnterminatedBlockComment,
    UnterminatedString,
    UnterminatedRawString,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct RustFrontendUnknown {
    pub kind: RustFrontendUnknownKind,
    pub span: RustByteSpan,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct RustFrontendReport {
    pub items: Vec<RustLogicalItem>,
    pub unknowns: Vec<RustFrontendUnknown>,
    pub truncated: bool,
}

#[must_use]
pub(crate) fn analyze_rust_logical_items(
    source: &str,
    limits: &RustFrontendLimits,
) -> RustFrontendReport {
    let mut builder = FrontendBuilder::new(*limits);
    if source.len() > limits.max_source_bytes {
        builder.truncated = true;
        builder.push_unknown(
            RustFrontendUnknownKind::SourceLimitExceeded,
            0..source.len(),
        );
        return builder.finish();
    }

    let (masked, terminal_unknown) = mask_rust_dead_zones(source);
    if let Some((kind, range)) = terminal_unknown {
        builder.push_unknown(kind, range);
    }
    scan_items(source, &masked, &mut builder);
    builder.finish()
}

struct FrontendBuilder {
    limits: RustFrontendLimits,
    items: Vec<RustLogicalItem>,
    unknowns: Vec<RustFrontendUnknown>,
    truncated: bool,
}

impl FrontendBuilder {
    fn new(limits: RustFrontendLimits) -> Self {
        Self {
            limits,
            items: Vec::new(),
            unknowns: Vec::new(),
            truncated: false,
        }
    }

    fn push_item(&mut self, item: RustLogicalItem) -> bool {
        if self.items.len() >= self.limits.max_items {
            self.truncated = true;
            self.push_unknown(
                RustFrontendUnknownKind::ItemLimitExceeded,
                item.span.byte_start..item.span.byte_end,
            );
            return false;
        }
        self.items.push(item);
        true
    }

    fn push_unknown(&mut self, kind: RustFrontendUnknownKind, range: Range<usize>) {
        if self.unknowns.len() >= self.limits.max_unknowns {
            self.truncated = true;
            return;
        }
        self.unknowns.push(RustFrontendUnknown {
            kind,
            span: RustByteSpan::from_range(range),
        });
    }

    fn finish(mut self) -> RustFrontendReport {
        self.unknowns.sort_unstable_by_key(|unknown| {
            (
                unknown.span.byte_start,
                unknown.span.byte_end,
                unknown_kind_rank(unknown.kind),
            )
        });
        self.unknowns.dedup();
        RustFrontendReport {
            items: self.items,
            unknowns: self.unknowns,
            truncated: self.truncated,
        }
    }
}

fn scan_items(source: &str, masked: &str, builder: &mut FrontendBuilder) {
    let bytes = masked.as_bytes();
    let mut cursor = 0usize;
    let mut pending_cfg = Vec::<RustCfgCondition>::new();
    let mut pending_is_test = false;
    while cursor < bytes.len() {
        if bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
            continue;
        }
        if bytes[cursor] == b'#' && bytes.get(cursor + 1) == Some(&b'[') {
            let end = find_balanced(masked, cursor + 1, builder.limits.max_item_bytes);
            let Some(end) = end else {
                let limit_end = cursor
                    .saturating_add(builder.limits.max_item_bytes)
                    .min(source.len());
                builder.push_unknown(RustFrontendUnknownKind::ItemTooLarge, cursor..limit_end);
                builder.truncated = true;
                break;
            };
            if let Some(condition) = cfg_condition(source, cursor..end, &builder.limits) {
                if pending_cfg.len() < builder.limits.max_cfg_conditions_per_item {
                    if !pending_cfg
                        .iter()
                        .any(|value| value.expression == condition.expression)
                    {
                        pending_cfg.push(condition);
                    }
                } else {
                    builder.push_unknown(RustFrontendUnknownKind::CfgLimitExceeded, cursor..end);
                }
            }
            pending_is_test |= is_test_attribute(source, cursor..end);
            cursor = end;
            continue;
        }
        if keyword_at(masked, cursor, "pub") {
            match parse_public_item(source, masked, cursor, &pending_cfg, &builder.limits) {
                ParsePublic::Item(mut item, next) => {
                    if pending_is_test && item.kind == RustLogicalItemKind::Function {
                        item.kind = RustLogicalItemKind::TestFunction;
                    }
                    let enter_public_module = item.kind == RustLogicalItemKind::Module;
                    let module_body_start = item.span.byte_end.saturating_add(1);
                    pending_cfg.clear();
                    pending_is_test = false;
                    cursor = if enter_public_module {
                        module_body_start
                    } else {
                        next
                    }
                    .max(cursor + 1);
                    if !builder.push_item(item) {
                        break;
                    }
                }
                ParsePublic::Restricted(next) => {
                    pending_cfg.clear();
                    pending_is_test = false;
                    cursor = next.max(cursor + 1);
                }
                ParsePublic::Unsupported(range, next) => {
                    pending_cfg.clear();
                    pending_is_test = false;
                    builder.push_unknown(RustFrontendUnknownKind::UnsupportedItem, range);
                    cursor = next.max(cursor + 1);
                }
            }
            continue;
        }
        if keyword_at(masked, cursor, "fn") {
            if pending_is_test {
                match parse_named_item(
                    masked,
                    cursor,
                    cursor,
                    cursor + "fn".len(),
                    RustLogicalItemKind::TestFunction,
                    &pending_cfg,
                    &builder.limits,
                ) {
                    ParsePublic::Item(item, next) => {
                        cursor = next.max(cursor + 1);
                        if !builder.push_item(item) {
                            break;
                        }
                    }
                    ParsePublic::Restricted(next) => cursor = next.max(cursor + 1),
                    ParsePublic::Unsupported(range, next) => {
                        builder.push_unknown(RustFrontendUnknownKind::UnsupportedItem, range);
                        cursor = next.max(cursor + 1);
                    }
                }
            } else {
                cursor = skip_item_extent(masked, cursor, builder.limits.max_item_bytes)
                    .unwrap_or(cursor + 2);
            }
            pending_cfg.clear();
            pending_is_test = false;
            continue;
        }
        if let Some((name, name_end)) = identifier_at(masked, cursor) {
            let bang = skip_ascii_whitespace(bytes, name_end);
            if bytes.get(bang) == Some(&b'!') {
                let end = if name == "macro_rules" {
                    macro_rules_extent(masked, cursor, bang, builder.limits.max_item_bytes)
                } else {
                    macro_extent(masked, cursor, bang, builder.limits.max_item_bytes)
                };
                let kind = if name == "include" || name == "include_str" || name == "include_bytes"
                {
                    RustFrontendUnknownKind::IncludeMacro
                } else {
                    RustFrontendUnknownKind::UnsupportedMacro
                };
                builder.push_unknown(kind, cursor..end);
                pending_cfg.clear();
                pending_is_test = false;
                cursor = end.max(cursor + 1);
                continue;
            }
        }
        pending_cfg.clear();
        pending_is_test = false;
        cursor += utf8_width(bytes[cursor]);
    }
}

enum ParsePublic {
    Item(RustLogicalItem, usize),
    Restricted(usize),
    Unsupported(Range<usize>, usize),
}

fn parse_public_item(
    source: &str,
    masked: &str,
    start: usize,
    cfg_conditions: &[RustCfgCondition],
    limits: &RustFrontendLimits,
) -> ParsePublic {
    let bytes = masked.as_bytes();
    let mut cursor = skip_ascii_whitespace(bytes, start + "pub".len());
    if bytes.get(cursor) == Some(&b'(') {
        let visibility_end = find_balanced(masked, cursor, limits.max_item_bytes)
            .unwrap_or_else(|| cursor.saturating_add(1).min(masked.len()));
        let next = skip_item_extent(masked, visibility_end, limits.max_item_bytes)
            .unwrap_or(visibility_end);
        return ParsePublic::Restricted(next);
    }

    loop {
        let Some((modifier, end)) = identifier_at(masked, cursor) else {
            return ParsePublic::Unsupported(
                start..cursor.min(masked.len()),
                cursor.min(masked.len()),
            );
        };
        match modifier {
            "async" | "unsafe" | "default" | "extern" => {
                cursor = skip_ascii_whitespace(bytes, end);
            }
            "const" => {
                let after_const = skip_ascii_whitespace(bytes, end);
                if keyword_at(masked, after_const, "fn") {
                    cursor = after_const;
                } else {
                    return parse_named_item(
                        masked,
                        start,
                        cursor,
                        end,
                        RustLogicalItemKind::Const,
                        cfg_conditions,
                        limits,
                    );
                }
            }
            "fn" => {
                return parse_named_item(
                    masked,
                    start,
                    cursor,
                    end,
                    RustLogicalItemKind::Function,
                    cfg_conditions,
                    limits,
                );
            }
            "use" => {
                return parse_reexport(source, masked, start, end, cfg_conditions, limits);
            }
            "struct" => {
                return parse_named_item(
                    masked,
                    start,
                    cursor,
                    end,
                    RustLogicalItemKind::Struct,
                    cfg_conditions,
                    limits,
                );
            }
            "enum" => {
                return parse_named_item(
                    masked,
                    start,
                    cursor,
                    end,
                    RustLogicalItemKind::Enum,
                    cfg_conditions,
                    limits,
                );
            }
            "trait" => {
                return parse_named_item(
                    masked,
                    start,
                    cursor,
                    end,
                    RustLogicalItemKind::Trait,
                    cfg_conditions,
                    limits,
                );
            }
            "type" => {
                return parse_named_item(
                    masked,
                    start,
                    cursor,
                    end,
                    RustLogicalItemKind::TypeAlias,
                    cfg_conditions,
                    limits,
                );
            }
            "mod" => {
                return parse_named_item(
                    masked,
                    start,
                    cursor,
                    end,
                    RustLogicalItemKind::Module,
                    cfg_conditions,
                    limits,
                );
            }
            "static" => {
                return parse_named_item(
                    masked,
                    start,
                    cursor,
                    end,
                    RustLogicalItemKind::Static,
                    cfg_conditions,
                    limits,
                );
            }
            _ => {
                return ParsePublic::Unsupported(start..end, end);
            }
        }
    }
}

fn parse_named_item(
    masked: &str,
    item_start: usize,
    keyword_start: usize,
    keyword_end: usize,
    kind: RustLogicalItemKind,
    cfg_conditions: &[RustCfgCondition],
    limits: &RustFrontendLimits,
) -> ParsePublic {
    let mut cursor = skip_ascii_whitespace(masked.as_bytes(), keyword_end);
    let Some((name, name_end)) = rust_identifier_at(masked, cursor) else {
        let end = cursor.saturating_add(1).min(masked.len());
        return ParsePublic::Unsupported(item_start..end, end);
    };
    let raw_name_start = if masked[cursor..].starts_with("r#") {
        cursor + 2
    } else {
        cursor
    };
    cursor = name_end;
    let Some((header_end, item_end)) = item_extent(masked, cursor, limits.max_item_bytes) else {
        let end = item_start
            .saturating_add(limits.max_item_bytes)
            .min(masked.len());
        return ParsePublic::Unsupported(item_start..end, end);
    };
    let _ = keyword_start;
    ParsePublic::Item(
        RustLogicalItem {
            kind,
            name: name.to_string(),
            span: RustByteSpan::from_range(item_start..header_end),
            name_span: RustByteSpan::from_range(raw_name_start..name_end),
            cfg_conditions: cfg_conditions.to_vec(),
        },
        item_end,
    )
}

fn parse_reexport(
    _source: &str,
    masked: &str,
    item_start: usize,
    use_end: usize,
    cfg_conditions: &[RustCfgCondition],
    limits: &RustFrontendLimits,
) -> ParsePublic {
    let target_start = skip_ascii_whitespace(masked.as_bytes(), use_end);
    let Some(end) = find_statement_end(masked, target_start, limits.max_item_bytes) else {
        let end = item_start
            .saturating_add(limits.max_item_bytes)
            .min(masked.len());
        return ParsePublic::Unsupported(item_start..end, end);
    };
    let target_end = end.saturating_sub(1);
    let Some(target) = masked.get(target_start..target_end) else {
        return ParsePublic::Unsupported(item_start..end, end);
    };
    let target = normalize_space(target);
    if target.is_empty() {
        return ParsePublic::Unsupported(item_start..end, end);
    }
    ParsePublic::Item(
        RustLogicalItem {
            kind: RustLogicalItemKind::Reexport,
            name: target,
            span: RustByteSpan::from_range(item_start..end),
            name_span: RustByteSpan::from_range(target_start..target_end),
            cfg_conditions: cfg_conditions.to_vec(),
        },
        end,
    )
}

fn cfg_condition(
    source: &str,
    range: Range<usize>,
    limits: &RustFrontendLimits,
) -> Option<RustCfgCondition> {
    let raw = source.get(range.clone())?.trim();
    let inner = raw.strip_prefix("#[")?.strip_suffix(']')?.trim();
    let expression = inner.strip_prefix("cfg")?.trim();
    let expression = expression.strip_prefix('(')?.strip_suffix(')')?.trim();
    if expression.is_empty() || expression.len() > limits.max_cfg_condition_bytes {
        return None;
    }
    Some(RustCfgCondition {
        expression: normalize_space(expression),
        span: RustByteSpan::from_range(range),
    })
}

fn is_test_attribute(source: &str, range: Range<usize>) -> bool {
    source
        .get(range)
        .map(str::trim)
        .and_then(|attribute| attribute.strip_prefix("#["))
        .and_then(|attribute| attribute.strip_suffix(']'))
        .is_some_and(|attribute| attribute.trim() == "test")
}

fn normalize_space(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn item_extent(masked: &str, start: usize, limit: usize) -> Option<(usize, usize)> {
    let bytes = masked.as_bytes();
    let scan_end = start.saturating_add(limit).min(bytes.len());
    let mut paren = 0usize;
    let mut bracket = 0usize;
    let mut cursor = start;
    while cursor < scan_end {
        match bytes[cursor] {
            b'(' => paren = paren.saturating_add(1),
            b')' => paren = paren.saturating_sub(1),
            b'[' => bracket = bracket.saturating_add(1),
            b']' => bracket = bracket.saturating_sub(1),
            b';' if paren == 0 && bracket == 0 => return Some((cursor + 1, cursor + 1)),
            b'{' if paren == 0 && bracket == 0 => {
                let end = find_balanced(masked, cursor, scan_end.saturating_sub(cursor))?;
                return Some((cursor, end));
            }
            _ => {}
        }
        cursor += 1;
    }
    None
}

fn skip_item_extent(masked: &str, start: usize, limit: usize) -> Option<usize> {
    item_extent(masked, start, limit).map(|(_, end)| end)
}

fn find_statement_end(masked: &str, start: usize, limit: usize) -> Option<usize> {
    let bytes = masked.as_bytes();
    let scan_end = start.saturating_add(limit).min(bytes.len());
    let mut stack = Vec::new();
    let mut cursor = start;
    while cursor < scan_end {
        match bytes[cursor] {
            b'(' | b'[' | b'{' => stack.push(bytes[cursor]),
            b')' | b']' | b'}' => {
                stack.pop()?;
            }
            b';' if stack.is_empty() => return Some(cursor + 1),
            _ => {}
        }
        cursor += 1;
    }
    None
}

fn find_balanced(masked: &str, open: usize, limit: usize) -> Option<usize> {
    let bytes = masked.as_bytes();
    let expected = matching_close(*bytes.get(open)?)?;
    let scan_end = open.saturating_add(limit).min(bytes.len());
    let mut stack = vec![expected];
    let mut cursor = open + 1;
    while cursor < scan_end {
        if let Some(close) = matching_close(bytes[cursor]) {
            stack.push(close);
        } else if stack.last() == Some(&bytes[cursor]) {
            stack.pop();
            if stack.is_empty() {
                return Some(cursor + 1);
            }
        }
        cursor += 1;
    }
    None
}

const fn matching_close(open: u8) -> Option<u8> {
    match open {
        b'(' => Some(b')'),
        b'[' => Some(b']'),
        b'{' => Some(b'}'),
        _ => None,
    }
}

const fn unknown_kind_rank(kind: RustFrontendUnknownKind) -> u8 {
    match kind {
        RustFrontendUnknownKind::SourceLimitExceeded => 0,
        RustFrontendUnknownKind::ItemLimitExceeded => 1,
        RustFrontendUnknownKind::ItemTooLarge => 2,
        RustFrontendUnknownKind::CfgLimitExceeded => 3,
        RustFrontendUnknownKind::UnsupportedItem => 4,
        RustFrontendUnknownKind::UnsupportedMacro => 5,
        RustFrontendUnknownKind::IncludeMacro => 6,
        RustFrontendUnknownKind::UnterminatedBlockComment => 7,
        RustFrontendUnknownKind::UnterminatedString => 8,
        RustFrontendUnknownKind::UnterminatedRawString => 9,
    }
}

fn macro_extent(masked: &str, start: usize, bang: usize, limit: usize) -> usize {
    let bytes = masked.as_bytes();
    let delimiter = skip_ascii_whitespace(bytes, bang + 1);
    let end = if bytes
        .get(delimiter)
        .copied()
        .and_then(matching_close)
        .is_some()
    {
        find_balanced(masked, delimiter, limit).unwrap_or(delimiter + 1)
    } else {
        bang + 1
    };
    let semicolon = skip_ascii_whitespace(bytes, end);
    if bytes.get(semicolon) == Some(&b';') {
        semicolon + 1
    } else {
        end.max(start + 1)
    }
}

fn macro_rules_extent(masked: &str, start: usize, bang: usize, limit: usize) -> usize {
    let bytes = masked.as_bytes();
    let name_start = skip_ascii_whitespace(bytes, bang + 1);
    let Some((_, name_end)) = identifier_at(masked, name_start) else {
        return macro_extent(masked, start, bang, limit);
    };
    let delimiter = skip_ascii_whitespace(bytes, name_end);
    let end = if bytes
        .get(delimiter)
        .copied()
        .and_then(matching_close)
        .is_some()
    {
        find_balanced(masked, delimiter, limit).unwrap_or(delimiter + 1)
    } else {
        delimiter.max(start + 1)
    };
    let semicolon = skip_ascii_whitespace(bytes, end);
    if bytes.get(semicolon) == Some(&b';') {
        semicolon + 1
    } else {
        end
    }
}

fn keyword_at(source: &str, start: usize, keyword: &str) -> bool {
    source
        .get(start..start.saturating_add(keyword.len()))
        .is_some_and(|value| value == keyword)
        && source[..start]
            .chars()
            .next_back()
            .is_none_or(|character| !is_identifier_continue(character))
        && source[start + keyword.len()..]
            .chars()
            .next()
            .is_none_or(|character| !is_identifier_continue(character))
}

fn identifier_at(source: &str, start: usize) -> Option<(&str, usize)> {
    let tail = source.get(start..)?;
    let mut chars = tail.char_indices();
    let (_, first) = chars.next()?;
    if !first.is_ascii_alphabetic() && first != '_' {
        return None;
    }
    let mut end = start + first.len_utf8();
    for (offset, character) in chars {
        if !is_identifier_continue(character) {
            break;
        }
        end = start + offset + character.len_utf8();
    }
    Some((&source[start..end], end))
}

fn rust_identifier_at(source: &str, start: usize) -> Option<(&str, usize)> {
    if source.get(start..)?.starts_with("r#") {
        return identifier_at(source, start + 2);
    }
    identifier_at(source, start)
}

const fn is_identifier_continue(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_'
}

fn skip_ascii_whitespace(bytes: &[u8], mut cursor: usize) -> usize {
    while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
        cursor += 1;
    }
    cursor
}

const fn utf8_width(first: u8) -> usize {
    if first < 0x80 {
        1
    } else if first < 0xe0 {
        2
    } else if first < 0xf0 {
        3
    } else {
        4
    }
}

fn mask_rust_dead_zones(source: &str) -> (String, Option<(RustFrontendUnknownKind, Range<usize>)>) {
    #[derive(Clone, Copy)]
    enum State {
        Normal,
        LineComment,
        BlockComment { depth: usize, start: usize },
        String { escaped: bool, start: usize },
        Character { escaped: bool, start: usize },
        RawString { hashes: usize, start: usize },
    }

    let bytes = source.as_bytes();
    let mut output = bytes.to_vec();
    let mut state = State::Normal;
    let mut cursor = 0usize;
    while cursor < bytes.len() {
        let current = bytes[cursor];
        let next = bytes.get(cursor + 1).copied();
        match state {
            State::Normal if current == b'/' && next == Some(b'/') => {
                mask_bytes(&mut output, cursor..cursor + 2);
                state = State::LineComment;
                cursor += 2;
                continue;
            }
            State::Normal if current == b'/' && next == Some(b'*') => {
                mask_bytes(&mut output, cursor..cursor + 2);
                state = State::BlockComment {
                    depth: 1,
                    start: cursor,
                };
                cursor += 2;
                continue;
            }
            State::Normal => {
                if let Some((delimiter_end, hashes)) = raw_string_open(bytes, cursor) {
                    mask_bytes(&mut output, cursor..delimiter_end);
                    state = State::RawString {
                        hashes,
                        start: cursor,
                    };
                    cursor = delimiter_end;
                    continue;
                }
                if current == b'"' {
                    output[cursor] = b' ';
                    state = State::String {
                        escaped: false,
                        start: cursor,
                    };
                } else if current == b'\'' && looks_like_character_literal(bytes, cursor) {
                    output[cursor] = b' ';
                    state = State::Character {
                        escaped: false,
                        start: cursor,
                    };
                }
            }
            State::LineComment if current == b'\n' => state = State::Normal,
            State::LineComment => output[cursor] = b' ',
            State::BlockComment { depth, start } if current == b'/' && next == Some(b'*') => {
                mask_bytes(&mut output, cursor..cursor + 2);
                state = State::BlockComment {
                    depth: depth.saturating_add(1),
                    start,
                };
                cursor += 2;
                continue;
            }
            State::BlockComment { depth, start } if current == b'*' && next == Some(b'/') => {
                mask_bytes(&mut output, cursor..cursor + 2);
                state = if depth == 1 {
                    State::Normal
                } else {
                    State::BlockComment {
                        depth: depth - 1,
                        start,
                    }
                };
                cursor += 2;
                continue;
            }
            State::BlockComment { .. } if current != b'\n' => output[cursor] = b' ',
            State::String { escaped, start } => {
                if current != b'\n' {
                    output[cursor] = b' ';
                }
                if current == b'"' && !escaped {
                    state = State::Normal;
                } else {
                    state = State::String {
                        escaped: current == b'\\' && !escaped,
                        start,
                    };
                }
            }
            State::Character { escaped, start } => {
                if current != b'\n' {
                    output[cursor] = b' ';
                }
                if current == b'\'' && !escaped {
                    state = State::Normal;
                } else {
                    state = State::Character {
                        escaped: current == b'\\' && !escaped,
                        start,
                    };
                }
            }
            State::RawString { hashes, start } => {
                if current != b'\n' {
                    output[cursor] = b' ';
                }
                if current == b'"' && raw_string_close(bytes, cursor, hashes) {
                    let end = cursor + 1 + hashes;
                    mask_bytes(&mut output, cursor..end);
                    state = State::Normal;
                    cursor = end;
                    continue;
                }
                state = State::RawString { hashes, start };
            }
            State::BlockComment { .. } => {}
        }
        cursor += 1;
    }

    let terminal_unknown = match state {
        State::BlockComment { start, .. } => Some((
            RustFrontendUnknownKind::UnterminatedBlockComment,
            start..source.len(),
        )),
        State::String { start, .. } | State::Character { start, .. } => Some((
            RustFrontendUnknownKind::UnterminatedString,
            start..source.len(),
        )),
        State::RawString { start, .. } => Some((
            RustFrontendUnknownKind::UnterminatedRawString,
            start..source.len(),
        )),
        State::Normal | State::LineComment => None,
    };
    (
        String::from_utf8(output).expect("ASCII masking preserves byte length and UTF-8"),
        terminal_unknown,
    )
}

fn raw_string_open(bytes: &[u8], start: usize) -> Option<(usize, usize)> {
    let mut cursor = start;
    if bytes.get(cursor) == Some(&b'b') {
        cursor += 1;
    }
    if bytes.get(cursor) != Some(&b'r') {
        return None;
    }
    cursor += 1;
    let hash_start = cursor;
    while bytes.get(cursor) == Some(&b'#') {
        cursor += 1;
    }
    (bytes.get(cursor) == Some(&b'"')).then_some((cursor + 1, cursor - hash_start))
}

fn raw_string_close(bytes: &[u8], quote: usize, hashes: usize) -> bool {
    (0..hashes).all(|offset| bytes.get(quote + 1 + offset) == Some(&b'#'))
}

fn looks_like_character_literal(bytes: &[u8], start: usize) -> bool {
    let mut cursor = start + 1;
    if bytes.get(cursor) == Some(&b'\\') {
        cursor += 2;
    } else {
        cursor += utf8_width(*bytes.get(cursor).unwrap_or(&b' '));
    }
    bytes.get(cursor) == Some(&b'\'')
}

fn mask_bytes(output: &mut [u8], range: Range<usize>) {
    for byte in &mut output[range] {
        if !matches!(*byte, b'\r' | b'\n') {
            *byte = b' ';
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multiline_public_function_retains_cfg_condition() {
        let source = r#"
#[cfg(feature = "remote")]
pub async fn
    audit_repository(
        root: &std::path::Path,
    ) -> Result<(), ()>
{
    let _ = root;
    Ok(())
}
"#;
        let report = analyze_rust_logical_items(source, &RustFrontendLimits::default());

        assert!(!report.truncated);
        assert!(report.unknowns.is_empty());
        assert_eq!(report.items.len(), 1);
        let item = &report.items[0];
        assert_eq!(item.kind, RustLogicalItemKind::Function);
        assert_eq!(item.name, "audit_repository");
        assert_eq!(item.cfg_conditions.len(), 1);
        assert_eq!(item.cfg_conditions[0].expression, "feature = \"remote\"");
        assert_eq!(
            &source[item.name_span.byte_start..item.name_span.byte_end],
            "audit_repository"
        );
    }

    #[test]
    fn public_reexport_is_kept_and_restricted_visibility_is_not_external() {
        let source = r#"
pub use crate::api::{AuditError, AuditReport};
pub(crate) fn internal_scan() {}
pub(super) use crate::private::Hidden;
"#;
        let report = analyze_rust_logical_items(source, &RustFrontendLimits::default());

        assert_eq!(report.items.len(), 1);
        assert_eq!(report.items[0].kind, RustLogicalItemKind::Reexport);
        assert_eq!(
            report.items[0].name,
            "crate::api::{AuditError, AuditReport}"
        );
        assert!(!report
            .items
            .iter()
            .any(|item| item.name.contains("internal") || item.name.contains("Hidden")));
    }

    #[test]
    fn strings_raw_strings_and_comments_cannot_create_false_items() {
        let source = r###"
// pub fn line_comment_fake() {}
/* pub use fake::BlockComment; */
const NORMAL: &str = "pub fn string_fake() {}";
const RAW: &str = r##"pub fn raw_fake() {}; include!("fake.rs");"##;
pub fn real_status() -> &'static str { "ready" }
"###;
        let report = analyze_rust_logical_items(source, &RustFrontendLimits::default());

        assert_eq!(report.items.len(), 1);
        assert_eq!(report.items[0].name, "real_status");
        assert!(report.unknowns.is_empty());
    }

    #[test]
    fn unsupported_macros_are_unknown_only_at_their_source_regions() {
        let source = r#"
macro_rules! declare_capability { ($name:ident) => { pub fn $name() {} }; }
declare_capability!(generated_scan);
include!(concat!(env!("OUT_DIR"), "/generated.rs"));
pub fn observed() {}
"#;
        let report = analyze_rust_logical_items(source, &RustFrontendLimits::default());

        assert_eq!(report.items.len(), 1);
        assert_eq!(report.items[0].name, "observed");
        assert!(report
            .unknowns
            .iter()
            .any(|unknown| unknown.kind == RustFrontendUnknownKind::UnsupportedMacro));
        assert!(report
            .unknowns
            .iter()
            .any(|unknown| unknown.kind == RustFrontendUnknownKind::IncludeMacro));
        for unknown in &report.unknowns {
            assert!(unknown.span.byte_start < unknown.span.byte_end);
            assert!(unknown.span.byte_end <= source.len());
        }
    }

    #[test]
    fn source_and_item_limits_are_fail_visible_and_bounded() {
        let source = "pub fn one() {}\npub fn two() {}\n";
        let report = analyze_rust_logical_items(
            source,
            &RustFrontendLimits {
                max_items: 1,
                ..RustFrontendLimits::default()
            },
        );
        assert!(report.truncated);
        assert_eq!(report.items.len(), 1);
        assert!(report
            .unknowns
            .iter()
            .any(|unknown| unknown.kind == RustFrontendUnknownKind::ItemLimitExceeded));

        let oversized = analyze_rust_logical_items(
            source,
            &RustFrontendLimits {
                max_source_bytes: source.len() - 1,
                ..RustFrontendLimits::default()
            },
        );
        assert!(oversized.truncated);
        assert!(oversized.items.is_empty());
        assert_eq!(oversized.unknowns.len(), 1);
        assert_eq!(
            oversized.unknowns[0].kind,
            RustFrontendUnknownKind::SourceLimitExceeded
        );
    }
}

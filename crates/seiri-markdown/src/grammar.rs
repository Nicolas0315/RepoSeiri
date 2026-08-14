use crate::context::mask_hidden_contexts;
use crate::polarity::{assess_predicate_polarity, has_explicit_negative_cue, PolarityAssessment};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use seiri_core::{
    AnswerState, ClaimAtom, ClaimAtomIR, ClaimAtomId, ClaimPolarity, CoverageIncompleteReason,
    CoverageStatus, DocumentEvent, DocumentLanguage, DocumentScan, GrammarDiagnostic,
    GrammarDiagnosticKind, GrammarEdge, GrammarNode, GrammarNodeId, GrammarPredicate,
    NarrativeRelation, ReadmeGrammarIR, ReadmeSection, ReadmeSectionAlignment, ReadmeSectionId,
    ReadmeSectionLanguage, ReadmeTranslationAlignmentIR, SourceSpan, TextDocumentBase,
    TranslationAlignmentState, TranslationClaimAlignment, TranslationDivergenceKind, UnknownReason,
    README_CLAIM_ATOM_REVISION,
};
use std::collections::BTreeSet;
use std::num::NonZeroU32;
use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadmeGrammarOptions {
    pub max_nodes: usize,
    pub max_edges: usize,
}

impl Default for ReadmeGrammarOptions {
    fn default() -> Self {
        Self {
            max_nodes: 4_096,
            max_edges: 8_192,
        }
    }
}

pub fn analyze_readme_grammar(
    document: &DocumentScan,
    options: &ReadmeGrammarOptions,
) -> Result<ReadmeGrammarIR, seiri_core::AppealIrError> {
    analyze_clauses(document, compatibility_document(document), options)
}

/// Reconstructs visible Markdown clauses from the exact source bound to `document`.
///
/// Unlike [`analyze_readme_grammar`], this path keeps inline text together across
/// emphasis, links, and code spans before applying clause-local modality and
/// negation. The source must be the same byte sequence that produced the scan.
pub fn analyze_readme_grammar_with_source(
    document: &DocumentScan,
    source: &str,
    options: &ReadmeGrammarOptions,
) -> Result<ReadmeGrammarIR, seiri_core::AppealIrError> {
    let supplied_base = TextDocumentBase::from_bytes(source.as_bytes());
    if document.source_bytes() != source.len() || document.base().digest() != supplied_base.digest()
    {
        return Err(seiri_core::AppealIrError::SourceMismatch);
    }
    analyze_clauses(document, visible_document(source), options)
}

fn analyze_clauses(
    document: &DocumentScan,
    mut visible: VisibleDocument,
    options: &ReadmeGrammarOptions,
) -> Result<ReadmeGrammarIR, seiri_core::AppealIrError> {
    let mut nodes = Vec::new();
    let mut atoms = Vec::new();
    let mut diagnostics = Vec::new();
    let mut truncated = false;
    let clauses = std::mem::take(&mut visible.clauses);
    for clause in clauses {
        let normalized = normalize_for_matching(&clause.text);
        let predicates = predicates_for(&normalized);
        if predicates.is_empty() {
            continue;
        }
        let section_language = clause
            .section
            .and_then(|index| visible.sections.get(index))
            .map(|section| section.language);
        let Some(language) = confident_clause_language(&clause.text, section_language) else {
            diagnostics.push(GrammarDiagnostic {
                kind: GrammarDiagnosticKind::AmbiguousLanguage,
                span: Some(clause.span),
            });
            push_unknown_reason(
                &mut visible.unknown_reasons,
                UnknownReason::UnsupportedSyntax,
            );
            continue;
        };
        for predicate in predicates {
            if nodes.len() >= options.max_nodes || nodes.len() >= u32::MAX as usize {
                truncated = true;
                break;
            }
            let negated = match assess_predicate_polarity(&normalized, predicate) {
                PolarityAssessment::Positive => false,
                PolarityAssessment::Negative => true,
                PolarityAssessment::Ambiguous => {
                    diagnostics.push(GrammarDiagnostic {
                        kind: GrammarDiagnosticKind::ConflictingClause,
                        span: Some(clause.span),
                    });
                    push_unknown_reason(
                        &mut visible.unknown_reasons,
                        UnknownReason::UnsupportedSyntax,
                    );
                    continue;
                }
            };
            let modality = modality_of(&normalized, negated);
            let id = NonZeroU32::new((nodes.len() + 1) as u32)
                .map(GrammarNodeId::new)
                .expect("node count was checked");
            nodes.push(GrammarNode {
                id,
                predicate,
                state: AnswerState::Explicit,
                language,
                modality,
                negated,
                span: Some(clause.span),
            });
            let atom_id = ClaimAtomId::new(
                NonZeroU32::new((atoms.len() + 1) as u32).expect("node count was checked"),
            );
            let terms = claim_terms(predicate, &normalized, language);
            atoms.push(ClaimAtom {
                id: atom_id,
                grammar_node: id,
                dimension: predicate.dimension(),
                subject: terms.subject,
                action: terms.action,
                object: terms.object,
                qualifiers: terms.qualifiers,
                condition: terms.condition,
                polarity: if negated {
                    ClaimPolarity::Negative
                } else {
                    ClaimPolarity::Positive
                },
                modality,
                language,
                span: Some(clause.span),
            });
            if let Some(section) = clause
                .section
                .and_then(|index| visible.sections.get_mut(index))
            {
                section.claim_atoms.push(atom_id);
            }
        }
        if truncated {
            break;
        }
    }

    if truncated {
        diagnostics.push(GrammarDiagnostic {
            kind: GrammarDiagnosticKind::NodeLimitExceeded,
            span: None,
        });
    }
    for section in &visible.sections {
        if matches!(section.language, ReadmeSectionLanguage::Ambiguous(_))
            && !diagnostics.iter().any(|diagnostic| {
                diagnostic.kind == GrammarDiagnosticKind::AmbiguousLanguage
                    && diagnostic.span == Some(section.span)
            })
        {
            diagnostics.push(GrammarDiagnostic {
                kind: GrammarDiagnosticKind::AmbiguousLanguage,
                span: Some(section.span),
            });
        }
    }
    diagnostics.sort_unstable_by_key(|diagnostic| {
        (
            diagnostic.span.map_or(usize::MAX, |span| span.byte_start),
            grammar_diagnostic_rank(diagnostic.kind),
        )
    });
    diagnostics.dedup();
    let claim_atoms = ClaimAtomIR {
        semantic_revision: README_CLAIM_ATOM_REVISION.to_string(),
        atoms,
    };
    let translation_alignment = build_translation_alignment(document, visible, &claim_atoms)?;
    let typed_translation =
        (!translation_alignment.sections.is_empty()).then_some(&translation_alignment);
    let edges = build_edges(
        &nodes,
        &claim_atoms,
        typed_translation,
        options.max_edges,
        &mut diagnostics,
    );
    let coverage = if truncated
        || diagnostics
            .iter()
            .any(|diagnostic| diagnostic.kind == GrammarDiagnosticKind::NodeLimitExceeded)
    {
        CoverageStatus::Partial(CoverageIncompleteReason::LimitExceeded)
    } else if diagnostics.iter().any(|diagnostic| {
        matches!(
            diagnostic.kind,
            GrammarDiagnosticKind::AmbiguousLanguage | GrammarDiagnosticKind::ConflictingClause
        )
    }) {
        CoverageStatus::Partial(CoverageIncompleteReason::UnsupportedSyntax)
    } else {
        CoverageStatus::Complete
    };
    ReadmeGrammarIR::try_new_with_claim_atoms_and_translation_alignment(
        document.path(),
        coverage,
        nodes,
        edges,
        diagnostics,
        claim_atoms,
        translation_alignment,
    )
}

#[derive(Debug)]
struct VisibleClause {
    text: String,
    span: SourceSpan,
    section: Option<usize>,
}

#[derive(Debug, Default)]
struct VisibleDocument {
    clauses: Vec<VisibleClause>,
    sections: Vec<VisibleSection>,
    unknown_reasons: Vec<UnknownReason>,
}

#[derive(Debug)]
struct VisibleSection {
    span: SourceSpan,
    language: ReadmeSectionLanguage,
    claim_atoms: Vec<ClaimAtomId>,
}

#[derive(Debug)]
struct VisibleHeading {
    text: String,
    span: SourceSpan,
}

fn compatibility_document(document: &DocumentScan) -> VisibleDocument {
    let prose_spans = document
        .events()
        .iter()
        .filter_map(|event| match event {
            DocumentEvent::VisibleProse(prose) => Some(prose.span),
            _ => None,
        })
        .collect::<Vec<_>>();
    let clauses = document
        .events()
        .iter()
        .filter_map(|event| match event {
            DocumentEvent::VisibleProse(prose) => Some(VisibleClause {
                text: prose.text.clone(),
                span: prose.span,
                section: None,
            }),
            DocumentEvent::Heading(heading)
                if heading.span.is_some_and(|heading_span| {
                    !prose_spans
                        .iter()
                        .any(|prose_span| span_contains(heading_span, *prose_span))
                }) =>
            {
                Some(VisibleClause {
                    text: heading.text.clone(),
                    span: heading.span.expect("guard requires a heading span"),
                    section: None,
                })
            }
            DocumentEvent::Heading(_)
            | DocumentEvent::Link(_)
            | DocumentEvent::Badge(_)
            | DocumentEvent::RouteCandidate(_) => None,
        })
        .collect();
    VisibleDocument {
        clauses,
        ..VisibleDocument::default()
    }
}

const fn span_contains(outer: SourceSpan, inner: SourceSpan) -> bool {
    outer.byte_start <= inner.byte_start && inner.byte_end <= outer.byte_end
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VisibleBlockKind {
    Paragraph,
    Heading,
}

#[derive(Debug)]
struct VisiblePiece {
    output: Range<usize>,
    source: Range<usize>,
    direct_mapping: bool,
}

#[derive(Debug)]
struct VisibleBlock {
    kind: VisibleBlockKind,
    text: String,
    pieces: Vec<VisiblePiece>,
}

impl VisibleBlock {
    fn new(kind: VisibleBlockKind) -> Self {
        Self {
            kind,
            text: String::new(),
            pieces: Vec::new(),
        }
    }

    fn push_text(&mut self, text: &str, source: Range<usize>, direct_mapping: bool) {
        if text.is_empty() {
            return;
        }
        let start = self.text.len();
        self.text.push_str(text);
        self.pieces.push(VisiblePiece {
            output: start..self.text.len(),
            source,
            direct_mapping,
        });
    }

    fn finish(
        self,
        lines: &SourceLineIndex,
        output: &mut Vec<VisibleClause>,
        headings: &mut Vec<VisibleHeading>,
    ) {
        if self.kind == VisibleBlockKind::Heading {
            if let Some(span) = self.source_span(0..self.text.len(), lines) {
                headings.push(VisibleHeading {
                    text: self.text.clone(),
                    span,
                });
            }
        }
        for range in clause_ranges(&self.text) {
            let Some(span) = self.source_span(range.clone(), lines) else {
                continue;
            };
            output.push(VisibleClause {
                text: self.text[range].to_string(),
                span,
                section: None,
            });
        }
    }

    fn source_span(&self, output: Range<usize>, lines: &SourceLineIndex) -> Option<SourceSpan> {
        let first = self
            .pieces
            .iter()
            .find(|piece| piece.output.end > output.start && piece.output.start < output.end)?;
        let last = self
            .pieces
            .iter()
            .rev()
            .find(|piece| piece.output.start < output.end && piece.output.end > output.start)?;

        let byte_start = if first.direct_mapping {
            first.source.start + output.start.saturating_sub(first.output.start)
        } else {
            first.source.start
        };
        let byte_end = if last.direct_mapping {
            last.source.start
                + output
                    .end
                    .min(last.output.end)
                    .saturating_sub(last.output.start)
        } else {
            last.source.end
        };
        Some(lines.span(byte_start..byte_end))
    }
}

#[derive(Debug)]
struct SourceLineIndex<'source> {
    starts: Vec<usize>,
    source: &'source str,
}

impl<'source> SourceLineIndex<'source> {
    fn new(source: &'source str) -> Self {
        let mut starts = vec![0];
        starts.extend(
            source
                .bytes()
                .enumerate()
                .filter_map(|(index, byte)| (byte == b'\n').then_some(index + 1)),
        );
        Self { starts, source }
    }

    fn span(&self, range: Range<usize>) -> SourceSpan {
        let line_index = self.starts.partition_point(|start| *start <= range.start) - 1;
        let line_start = self.starts[line_index];
        let column = self.source[line_start..range.start].chars().count() + 1;
        SourceSpan::new(line_index + 1, column, range.start, range.end)
    }
}

fn visible_document(source: &str) -> VisibleDocument {
    let masked = mask_hidden_contexts(source);
    debug_assert_eq!(masked.len(), source.len());
    let lines = SourceLineIndex::new(source);
    let mut clauses = Vec::new();
    let mut headings = Vec::new();
    let mut block = None::<VisibleBlock>;
    let mut code_block_depth = 0usize;
    let mut image_depth = 0usize;

    for (event, range) in Parser::new_ext(source, Options::empty()).into_offset_iter() {
        match event {
            Event::Start(Tag::Paragraph) => {
                finish_open_block(&mut block, &lines, &mut clauses, &mut headings);
                block = Some(VisibleBlock::new(VisibleBlockKind::Paragraph));
            }
            Event::Start(Tag::Heading { .. }) => {
                finish_open_block(&mut block, &lines, &mut clauses, &mut headings);
                block = Some(VisibleBlock::new(VisibleBlockKind::Heading));
            }
            Event::End(TagEnd::Paragraph) => {
                finish_matching_block(
                    &mut block,
                    VisibleBlockKind::Paragraph,
                    &lines,
                    &mut clauses,
                    &mut headings,
                );
            }
            Event::End(TagEnd::Heading(_)) => {
                finish_matching_block(
                    &mut block,
                    VisibleBlockKind::Heading,
                    &lines,
                    &mut clauses,
                    &mut headings,
                );
            }
            Event::Start(Tag::CodeBlock(_)) => {
                code_block_depth = code_block_depth.saturating_add(1);
            }
            Event::End(TagEnd::CodeBlock) => {
                code_block_depth = code_block_depth.saturating_sub(1);
            }
            Event::Start(Tag::Image { .. }) => {
                image_depth = image_depth.saturating_add(1);
            }
            Event::End(TagEnd::Image) => {
                image_depth = image_depth.saturating_sub(1);
            }
            Event::Text(value)
                if code_block_depth == 0
                    && image_depth == 0
                    && (value.chars().all(char::is_whitespace)
                        || range_has_visible_text(&masked, range.clone())) =>
            {
                if let Some(block) = block.as_mut() {
                    let direct_mapping = source.get(range.clone()) == Some(value.as_ref());
                    block.push_text(&value, range, direct_mapping);
                }
            }
            Event::Code(value) if code_block_depth == 0 && image_depth == 0 => {
                if let Some(block) = block.as_mut() {
                    block.push_text(&value, range, false);
                }
            }
            Event::SoftBreak | Event::HardBreak if code_block_depth == 0 && image_depth == 0 => {
                if let Some(block) = block.as_mut() {
                    block.push_text(" ", range, false);
                }
            }
            _ => {}
        }
    }
    finish_open_block(&mut block, &lines, &mut clauses, &mut headings);
    section_visible_document(source, &lines, clauses, headings)
}

fn finish_matching_block(
    block: &mut Option<VisibleBlock>,
    kind: VisibleBlockKind,
    lines: &SourceLineIndex,
    clauses: &mut Vec<VisibleClause>,
    headings: &mut Vec<VisibleHeading>,
) {
    if block.as_ref().is_some_and(|block| block.kind == kind) {
        finish_open_block(block, lines, clauses, headings);
    }
}

fn finish_open_block(
    block: &mut Option<VisibleBlock>,
    lines: &SourceLineIndex,
    clauses: &mut Vec<VisibleClause>,
    headings: &mut Vec<VisibleHeading>,
) {
    if let Some(block) = block.take() {
        block.finish(lines, clauses, headings);
    }
}

fn section_visible_document(
    source: &str,
    lines: &SourceLineIndex,
    mut clauses: Vec<VisibleClause>,
    headings: Vec<VisibleHeading>,
) -> VisibleDocument {
    let anchors = headings
        .into_iter()
        .filter_map(|heading| {
            explicit_section_language(&heading.text).map(|language| (heading, language))
        })
        .collect::<Vec<_>>();
    let mut drafts = Vec::<(usize, usize, Option<ReadmeSectionLanguage>)>::new();
    if anchors.is_empty() {
        if let (Some(first), Some(last)) = (clauses.first(), clauses.last()) {
            drafts.push((first.span.byte_start, last.span.byte_end, None));
        }
    } else {
        let first_start = anchors[0].0.span.byte_start;
        if let Some(first_clause) = clauses
            .iter()
            .find(|clause| clause.span.byte_start < first_start)
        {
            drafts.push((first_clause.span.byte_start, first_start, None));
        }
        for (index, (heading, language)) in anchors.iter().enumerate() {
            let end = anchors
                .get(index + 1)
                .map_or(source.len(), |(next, _)| next.span.byte_start);
            drafts.push((heading.span.byte_start, end, Some(*language)));
        }
    }

    let mut sections = Vec::new();
    for (start, end, declared) in drafts {
        if start >= end {
            continue;
        }
        let member_indices = clauses
            .iter()
            .enumerate()
            .filter_map(|(index, clause)| {
                (start <= clause.span.byte_start && clause.span.byte_start < end).then_some(index)
            })
            .collect::<Vec<_>>();
        if member_indices.is_empty() {
            continue;
        }
        if declared.is_none()
            && !member_indices.iter().any(|index| {
                let normalized = normalize_for_matching(&clauses[*index].text);
                !predicates_for(&normalized).is_empty()
            })
        {
            continue;
        }
        let language = classify_section_language(
            declared,
            member_indices.iter().map(|index| &clauses[*index]),
        );
        let section_index = sections.len();
        for index in member_indices {
            clauses[index].section = Some(section_index);
        }
        sections.push(VisibleSection {
            span: lines.span(start..end),
            language,
            claim_atoms: Vec::new(),
        });
    }

    let mut unknown_reasons = Vec::new();
    if sections.iter().any(|section| {
        matches!(
            section.language,
            ReadmeSectionLanguage::Ambiguous(UnknownReason::UnsupportedSyntax)
        )
    }) {
        push_unknown_reason(&mut unknown_reasons, UnknownReason::UnsupportedSyntax);
    }
    VisibleDocument {
        clauses,
        sections,
        unknown_reasons,
    }
}

fn explicit_section_language(text: &str) -> Option<ReadmeSectionLanguage> {
    let normalized = normalize_for_matching(text);
    let japanese = normalized.contains("日本語")
        || contains_ascii_word(&normalized, "japanese")
        || normalized == "ja";
    let english = contains_ascii_word(&normalized, "english") || normalized == "en";
    match (japanese, english) {
        (true, true) => Some(ReadmeSectionLanguage::Mixed),
        (true, false) => Some(ReadmeSectionLanguage::Japanese),
        (false, true) => Some(ReadmeSectionLanguage::English),
        (false, false) => None,
    }
}

fn classify_section_language<'a>(
    declared: Option<ReadmeSectionLanguage>,
    clauses: impl Iterator<Item = &'a VisibleClause>,
) -> ReadmeSectionLanguage {
    let mut japanese = false;
    let mut english = false;
    let mut mixed = false;
    for clause in clauses {
        let normalized = normalize_for_matching(&clause.text);
        if predicates_for(&normalized).is_empty() {
            continue;
        }
        match raw_clause_language(&clause.text) {
            ClauseLanguage::Japanese => japanese = true,
            ClauseLanguage::English => english = true,
            ClauseLanguage::Mixed => mixed = true,
            ClauseLanguage::Ambiguous => {}
        }
    }
    match declared {
        Some(ReadmeSectionLanguage::Japanese) if english || mixed => ReadmeSectionLanguage::Mixed,
        Some(ReadmeSectionLanguage::English) if japanese || mixed => ReadmeSectionLanguage::Mixed,
        Some(language) => language,
        None if mixed || (japanese && english) => ReadmeSectionLanguage::Mixed,
        None if japanese => ReadmeSectionLanguage::Japanese,
        None if english => ReadmeSectionLanguage::English,
        None => ReadmeSectionLanguage::Ambiguous(UnknownReason::UnsupportedSyntax),
    }
}

fn range_has_visible_text(masked: &str, range: Range<usize>) -> bool {
    masked
        .get(range)
        .is_some_and(|value| value.chars().any(|character| !character.is_whitespace()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct ClauseBoundary {
    left_end: usize,
    right_start: usize,
}

fn clause_ranges(text: &str) -> Vec<Range<usize>> {
    let mut boundaries = Vec::new();
    for (index, character) in text.char_indices() {
        if matches!(character, '.' | '!' | '?' | ';' | '。' | '！' | '？' | '；') {
            let end = index + character.len_utf8();
            boundaries.push(ClauseBoundary {
                left_end: end,
                right_start: end,
            });
        }
    }
    for marker in ["but", "however", "yet", "while"] {
        push_ascii_word_boundaries(text, marker, &mut boundaries);
    }
    for marker in ["しかし", "ただし", "一方で", "だが", "ですが"] {
        for (start, _) in text.match_indices(marker) {
            boundaries.push(ClauseBoundary {
                left_end: start,
                right_start: start + marker.len(),
            });
        }
    }
    boundaries.sort_unstable();
    boundaries.dedup();

    let mut ranges = Vec::new();
    let mut start = 0usize;
    for boundary in boundaries {
        if boundary.left_end < start || boundary.right_start > text.len() {
            continue;
        }
        if let Some(range) = trim_clause_range(text, start..boundary.left_end) {
            ranges.push(range);
        }
        start = start.max(boundary.right_start);
    }
    if let Some(range) = trim_clause_range(text, start..text.len()) {
        ranges.push(range);
    }
    ranges
}

fn push_ascii_word_boundaries(text: &str, marker: &str, boundaries: &mut Vec<ClauseBoundary>) {
    let lower = text.to_ascii_lowercase();
    for (start, _) in lower.match_indices(marker) {
        let end = start + marker.len();
        let left_is_boundary = lower[..start]
            .chars()
            .next_back()
            .is_none_or(|character| !character.is_ascii_alphanumeric());
        let right_is_boundary = lower[end..]
            .chars()
            .next()
            .is_none_or(|character| !character.is_ascii_alphanumeric());
        if start > 0 && end < text.len() && left_is_boundary && right_is_boundary {
            boundaries.push(ClauseBoundary {
                left_end: start,
                right_start: end,
            });
        }
    }
}

fn trim_clause_range(text: &str, range: Range<usize>) -> Option<Range<usize>> {
    let value = text.get(range.clone())?;
    let trimmed_start = value.len() - value.trim_start_matches(clause_edge).len();
    let trimmed_end = value.trim_end_matches(clause_edge).len();
    let range = range.start + trimmed_start..range.start + trimmed_end;
    (range.start < range.end).then_some(range)
}

fn clause_edge(character: char) -> bool {
    character.is_whitespace() || matches!(character, ',' | ':' | ';' | '、' | '，' | '：' | '；')
}

#[derive(Debug, Default)]
struct ClaimTerms {
    subject: Option<String>,
    action: Option<String>,
    object: Option<String>,
    qualifiers: Vec<String>,
    condition: Option<String>,
}

fn claim_terms(predicate: GrammarPredicate, text: &str, language: DocumentLanguage) -> ClaimTerms {
    match language {
        DocumentLanguage::Japanese => japanese_claim_terms(predicate, text),
        DocumentLanguage::English => english_claim_terms(predicate, text),
    }
}

fn english_claim_terms(predicate: GrammarPredicate, text: &str) -> ClaimTerms {
    let tokens = text.split_whitespace().collect::<Vec<_>>();
    let action = action_candidates(predicate)
        .iter()
        .find_map(|(stem, canonical)| {
            tokens
                .iter()
                .position(|token| token_matches_stem(token, stem))
                .map(|index| (index, *canonical))
        });
    let subject = action.and_then(|(index, _)| {
        tokens[..index]
            .iter()
            .copied()
            .find(|token| is_meaningful_term(token))
            .map(str::to_string)
    });
    let object =
        action.and_then(|(index, canonical)| english_object(text, &tokens, index, canonical));
    let mut qualifiers = tokens
        .iter()
        .copied()
        .filter(|token| is_qualifier(token))
        .map(canonical_qualifier)
        .map(str::to_string)
        .collect::<Vec<_>>();
    if contains_ascii_word(text, "evidence") && contains_ascii_word(text, "with") {
        qualifiers.push("with_evidence".to_string());
    }
    if contains_ascii_word(text, "offline") {
        qualifiers.push("offline".to_string());
    }
    if contains_ascii_word(text, "automatically") {
        qualifiers.push("automatically".to_string());
    }
    qualifiers.sort_unstable();
    qualifiers.dedup();
    ClaimTerms {
        subject,
        action: action.map(|(_, value)| value.to_string()),
        object,
        qualifiers,
        condition: condition_from(&tokens),
    }
}

fn japanese_claim_terms(predicate: GrammarPredicate, text: &str) -> ClaimTerms {
    let action = japanese_action_candidates(predicate)
        .iter()
        .filter_map(|(surface, canonical)| text.find(surface).map(|index| (index, *canonical)))
        .min_by_key(|(index, _)| *index);
    let mut qualifiers = Vec::new();
    for (surface, canonical) in [
        ("ローカル", "locally"),
        ("手元", "locally"),
        ("局所", "locally"),
        ("のみ", "only"),
        ("だけ", "only"),
        ("上限", "bounded"),
        ("制限", "bounded"),
        ("検証済み", "verified"),
        ("オフライン", "offline"),
        ("根拠付き", "with_evidence"),
        ("証拠付き", "with_evidence"),
    ] {
        if text.contains(surface) {
            qualifiers.push(canonical.to_string());
        }
    }
    qualifiers.sort_unstable();
    qualifiers.dedup();
    ClaimTerms {
        subject: action.and_then(|(index, _)| japanese_subject(&text[..index])),
        action: action.map(|(_, canonical)| canonical.to_string()),
        object: action.and_then(|(index, canonical)| japanese_object(text, index, canonical)),
        qualifiers,
        condition: japanese_condition(text),
    }
}

fn japanese_action_candidates(
    predicate: GrammarPredicate,
) -> &'static [(&'static str, &'static str)] {
    match predicate {
        GrammarPredicate::DefinesIdentity => &[("です", "be")],
        GrammarPredicate::TargetsAudience => &[("向け", "target")],
        GrammarPredicate::StatesProblem => {
            &[("防ぐ", "prevent"), ("減ら", "reduce"), ("避け", "avoid")]
        }
        GrammarPredicate::PerformsOperation => &[
            ("監査", "audit"),
            ("解析", "analyze"),
            ("検出", "detect"),
            ("生成", "generate"),
            ("整理", "organize"),
            ("調べ", "inspect"),
            ("確認", "inspect"),
            ("書き込", "write"),
            ("使用", "use"),
        ],
        GrammarPredicate::ProducesOutcome => &[
            ("提示", "present"),
            ("出力", "output"),
            ("返", "return"),
            ("生成", "produce"),
            ("得ら", "produce"),
        ],
        GrammarPredicate::DescribesFirstAction | GrammarPredicate::RequestsAction => &[
            ("インストール", "install"),
            ("実行", "run"),
            ("使用", "use"),
            ("始め", "start"),
            ("試", "try"),
            ("参照", "see"),
        ],
        GrammarPredicate::DescribesFirstResult => &[
            ("表示", "print"),
            ("返", "return"),
            ("出力", "output"),
            ("得ら", "see"),
        ],
        GrammarPredicate::ProvidesEvidence => &[
            ("テスト", "test"),
            ("ベンチマーク", "benchmark"),
            ("実例", "example"),
            ("検証", "verify"),
        ],
        GrammarPredicate::StatesConstraint => &[
            ("使用", "use"),
            ("書き込", "write"),
            ("監査", "audit"),
            ("解析", "analyze"),
            ("検出", "detect"),
            ("生成", "generate"),
            ("整理", "organize"),
            ("調べ", "inspect"),
            ("確認", "inspect"),
            ("必要", "require"),
            ("制限", "limit"),
            ("上限", "limit"),
        ],
        GrammarPredicate::StatesDifferentiation => {
            &[("異な", "differentiate"), ("なしで", "differentiate")]
        }
    }
}

fn japanese_subject(prefix: &str) -> Option<String> {
    let marker = prefix.rfind('は').or_else(|| prefix.rfind('が'))?;
    canonical_semantic_term(&prefix[..marker])
}

fn japanese_object(text: &str, action_index: usize, action: &str) -> Option<String> {
    if action == "write" && text.contains("リポジトリ") && text.contains("ファイル") {
        return Some("repository changes".to_string());
    }
    let after_action = &text[action_index..];
    if action == "audit" && after_action.contains("監査結果") {
        return Some("results".to_string());
    }
    let prefix = &text[..action_index];
    let marker = prefix.rfind('を')?;
    let before = &prefix[..marker];
    let start = before
        .char_indices()
        .rev()
        .find_map(|(index, character)| {
            matches!(character, 'は' | 'が' | 'へ' | 'に' | 'で' | 'と' | '、')
                .then_some(index + character.len_utf8())
        })
        .unwrap_or(0);
    canonical_semantic_term(&before[start..])
}

fn english_object(
    text: &str,
    tokens: &[&str],
    action_index: usize,
    action: &str,
) -> Option<String> {
    let values = tokens[action_index + 1..]
        .iter()
        .copied()
        .filter(|token| is_meaningful_term(token) && !is_qualifier(token))
        .collect::<Vec<_>>();
    if action == "present" && values.starts_with(&["audit", "results"]) {
        return Some("audit results".to_string());
    }
    let first = values.first().copied()?;
    if action == "write"
        && contains_ascii_word(text, "repository")
        && matches!(first, "fixes" | "files" | "changes")
    {
        return Some("repository changes".to_string());
    }
    Some(match first {
        "repository" | "repositories" => "repositories".to_string(),
        other => other.to_string(),
    })
}

fn canonical_semantic_term(value: &str) -> Option<String> {
    let trimmed = value
        .trim()
        .trim_matches(|character: char| !character.is_alphanumeric());
    if trimmed.is_empty() {
        return None;
    }
    let canonical = match trimmed {
        "リポジトリ" => "repositories",
        "ネットワーク" => "network",
        "ファイル" => "files",
        "修正" | "修正内容" => "fixes",
        "監査結果" => "audit results",
        "結果" => "results",
        "証拠" | "根拠" => "evidence",
        other => other,
    };
    Some(canonical.to_lowercase())
}

fn japanese_condition(text: &str) -> Option<String> {
    let marker = ["場合", "とき", "なら", "に限り"]
        .into_iter()
        .filter_map(|marker| text.find(marker).map(|index| (index, marker)))
        .min_by_key(|(index, _)| *index)?;
    let prefix = text[..marker.0].trim();
    let value = prefix
        .rsplit(['。', '；', ';', '、'])
        .next()
        .unwrap_or(prefix)
        .trim();
    if value.contains("オフライン") {
        Some("offline".to_string())
    } else if value.contains("ローカル") || value.contains("手元") {
        Some("locally".to_string())
    } else if value.to_ascii_lowercase().contains("ci") {
        Some("ci".to_string())
    } else {
        (!value.is_empty()).then(|| value.to_string())
    }
}

fn action_candidates(predicate: GrammarPredicate) -> &'static [(&'static str, &'static str)] {
    match predicate {
        GrammarPredicate::DefinesIdentity => &[("is", "be"), ("are", "be")],
        GrammarPredicate::TargetsAudience => &[("for", "target"), ("designed", "target")],
        GrammarPredicate::StatesProblem => &[
            ("avoid", "avoid"),
            ("reduce", "reduce"),
            ("prevent", "prevent"),
        ],
        GrammarPredicate::PerformsOperation => &[
            ("analy", "analyze"),
            ("scan", "scan"),
            ("generat", "generate"),
            ("detect", "detect"),
            ("organiz", "organize"),
            ("audit", "audit"),
            ("inspect", "inspect"),
            ("review", "review"),
            ("write", "write"),
        ],
        GrammarPredicate::ProducesOutcome => &[
            ("allow", "allow"),
            ("enable", "enable"),
            ("return", "return"),
            ("produc", "produce"),
            ("output", "output"),
            ("present", "present"),
        ],
        GrammarPredicate::DescribesFirstAction | GrammarPredicate::RequestsAction => &[
            ("install", "install"),
            ("run", "run"),
            ("use", "use"),
            ("start", "start"),
            ("try", "try"),
            ("see", "see"),
        ],
        GrammarPredicate::DescribesFirstResult => &[
            ("print", "print"),
            ("return", "return"),
            ("see", "see"),
            ("output", "output"),
        ],
        GrammarPredicate::ProvidesEvidence => &[
            ("test", "test"),
            ("benchmark", "benchmark"),
            ("example", "example"),
            ("verif", "verify"),
        ],
        GrammarPredicate::StatesConstraint => &[
            ("use", "use"),
            ("write", "write"),
            ("analy", "analyze"),
            ("scan", "scan"),
            ("generat", "generate"),
            ("detect", "detect"),
            ("organiz", "organize"),
            ("audit", "audit"),
            ("inspect", "inspect"),
            ("review", "review"),
            ("requir", "require"),
            ("limit", "limit"),
        ],
        GrammarPredicate::StatesDifferentiation => &[
            ("unlike", "differentiate"),
            ("instead", "differentiate"),
            ("without", "differentiate"),
        ],
    }
}

fn token_matches_stem(token: &str, stem: &str) -> bool {
    if stem.len() <= 3 {
        token == stem
    } else {
        token.starts_with(stem)
    }
}

fn is_meaningful_term(token: &str) -> bool {
    !matches!(
        token,
        "a" | "an"
            | "and"
            | "are"
            | "as"
            | "be"
            | "by"
            | "do"
            | "does"
            | "for"
            | "from"
            | "in"
            | "is"
            | "it"
            | "not"
            | "of"
            | "on"
            | "or"
            | "that"
            | "the"
            | "this"
            | "to"
            | "with"
            | "without"
    )
}

fn is_qualifier(token: &str) -> bool {
    matches!(
        token,
        "bounded"
            | "limited"
            | "local"
            | "locally"
            | "only"
            | "possible"
            | "required"
            | "verified"
            | "without"
    )
}

fn canonical_qualifier(token: &str) -> &str {
    match token {
        "limited" => "bounded",
        "local" => "locally",
        other => other,
    }
}

fn condition_from(tokens: &[&str]) -> Option<String> {
    let marker = tokens
        .iter()
        .position(|token| matches!(*token, "if" | "when" | "under"))?;
    let condition = tokens[marker + 1..]
        .iter()
        .copied()
        .filter(|token| is_meaningful_term(token))
        .take(8)
        .collect::<Vec<_>>()
        .join(" ");
    (!condition.is_empty()).then_some(condition)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClauseLanguage {
    Japanese,
    English,
    Mixed,
    Ambiguous,
}

fn confident_clause_language(
    text: &str,
    section: Option<ReadmeSectionLanguage>,
) -> Option<DocumentLanguage> {
    match raw_clause_language(text) {
        ClauseLanguage::Japanese => Some(DocumentLanguage::Japanese),
        ClauseLanguage::English => Some(DocumentLanguage::English),
        ClauseLanguage::Mixed => None,
        ClauseLanguage::Ambiguous => match section {
            Some(ReadmeSectionLanguage::Japanese) => Some(DocumentLanguage::Japanese),
            Some(ReadmeSectionLanguage::English) => Some(DocumentLanguage::English),
            Some(ReadmeSectionLanguage::Mixed | ReadmeSectionLanguage::Ambiguous(_)) | None => None,
        },
    }
}

fn raw_clause_language(text: &str) -> ClauseLanguage {
    let has_kana = text
        .chars()
        .any(|character| matches!(character, '\u{3040}'..='\u{30ff}'));
    let has_han = text
        .chars()
        .any(|character| matches!(character, '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}'));
    let english = text
        .split(|character: char| !character.is_ascii_alphabetic())
        .filter(|word| !word.is_empty())
        .map(str::to_ascii_lowercase)
        .any(|word| {
            matches!(
                word.as_str(),
                "a" | "an"
                    | "and"
                    | "are"
                    | "audit"
                    | "audits"
                    | "benchmark"
                    | "does"
                    | "english"
                    | "evidence"
                    | "for"
                    | "from"
                    | "generates"
                    | "is"
                    | "mixed"
                    | "network"
                    | "not"
                    | "only"
                    | "output"
                    | "presents"
                    | "repository"
                    | "repositories"
                    | "result"
                    | "results"
                    | "returns"
                    | "tool"
                    | "use"
                    | "uses"
                    | "with"
                    | "without"
                    | "write"
                    | "writes"
            )
        });
    match (has_kana, has_han, english) {
        (true, _, true) => ClauseLanguage::Mixed,
        (true, _, false) => ClauseLanguage::Japanese,
        (false, true, true) => ClauseLanguage::Mixed,
        (false, true, false) => ClauseLanguage::Ambiguous,
        (false, false, true) => ClauseLanguage::English,
        (false, false, false) => ClauseLanguage::Ambiguous,
    }
}

fn contains_ascii_word(text: &str, expected: &str) -> bool {
    text.split(|character: char| !character.is_ascii_alphabetic())
        .any(|word| word.eq_ignore_ascii_case(expected))
}

fn push_unknown_reason(reasons: &mut Vec<UnknownReason>, reason: UnknownReason) {
    if !reasons.contains(&reason) {
        reasons.push(reason);
    }
}

fn normalize_for_matching(text: &str) -> String {
    text.chars()
        .flat_map(char::to_lowercase)
        .map(|character| {
            if character.is_alphanumeric() || !character.is_ascii() {
                character
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn predicates_for(text: &str) -> Vec<GrammarPredicate> {
    let mut predicates = Vec::new();
    push_if(
        &mut predicates,
        GrammarPredicate::DefinesIdentity,
        contains_any(
            text,
            &[
                " is a ",
                " is an ",
                " tool",
                " cli",
                " plugin",
                "ライブラリ",
                "ツール",
                "プラグイン",
                "です",
            ],
        ),
    );
    push_if(
        &mut predicates,
        GrammarPredicate::TargetsAudience,
        contains_any(
            text,
            &[
                " for developers",
                " for teams",
                " for maintainers",
                "designed for",
                "向け",
                "開発者",
                "利用者",
                "メンテナ",
                "チーム",
            ],
        ),
    );
    push_if(
        &mut predicates,
        GrammarPredicate::StatesProblem,
        contains_any(
            text,
            &[
                "problem", "pain", "avoid", "reduce", "prevent", "課題", "問題", "防ぐ", "減ら",
                "迷わ",
            ],
        ),
    );
    push_if(
        &mut predicates,
        GrammarPredicate::PerformsOperation,
        contains_any(
            text,
            &[
                "analy",
                "scan",
                "generate",
                "detect",
                "organize",
                "audit",
                "inspect",
                "review",
                "write",
                "解析",
                "検出",
                "生成",
                "整理",
                "監査",
                "調べ",
                "確認",
                "書き込",
                "使用",
            ],
        ),
    );
    push_if(
        &mut predicates,
        GrammarPredicate::ProducesOutcome,
        contains_any(
            text,
            &[
                "so that",
                "allows",
                "enables",
                "result",
                "output",
                "returns",
                "produces",
                "present",
                "できる",
                "できます",
                "得られ",
                "結果",
                "出力",
                "提示",
            ],
        ),
    );
    push_if(
        &mut predicates,
        GrammarPredicate::DescribesFirstAction,
        contains_any(
            text,
            &[
                "quickstart",
                "quick start",
                "getting started",
                "install",
                "run ",
                "use ",
                "first step",
                "クイックスタート",
                "開始",
                "インストール",
                "実行",
                "使い方",
            ],
        ),
    );
    push_if(
        &mut predicates,
        GrammarPredicate::DescribesFirstResult,
        contains_any(
            text,
            &[
                "you should see",
                "prints",
                "returns",
                "first result",
                "expected output",
                "表示され",
                "得られ",
                "期待する出力",
                "実行結果",
            ],
        ),
    );
    push_if(
        &mut predicates,
        GrammarPredicate::ProvidesEvidence,
        contains_any(
            text,
            &[
                "test",
                " ci ",
                "benchmark",
                "example",
                "verified",
                "テスト",
                "検証",
                "実例",
                "ベンチマーク",
            ],
        ),
    );
    push_if(
        &mut predicates,
        GrammarPredicate::StatesConstraint,
        has_explicit_negative_cue(text)
            || contains_any(
                text,
                &[
                    "only ", "limited", "requires", "のみ", "制約", "上限", "必要",
                ],
            ),
    );
    push_if(
        &mut predicates,
        GrammarPredicate::StatesDifferentiation,
        contains_any(
            text,
            &[
                "unlike",
                "instead of",
                "single session",
                "without ",
                "一つの",
                "単一",
                "一度に",
                "なしで",
                "異な",
            ],
        ),
    );
    push_if(
        &mut predicates,
        GrammarPredicate::RequestsAction,
        contains_any(
            text,
            &[
                "try ",
                "start ",
                "run ",
                "see ",
                "試す",
                "始め",
                "実行して",
                "参照して",
            ],
        ),
    );
    predicates.sort_unstable();
    predicates.dedup();
    predicates
}

fn push_if(values: &mut Vec<GrammarPredicate>, value: GrammarPredicate, condition: bool) {
    if condition {
        values.push(value);
    }
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    let padded = format!(" {value} ");
    needles.iter().any(|needle| padded.contains(needle))
}

fn modality_of(text: &str, negated: bool) -> seiri_core::ClaimModality {
    if negated {
        seiri_core::ClaimModality::Prohibited
    } else if contains_any(text, &["must", "requires", "required", "必要", "必須"]) {
        seiri_core::ClaimModality::Required
    } else if contains_any(text, &["may", "might", "possible", "可能", "場合があり"]) {
        seiri_core::ClaimModality::Possible
    } else if contains_any(
        text,
        &[
            "only", "within", "bounded", "limited", "のみ", "範囲", "上限",
        ],
    ) {
        seiri_core::ClaimModality::Qualified
    } else {
        seiri_core::ClaimModality::Asserted
    }
}

fn build_translation_alignment(
    document: &DocumentScan,
    mut visible: VisibleDocument,
    claim_atoms: &ClaimAtomIR,
) -> Result<ReadmeTranslationAlignmentIR, seiri_core::AppealIrError> {
    let sections = visible
        .sections
        .into_iter()
        .enumerate()
        .map(|(index, section)| ReadmeSection {
            id: ReadmeSectionId::new(
                NonZeroU32::new((index + 1) as u32).expect("README section count is bounded"),
            ),
            language: section.language,
            span: section.span,
            claim_atoms: section.claim_atoms,
        })
        .collect::<Vec<_>>();
    let japanese = sections
        .iter()
        .filter(|section| section.language == ReadmeSectionLanguage::Japanese)
        .collect::<Vec<_>>();
    let english = sections
        .iter()
        .filter(|section| section.language == ReadmeSectionLanguage::English)
        .collect::<Vec<_>>();
    let section_pairs = pair_translation_sections(
        &japanese,
        &english,
        claim_atoms,
        &mut visible.unknown_reasons,
    );
    let mut alignments = section_pairs
        .into_iter()
        .map(|(source, counterpart)| {
            align_translation_sections(
                source,
                counterpart,
                claim_atoms,
                &mut visible.unknown_reasons,
            )
        })
        .collect::<Vec<_>>();
    alignments.sort_unstable_by_key(|alignment| {
        (alignment.source_section, alignment.counterpart_section)
    });

    ReadmeTranslationAlignmentIR::try_new(
        document.path(),
        document.base().digest(),
        document.source_bytes(),
        sections,
        alignments,
        visible.unknown_reasons,
    )
}

fn pair_translation_sections<'a>(
    japanese: &[&'a ReadmeSection],
    english: &[&'a ReadmeSection],
    claim_atoms: &ClaimAtomIR,
    unknown_reasons: &mut Vec<UnknownReason>,
) -> Vec<(&'a ReadmeSection, &'a ReadmeSection)> {
    if japanese.is_empty() || english.is_empty() {
        return Vec::new();
    }
    let singleton_pair = japanese.len() == 1 && english.len() == 1;
    let mut candidates = japanese
        .iter()
        .flat_map(|source| {
            english.iter().filter_map(move |counterpart| {
                let overlap = section_semantic_overlap(source, counterpart, claim_atoms);
                (overlap > 0 || singleton_pair).then_some((overlap, *source, *counterpart))
            })
        })
        .collect::<Vec<_>>();
    candidates.sort_unstable_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then_with(|| left.1.id.cmp(&right.1.id))
            .then_with(|| left.2.id.cmp(&right.2.id))
    });

    let mut used_japanese = BTreeSet::new();
    let mut used_english = BTreeSet::new();
    let mut pairs = Vec::new();
    for (_, source, counterpart) in candidates {
        if used_japanese.contains(&source.id) || used_english.contains(&counterpart.id) {
            continue;
        }
        used_japanese.insert(source.id);
        used_english.insert(counterpart.id);
        pairs.push((source, counterpart));
    }
    if used_japanese.len() != japanese.len() || used_english.len() != english.len() {
        push_unknown_reason(unknown_reasons, UnknownReason::Unavailable);
    }
    pairs.sort_unstable_by_key(|(source, counterpart)| (source.id, counterpart.id));
    pairs
}

fn section_semantic_overlap(
    source: &ReadmeSection,
    counterpart: &ReadmeSection,
    claim_atoms: &ClaimAtomIR,
) -> usize {
    let source = source
        .claim_atoms
        .iter()
        .filter_map(|id| claim_atom(claim_atoms, *id))
        .filter_map(semantic_identity)
        .collect::<BTreeSet<_>>();
    let counterpart = counterpart
        .claim_atoms
        .iter()
        .filter_map(|id| claim_atom(claim_atoms, *id))
        .filter_map(semantic_identity)
        .collect::<BTreeSet<_>>();
    source.intersection(&counterpart).count()
}

fn align_translation_sections(
    source: &ReadmeSection,
    counterpart: &ReadmeSection,
    claim_atoms: &ClaimAtomIR,
    unknown_reasons: &mut Vec<UnknownReason>,
) -> ReadmeSectionAlignment {
    let mut used_counterparts = BTreeSet::new();
    let mut claims = Vec::new();
    for source_id in &source.claim_atoms {
        let Some(source_atom) = claim_atom(claim_atoms, *source_id) else {
            continue;
        };
        let counterpart_atom = counterpart
            .claim_atoms
            .iter()
            .filter(|id| !used_counterparts.contains(*id))
            .filter_map(|id| {
                let atom = claim_atom(claim_atoms, *id)?;
                semantic_identity_matches(source_atom, atom).then(|| {
                    let divergences = translation_divergences(source_atom, atom);
                    (*id, atom, divergences)
                })
            })
            .min_by_key(|(id, _, divergences)| (divergences.len(), *id));
        let Some((counterpart_id, _counterpart_atom, divergences)) = counterpart_atom else {
            claims.push(unaligned_translation_claim(
                *source_id,
                source_atom,
                unknown_reasons,
            ));
            continue;
        };
        used_counterparts.insert(counterpart_id);
        let state = if divergences.is_empty() {
            TranslationAlignmentState::Aligned
        } else {
            TranslationAlignmentState::Divergent
        };
        claims.push(TranslationClaimAlignment {
            source_claim: *source_id,
            counterpart_claim: Some(counterpart_id),
            state,
            divergences,
        });
    }
    for counterpart_id in &counterpart.claim_atoms {
        if used_counterparts.contains(counterpart_id) {
            continue;
        }
        let Some(counterpart_atom) = claim_atom(claim_atoms, *counterpart_id) else {
            continue;
        };
        claims.push(unaligned_translation_claim(
            *counterpart_id,
            counterpart_atom,
            unknown_reasons,
        ));
    }
    claims.sort_unstable_by_key(|claim| (claim.source_claim, claim.counterpart_claim));
    let state = if claims.is_empty()
        || claims
            .iter()
            .any(|claim| matches!(claim.state, TranslationAlignmentState::Unknown(_)))
    {
        push_unknown_reason(unknown_reasons, UnknownReason::UnsupportedSyntax);
        TranslationAlignmentState::Unknown(UnknownReason::UnsupportedSyntax)
    } else if claims
        .iter()
        .all(|claim| claim.state == TranslationAlignmentState::Aligned)
    {
        TranslationAlignmentState::Aligned
    } else {
        TranslationAlignmentState::Divergent
    };
    ReadmeSectionAlignment {
        source_section: source.id,
        counterpart_section: counterpart.id,
        state,
        claims,
    }
}

fn claim_atom(claim_atoms: &ClaimAtomIR, id: ClaimAtomId) -> Option<&ClaimAtom> {
    claim_atoms.atoms.iter().find(|atom| atom.id == id)
}

type SemanticIdentity<'atom> = (
    seiri_core::ValueDimension,
    Option<&'atom str>,
    Option<&'atom str>,
    Option<&'atom str>,
);

fn semantic_identity(atom: &ClaimAtom) -> Option<SemanticIdentity<'_>> {
    (atom.subject.is_some() || atom.action.is_some() || atom.object.is_some()).then_some((
        atom.dimension,
        atom.subject.as_deref(),
        atom.action.as_deref(),
        atom.object.as_deref(),
    ))
}

fn semantic_identity_matches(source: &ClaimAtom, counterpart: &ClaimAtom) -> bool {
    semantic_identity(source)
        .is_some_and(|identity| semantic_identity(counterpart) == Some(identity))
}

fn translation_divergences(
    source: &ClaimAtom,
    counterpart: &ClaimAtom,
) -> Vec<TranslationDivergenceKind> {
    let mut divergences = Vec::new();
    if source.polarity != counterpart.polarity {
        divergences.push(TranslationDivergenceKind::Polarity);
    }
    if source.qualifiers != counterpart.qualifiers {
        divergences.push(TranslationDivergenceKind::Qualifier);
    }
    if source.condition != counterpart.condition {
        divergences.push(TranslationDivergenceKind::Condition);
    }
    divergences.sort_unstable();
    divergences
}

fn unaligned_translation_claim(
    source_claim: ClaimAtomId,
    source_atom: &ClaimAtom,
    unknown_reasons: &mut Vec<UnknownReason>,
) -> TranslationClaimAlignment {
    if semantic_identity(source_atom).is_none() {
        push_unknown_reason(unknown_reasons, UnknownReason::UnsupportedSyntax);
        TranslationClaimAlignment {
            source_claim,
            counterpart_claim: None,
            state: TranslationAlignmentState::Unknown(UnknownReason::UnsupportedSyntax),
            divergences: Vec::new(),
        }
    } else {
        TranslationClaimAlignment {
            source_claim,
            counterpart_claim: None,
            state: TranslationAlignmentState::Divergent,
            divergences: vec![TranslationDivergenceKind::MissingCounterpart],
        }
    }
}

fn build_edges(
    nodes: &[GrammarNode],
    claim_atoms: &ClaimAtomIR,
    translation_alignment: Option<&ReadmeTranslationAlignmentIR>,
    max_edges: usize,
    diagnostics: &mut Vec<GrammarDiagnostic>,
) -> Vec<GrammarEdge> {
    let mut candidates = Vec::new();
    for pair in nodes.windows(2) {
        candidates.push(GrammarEdge {
            from: pair[0].id,
            to: pair[1].id,
            relation: NarrativeRelation::Precedes,
        });
    }
    for (index, node) in nodes.iter().enumerate() {
        if node.predicate == GrammarPredicate::ProvidesEvidence {
            if let Some(subject) = nodes[..index].iter().rev().find(|candidate| {
                matches!(
                    candidate.predicate,
                    GrammarPredicate::PerformsOperation | GrammarPredicate::ProducesOutcome
                )
            }) {
                candidates.push(GrammarEdge {
                    from: node.id,
                    to: subject.id,
                    relation: NarrativeRelation::Supports,
                });
            }
        }
        if node.predicate == GrammarPredicate::StatesConstraint {
            if let Some(subject) = nodes[..index].iter().rev().find(|candidate| {
                matches!(
                    candidate.predicate,
                    GrammarPredicate::PerformsOperation | GrammarPredicate::ProducesOutcome
                )
            }) {
                candidates.push(GrammarEdge {
                    from: node.id,
                    to: subject.id,
                    relation: NarrativeRelation::Qualifies,
                });
            }
        }
        if node.predicate == GrammarPredicate::StatesProblem {
            if let Some(capability) = nodes[index + 1..]
                .iter()
                .find(|candidate| candidate.predicate == GrammarPredicate::PerformsOperation)
            {
                candidates.push(GrammarEdge {
                    from: node.id,
                    to: capability.id,
                    relation: NarrativeRelation::Explains,
                });
            }
        }
        if translation_alignment.is_none() {
            if let Some(translation) = nodes[index + 1..].iter().find(|candidate| {
                candidate.predicate == node.predicate && candidate.language != node.language
            }) {
                candidates.push(GrammarEdge {
                    from: node.id,
                    to: translation.id,
                    relation: NarrativeRelation::Translates,
                });
            }
        }
    }
    if let Some(translation_alignment) = translation_alignment {
        for alignment in &translation_alignment.alignments {
            for claim in &alignment.claims {
                if claim.state != TranslationAlignmentState::Aligned {
                    continue;
                }
                let Some(counterpart) = claim.counterpart_claim else {
                    continue;
                };
                let Some(source_atom) = claim_atom(claim_atoms, claim.source_claim) else {
                    continue;
                };
                let Some(counterpart_atom) = claim_atom(claim_atoms, counterpart) else {
                    continue;
                };
                candidates.push(GrammarEdge {
                    from: source_atom.grammar_node,
                    to: counterpart_atom.grammar_node,
                    relation: NarrativeRelation::Translates,
                });
            }
        }
    }
    candidates.sort_unstable_by_key(|edge| (edge.from, edge.to, relation_rank(edge.relation)));
    candidates.dedup_by_key(|edge| (edge.from, edge.to, relation_rank(edge.relation)));
    if candidates.len() > max_edges {
        candidates.truncate(max_edges);
        diagnostics.push(GrammarDiagnostic {
            kind: GrammarDiagnosticKind::NodeLimitExceeded,
            span: None,
        });
    }
    candidates
}

const fn grammar_diagnostic_rank(kind: GrammarDiagnosticKind) -> u8 {
    match kind {
        GrammarDiagnosticKind::SourceLimitExceeded => 0,
        GrammarDiagnosticKind::NodeLimitExceeded => 1,
        GrammarDiagnosticKind::AmbiguousLanguage => 2,
        GrammarDiagnosticKind::UnsupportedConstruct => 3,
        GrammarDiagnosticKind::ConflictingClause => 4,
    }
}

const fn relation_rank(relation: NarrativeRelation) -> u8 {
    match relation {
        NarrativeRelation::Precedes => 0,
        NarrativeRelation::Supports => 1,
        NarrativeRelation::Explains => 2,
        NarrativeRelation::Qualifies => 3,
        NarrativeRelation::Translates => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan_document;

    #[test]
    fn grammar_uses_visible_prose_and_excludes_code_fences() {
        let scan = scan_document(
            "README.md",
            "# Tool\nA tool for maintainers that scans repositories and produces a report.\n```text\nThis benchmark proves quality.\n```\n",
        )
        .expect("scan");
        let grammar =
            analyze_readme_grammar(&scan, &ReadmeGrammarOptions::default()).expect("grammar");
        assert!(grammar
            .nodes
            .iter()
            .any(|node| node.predicate == GrammarPredicate::TargetsAudience));
        assert!(grammar
            .nodes
            .iter()
            .any(|node| node.predicate == GrammarPredicate::PerformsOperation));
        assert!(!grammar
            .nodes
            .iter()
            .any(|node| node.predicate == GrammarPredicate::ProvidesEvidence));
    }

    #[test]
    fn grammar_preserves_japanese_constraint_and_negation() {
        let scan = scan_document(
            "README.md",
            "# ツール\nこのツールはリポジトリを解析します。ネットワーク通信はしません。\n",
        )
        .expect("scan");
        let grammar =
            analyze_readme_grammar(&scan, &ReadmeGrammarOptions::default()).expect("grammar");
        let constraint = grammar
            .nodes
            .iter()
            .find(|node| node.predicate == GrammarPredicate::StatesConstraint)
            .expect("constraint");
        assert_eq!(constraint.language, DocumentLanguage::Japanese);
        assert!(constraint.negated);
        assert_eq!(constraint.modality, seiri_core::ClaimModality::Prohibited);
    }

    #[test]
    fn grammar_reports_node_budget_without_collapsing_to_absence() {
        let scan = scan_document(
            "README.md",
            "A tool for teams that scans repositories and produces output.",
        )
        .expect("scan");
        let grammar = analyze_readme_grammar(
            &scan,
            &ReadmeGrammarOptions {
                max_nodes: 1,
                max_edges: 1,
            },
        )
        .expect("grammar");
        assert_eq!(
            grammar.coverage,
            CoverageStatus::Partial(CoverageIncompleteReason::LimitExceeded)
        );
        assert_eq!(grammar.nodes.len(), 1);
    }

    #[test]
    fn source_grammar_scopes_negation_to_the_independent_clause() {
        let source = "# Tool\nDoes not use the network; audits repositories locally.\n";
        let scan = scan_document("README.md", source).expect("scan");
        let grammar =
            analyze_readme_grammar_with_source(&scan, source, &ReadmeGrammarOptions::default())
                .expect("source grammar");

        let constraint = grammar
            .nodes
            .iter()
            .find(|node| node.predicate == GrammarPredicate::StatesConstraint)
            .expect("constraint");
        let operation = grammar
            .nodes
            .iter()
            .find(|node| node.predicate == GrammarPredicate::PerformsOperation)
            .expect("operation");
        assert!(constraint.negated);
        assert_eq!(constraint.modality, seiri_core::ClaimModality::Prohibited);
        assert!(!operation.negated);
        assert_eq!(operation.modality, seiri_core::ClaimModality::Asserted);
        assert!(
            constraint.span.expect("constraint span").byte_end
                <= operation.span.expect("operation span").byte_start
        );
    }

    #[test]
    fn source_grammar_reconstructs_emphasis_links_and_inline_code() {
        let source = "RepoSeiri does **not** write files; it [audits](docs/audit.md) repositories with `verified` output.\n";
        let scan = scan_document("README.md", source).expect("scan");
        let grammar =
            analyze_readme_grammar_with_source(&scan, source, &ReadmeGrammarOptions::default())
                .expect("source grammar");

        let operation = grammar
            .nodes
            .iter()
            .find(|node| {
                let audit_start = source.find("audits").expect("linked verb");
                node.predicate == GrammarPredicate::PerformsOperation
                    && node.span.is_some_and(|span| {
                        span.byte_start <= audit_start
                            && audit_start + "audits".len() <= span.byte_end
                    })
            })
            .expect("linked operation");
        let evidence = grammar
            .nodes
            .iter()
            .find(|node| node.predicate == GrammarPredicate::ProvidesEvidence)
            .expect("inline-code evidence");
        assert!(!operation.negated);
        assert!(!evidence.negated);
        let clause_span = operation.span.expect("operation span");
        let inline_start = source.find("`verified`").expect("inline code");
        assert!(clause_span.byte_start < inline_start);
        assert!(clause_span.byte_end >= inline_start + "`verified`".len());
        assert_eq!(operation.span, evidence.span);
    }

    #[test]
    fn heading_predicates_are_not_duplicated_in_either_api() {
        let source = "# Audit tool\n";
        let scan = scan_document("README.md", source).expect("scan");
        let compatibility =
            analyze_readme_grammar(&scan, &ReadmeGrammarOptions::default()).expect("grammar");
        let source_aware =
            analyze_readme_grammar_with_source(&scan, source, &ReadmeGrammarOptions::default())
                .expect("source grammar");

        for grammar in [&compatibility, &source_aware] {
            assert_eq!(
                grammar
                    .nodes
                    .iter()
                    .filter(|node| node.predicate == GrammarPredicate::DefinesIdentity)
                    .count(),
                1
            );
            assert_eq!(
                grammar
                    .nodes
                    .iter()
                    .filter(|node| node.predicate == GrammarPredicate::PerformsOperation)
                    .count(),
                1
            );
        }
    }

    #[test]
    fn source_grammar_rejects_length_and_digest_mismatches() {
        let source = "# Tool\nAudits repositories.\n";
        let scan = scan_document("README.md", source).expect("scan");
        let same_length_mismatch = source.replacen("Audits", "Writes", 1);

        for mismatched in [same_length_mismatch, format!("{source} ")] {
            assert_eq!(
                analyze_readme_grammar_with_source(
                    &scan,
                    &mismatched,
                    &ReadmeGrammarOptions::default(),
                ),
                Err(seiri_core::AppealIrError::SourceMismatch)
            );
        }
    }

    #[test]
    fn source_grammar_aligns_distant_japanese_and_english_sections() {
        let source = "## 日本語\nRepoSeiri は監査結果を根拠付きで提示します。\n\n## Reference data\n```text\nline-01\nline-02\nline-03\n```\n\n## English\nRepoSeiri presents audit results with their evidence.\n";
        let scan = scan_document("README.md", source).expect("scan");
        let grammar =
            analyze_readme_grammar_with_source(&scan, source, &ReadmeGrammarOptions::default())
                .expect("source grammar");

        assert_eq!(grammar.translation_alignment.sections.len(), 2);
        assert_eq!(
            grammar.translation_alignment.sections[0].language,
            ReadmeSectionLanguage::Japanese
        );
        assert_eq!(
            grammar.translation_alignment.sections[1].language,
            ReadmeSectionLanguage::English
        );
        let alignment = grammar
            .translation_alignment
            .alignments
            .first()
            .expect("section alignment");
        assert_eq!(alignment.state, TranslationAlignmentState::Aligned);
        assert!(alignment
            .claims
            .iter()
            .all(|claim| claim.state == TranslationAlignmentState::Aligned));
        assert!(grammar.edges.iter().any(|edge| {
            edge.relation == NarrativeRelation::Translates
                && grammar
                    .nodes
                    .iter()
                    .find(|node| node.id == edge.from)
                    .is_some_and(|node| node.language == DocumentLanguage::Japanese)
        }));
    }

    #[test]
    fn monolingual_readme_does_not_invent_a_missing_translation() {
        let source = "## English\nRepoSeiri audits repositories locally.\n";
        let scan = scan_document("README.md", source).expect("scan");
        let grammar =
            analyze_readme_grammar_with_source(&scan, source, &ReadmeGrammarOptions::default())
                .expect("source grammar");

        assert_eq!(grammar.translation_alignment.sections.len(), 1);
        assert!(grammar.translation_alignment.alignments.is_empty());
        assert!(grammar.translation_alignment.unknown_reasons.is_empty());
        assert_eq!(grammar.coverage, CoverageStatus::Complete);
    }

    #[test]
    fn nonsemantic_heading_does_not_make_every_missing_value_unknown() {
        let source = "# Appeal fixture\n";
        let scan = scan_document("README.md", source).expect("scan");
        let grammar =
            analyze_readme_grammar_with_source(&scan, source, &ReadmeGrammarOptions::default())
                .expect("source grammar");

        assert!(grammar.nodes.is_empty());
        assert!(grammar.translation_alignment.sections.is_empty());
        assert!(grammar.translation_alignment.unknown_reasons.is_empty());
        assert_eq!(grammar.coverage, CoverageStatus::Complete);
    }

    #[test]
    fn inverse_bilingual_polarity_is_divergent_and_not_a_translation_edge() {
        let source = "## 日本語\nRepoSeiri はリポジトリへファイルを書き込みません。\n\n## English\nRepoSeiri writes fixes to the repository automatically.\n";
        let scan = scan_document("README.md", source).expect("scan");
        let grammar =
            analyze_readme_grammar_with_source(&scan, source, &ReadmeGrammarOptions::default())
                .expect("source grammar");

        let alignment = grammar
            .translation_alignment
            .alignments
            .first()
            .expect("section alignment");
        assert_eq!(alignment.state, TranslationAlignmentState::Divergent);
        let polarity = alignment
            .claims
            .iter()
            .find(|claim| {
                claim
                    .divergences
                    .contains(&TranslationDivergenceKind::Polarity)
            })
            .expect("polarity divergence");
        let counterpart = polarity.counterpart_claim.expect("paired claim");
        let source_node = claim_atom(&grammar.claim_atoms, polarity.source_claim)
            .expect("source atom")
            .grammar_node;
        let counterpart_node = claim_atom(&grammar.claim_atoms, counterpart)
            .expect("counterpart atom")
            .grammar_node;
        assert!(!grammar.edges.iter().any(|edge| {
            edge.relation == NarrativeRelation::Translates
                && edge.from == source_node
                && edge.to == counterpart_node
        }));
    }

    #[test]
    fn mixed_and_ambiguous_sections_are_not_collapsed_to_binary_language() {
        let mixed_source =
            "## 日本語 and English\nRepoSeiri audits repositories. RepoSeiri は監査します。\n";
        let mixed_scan = scan_document("README.md", mixed_source).expect("scan");
        let mixed = analyze_readme_grammar_with_source(
            &mixed_scan,
            mixed_source,
            &ReadmeGrammarOptions::default(),
        )
        .expect("mixed grammar");
        assert_eq!(
            mixed.translation_alignment.sections[0].language,
            ReadmeSectionLanguage::Mixed
        );

        let ambiguous_source = "## 漢字\n監査\n";
        let ambiguous_scan = scan_document("README.md", ambiguous_source).expect("scan");
        let ambiguous = analyze_readme_grammar_with_source(
            &ambiguous_scan,
            ambiguous_source,
            &ReadmeGrammarOptions::default(),
        )
        .expect("ambiguous grammar");
        assert!(matches!(
            ambiguous.translation_alignment.sections[0].language,
            ReadmeSectionLanguage::Ambiguous(UnknownReason::UnsupportedSyntax)
        ));
        assert_eq!(
            ambiguous.coverage,
            CoverageStatus::Partial(CoverageIncompleteReason::UnsupportedSyntax)
        );
        assert!(ambiguous.claim_atoms.atoms.is_empty());
    }

    #[test]
    fn japanese_atoms_keep_section_membership_terms_qualifiers_and_conditions() {
        let source = "## 日本語\nRepoSeiri はリポジトリをローカルで監査します。オフラインの場合のみ監査します。\n";
        let scan = scan_document("README.md", source).expect("scan");
        let grammar =
            analyze_readme_grammar_with_source(&scan, source, &ReadmeGrammarOptions::default())
                .expect("source grammar");
        let audit = grammar
            .claim_atoms
            .atoms
            .iter()
            .find(|atom| {
                atom.dimension == seiri_core::ValueDimension::Capability
                    && atom.object.as_deref() == Some("repositories")
            })
            .expect("Japanese audit atom");
        assert_eq!(audit.subject.as_deref(), Some("reposeiri"));
        assert_eq!(audit.action.as_deref(), Some("audit"));
        assert!(audit.qualifiers.iter().any(|value| value == "locally"));
        let conditioned = grammar
            .claim_atoms
            .atoms
            .iter()
            .find(|atom| atom.condition.as_deref() == Some("offline"))
            .expect("canonical condition");
        assert!(conditioned.qualifiers.iter().any(|value| value == "only"));
        let section = &grammar.translation_alignment.sections[0];
        assert!(section.claim_atoms.contains(&audit.id));
        assert!(section.claim_atoms.contains(&conditioned.id));
        assert_eq!(
            section.claim_atoms.len(),
            grammar.claim_atoms.atoms.len(),
            "every emitted atom belongs to exactly one source section"
        );
    }
}

use crate::{
    MarkdownBadge, MarkdownHeading, MarkdownLink, RouteCandidate, SourceSpan, TextDocumentBase,
};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum DocumentEvent {
    VisibleProse(MarkdownProse),
    Heading(MarkdownHeading),
    Link(MarkdownLink),
    Badge(MarkdownBadge),
    RouteCandidate(RouteCandidate),
}

impl DocumentEvent {
    #[must_use]
    pub const fn span(&self) -> Option<SourceSpan> {
        match self {
            Self::VisibleProse(value) => Some(value.span),
            Self::Heading(value) => value.span,
            Self::Link(value) => value.span,
            Self::Badge(value) => value.span,
            Self::RouteCandidate(value) => value.span,
        }
    }

    pub const fn order_rank(&self) -> u8 {
        match self {
            Self::VisibleProse(_) => 0,
            Self::Heading(_) => 1,
            Self::Link(_) => 2,
            Self::Badge(_) => 3,
            Self::RouteCandidate(_) => 4,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarkdownProse {
    pub text: String,
    pub line: usize,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentDiagnosticKind {
    UnclosedLinkLabel,
    UnclosedLinkTarget,
    UnresolvedReferenceLink,
    UnsupportedHtml,
    HtmlAttributeLimitExceeded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentDiagnostic {
    pub kind: DocumentDiagnosticKind,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DocumentScan {
    path: String,
    source_bytes: usize,
    base: TextDocumentBase,
    events: Vec<DocumentEvent>,
    diagnostics: Vec<DocumentDiagnostic>,
}

impl DocumentScan {
    pub fn new(
        path: String,
        base: TextDocumentBase,
        events: Vec<DocumentEvent>,
        diagnostics: Vec<DocumentDiagnostic>,
    ) -> Result<Self, DocumentScanInvariantError> {
        let source_bytes = base.byte_len();
        validate_document_scan(&path, source_bytes, &events, &diagnostics)?;
        Ok(Self {
            path,
            source_bytes,
            base,
            events,
            diagnostics,
        })
    }

    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    #[must_use]
    pub const fn source_bytes(&self) -> usize {
        self.source_bytes
    }

    #[must_use]
    pub const fn base(&self) -> &TextDocumentBase {
        &self.base
    }

    #[must_use]
    pub fn events(&self) -> &[DocumentEvent] {
        &self.events
    }

    #[must_use]
    pub fn diagnostics(&self) -> &[DocumentDiagnostic] {
        &self.diagnostics
    }

    /// Revalidates the structural scan against the exact UTF-8 source and
    /// repository-relative path that produced it.
    pub fn validate_against_source(
        &self,
        path: &str,
        source: &[u8],
    ) -> Result<(), DocumentScanInvariantError> {
        if self.path != path {
            return Err(DocumentScanInvariantError::SourcePathMismatch);
        }
        if self.source_bytes != source.len() {
            return Err(DocumentScanInvariantError::SourceLengthMismatch);
        }
        let source_base = TextDocumentBase::from_bytes(source);
        if self.base.digest() != source_base.digest() {
            return Err(DocumentScanInvariantError::SourceDigestMismatch);
        }
        let source =
            std::str::from_utf8(source).map_err(|_| DocumentScanInvariantError::SourceNotUtf8)?;
        for (event_index, event) in self.events.iter().enumerate() {
            let span = event
                .span()
                .ok_or(DocumentScanInvariantError::MissingEventSpan { event_index })?;
            validate_source_span(source, span).map_err(|error| match error {
                SourceSpanValidationError::OutOfBounds => {
                    DocumentScanInvariantError::EventSpanOutOfBounds { event_index }
                }
                SourceSpanValidationError::NotCharBoundary => {
                    DocumentScanInvariantError::EventSpanNotCharBoundary { event_index }
                }
                SourceSpanValidationError::LineColumnMismatch => {
                    DocumentScanInvariantError::EventLineColumnMismatch { event_index }
                }
            })?;
        }
        for (diagnostic_index, diagnostic) in self.diagnostics.iter().enumerate() {
            validate_source_span(source, diagnostic.span).map_err(|error| match error {
                SourceSpanValidationError::OutOfBounds => {
                    DocumentScanInvariantError::DiagnosticSpanOutOfBounds { diagnostic_index }
                }
                SourceSpanValidationError::NotCharBoundary => {
                    DocumentScanInvariantError::DiagnosticSpanNotCharBoundary { diagnostic_index }
                }
                SourceSpanValidationError::LineColumnMismatch => {
                    DocumentScanInvariantError::DiagnosticLineColumnMismatch { diagnostic_index }
                }
            })?;
        }
        Ok(())
    }
}

impl<'de> Deserialize<'de> for DocumentScan {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct WireScan {
            path: String,
            source_bytes: usize,
            base: TextDocumentBase,
            events: Vec<DocumentEvent>,
            diagnostics: Vec<DocumentDiagnostic>,
        }

        let wire = WireScan::deserialize(deserializer)?;
        if wire.source_bytes != wire.base.byte_len() {
            return Err(D::Error::custom(
                "document source_bytes must match base byte_len",
            ));
        }
        Self::new(wire.path, wire.base, wire.events, wire.diagnostics).map_err(D::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentScanInvariantError {
    EmptyPath,
    SourcePathMismatch,
    SourceLengthMismatch,
    SourceDigestMismatch,
    SourceNotUtf8,
    MissingEventSpan { event_index: usize },
    EventSpanOutOfBounds { event_index: usize },
    EventSpanNotCharBoundary { event_index: usize },
    EventLineColumnMismatch { event_index: usize },
    DiagnosticSpanOutOfBounds { diagnostic_index: usize },
    DiagnosticSpanNotCharBoundary { diagnostic_index: usize },
    DiagnosticLineColumnMismatch { diagnostic_index: usize },
    NonCanonicalEventOrder { event_index: usize },
    NonCanonicalDiagnosticOrder { diagnostic_index: usize },
}

impl Display for DocumentScanInvariantError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyPath => formatter.write_str("document scan path must not be empty"),
            Self::SourcePathMismatch => {
                formatter.write_str("document scan path does not match the bound source")
            }
            Self::SourceLengthMismatch => {
                formatter.write_str("document scan byte length does not match the bound source")
            }
            Self::SourceDigestMismatch => {
                formatter.write_str("document scan digest does not match the bound source")
            }
            Self::SourceNotUtf8 => formatter.write_str("document scan source must be valid UTF-8"),
            Self::MissingEventSpan { event_index } => {
                write!(
                    formatter,
                    "document event {event_index} is missing a source span"
                )
            }
            Self::EventSpanOutOfBounds { event_index } => write!(
                formatter,
                "document event {event_index} has a span outside the source byte range"
            ),
            Self::EventSpanNotCharBoundary { event_index } => write!(
                formatter,
                "document event {event_index} span splits a UTF-8 code point"
            ),
            Self::EventLineColumnMismatch { event_index } => write!(
                formatter,
                "document event {event_index} line or column does not match its byte span"
            ),
            Self::DiagnosticSpanOutOfBounds { diagnostic_index } => write!(
                formatter,
                "document diagnostic {diagnostic_index} has a span outside the source byte range"
            ),
            Self::DiagnosticSpanNotCharBoundary { diagnostic_index } => write!(
                formatter,
                "document diagnostic {diagnostic_index} span splits a UTF-8 code point"
            ),
            Self::DiagnosticLineColumnMismatch { diagnostic_index } => write!(
                formatter,
                "document diagnostic {diagnostic_index} line or column does not match its byte span"
            ),
            Self::NonCanonicalEventOrder { event_index } => write!(
                formatter,
                "document event {event_index} is not in deterministic source order"
            ),
            Self::NonCanonicalDiagnosticOrder { diagnostic_index } => write!(
                formatter,
                "document diagnostic {diagnostic_index} is not in deterministic source order"
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourceSpanValidationError {
    OutOfBounds,
    NotCharBoundary,
    LineColumnMismatch,
}

fn validate_source_span(source: &str, span: SourceSpan) -> Result<(), SourceSpanValidationError> {
    if span.byte_start > span.byte_end || span.byte_end > source.len() {
        return Err(SourceSpanValidationError::OutOfBounds);
    }
    if !source.is_char_boundary(span.byte_start) || !source.is_char_boundary(span.byte_end) {
        return Err(SourceSpanValidationError::NotCharBoundary);
    }
    let prefix = &source[..span.byte_start];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let line_start = prefix.rfind('\n').map_or(0, |index| index + 1);
    let column = source[line_start..span.byte_start].chars().count() + 1;
    if span.line != line || span.column != column {
        return Err(SourceSpanValidationError::LineColumnMismatch);
    }
    Ok(())
}

impl std::error::Error for DocumentScanInvariantError {}

fn validate_document_scan(
    path: &str,
    source_bytes: usize,
    events: &[DocumentEvent],
    diagnostics: &[DocumentDiagnostic],
) -> Result<(), DocumentScanInvariantError> {
    if path.is_empty() {
        return Err(DocumentScanInvariantError::EmptyPath);
    }

    let mut previous_key = None;
    for (event_index, event) in events.iter().enumerate() {
        let span = event
            .span()
            .ok_or(DocumentScanInvariantError::MissingEventSpan { event_index })?;
        if span.byte_start > span.byte_end || span.byte_end > source_bytes {
            return Err(DocumentScanInvariantError::EventSpanOutOfBounds { event_index });
        }
        let key = (span.byte_start, event.order_rank(), span.byte_end);
        if previous_key.is_some_and(|previous| previous > key) {
            return Err(DocumentScanInvariantError::NonCanonicalEventOrder { event_index });
        }
        previous_key = Some(key);
    }

    let mut previous_diagnostic_key = None;
    for (diagnostic_index, diagnostic) in diagnostics.iter().enumerate() {
        if diagnostic.span.byte_start > diagnostic.span.byte_end
            || diagnostic.span.byte_end > source_bytes
        {
            return Err(DocumentScanInvariantError::DiagnosticSpanOutOfBounds { diagnostic_index });
        }
        let key = (
            diagnostic.span.byte_start,
            diagnostic_kind_rank(diagnostic.kind),
            diagnostic.span.byte_end,
        );
        if previous_diagnostic_key.is_some_and(|previous| previous > key) {
            return Err(DocumentScanInvariantError::NonCanonicalDiagnosticOrder {
                diagnostic_index,
            });
        }
        previous_diagnostic_key = Some(key);
    }
    Ok(())
}

const fn diagnostic_kind_rank(kind: DocumentDiagnosticKind) -> u8 {
    match kind {
        DocumentDiagnosticKind::UnclosedLinkLabel => 0,
        DocumentDiagnosticKind::UnclosedLinkTarget => 1,
        DocumentDiagnosticKind::UnresolvedReferenceLink => 2,
        DocumentDiagnosticKind::UnsupportedHtml => 3,
        DocumentDiagnosticKind::HtmlAttributeLimitExceeded => 4,
    }
}

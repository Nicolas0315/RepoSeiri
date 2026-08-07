#![forbid(unsafe_code)]

use seiri_core::{
    CapabilityEdge, CapabilityKind, CapabilityNode, CapabilityNodeId, CapabilityProvenance,
    CapabilityProvenanceKind, CapabilityRelation, CapabilitySupport, CoverageIncompleteReason,
    CoverageStatus, FileKind, FileRecord, RepositoryCapabilityIR, SourceDocument, SourceSpan,
    SourceStore, SourceStoreError, UnknownReason,
};
use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::io::{self, Read};
use std::num::NonZeroU32;
use std::path::{Component, Path};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgramSourceOptions {
    pub max_files: usize,
    pub max_total_source_bytes: usize,
    pub max_source_bytes: usize,
}

impl Default for ProgramSourceOptions {
    fn default() -> Self {
        Self {
            max_files: 512,
            max_total_source_bytes: 8 * 1024 * 1024,
            max_source_bytes: 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgramAnalysisOptions {
    pub max_nodes: usize,
    pub max_edges: usize,
}

impl Default for ProgramAnalysisOptions {
    fn default() -> Self {
        Self {
            max_nodes: 65_536,
            max_edges: 131_072,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramReadIssue {
    pub path: String,
    pub reason: UnknownReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramSourceReport {
    pub coverage: CoverageStatus,
    pub candidate_files: usize,
    pub selected_files: usize,
    pub selected_bytes: usize,
    pub skipped_existing: usize,
    pub issues: Vec<ProgramReadIssue>,
}

pub struct ProgramSourceSession {
    store: SourceStore,
    report: ProgramSourceReport,
}

impl ProgramSourceSession {
    #[must_use]
    pub const fn store(&self) -> &SourceStore {
        &self.store
    }

    #[must_use]
    pub const fn report(&self) -> &ProgramSourceReport {
        &self.report
    }

    #[must_use]
    pub fn into_parts(self) -> (SourceStore, ProgramSourceReport) {
        (self.store, self.report)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgramLocalError {
    SourceStore(SourceStoreError),
}

impl Display for ProgramLocalError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SourceStore(error) => write!(formatter, "program source store failed: {error}"),
        }
    }
}

impl std::error::Error for ProgramLocalError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SourceStore(error) => Some(error),
        }
    }
}

impl From<SourceStoreError> for ProgramLocalError {
    fn from(value: SourceStoreError) -> Self {
        Self::SourceStore(value)
    }
}

pub fn scan_program_source_session(
    repo_root: &Path,
    files: &[FileRecord],
    repository_complete: bool,
    existing: SourceStore,
    options: &ProgramSourceOptions,
) -> Result<ProgramSourceSession, ProgramLocalError> {
    let mut candidates = files
        .iter()
        .filter(|file| file.kind == FileKind::File && is_program_source_path(&file.path))
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        source_rank(&left.path)
            .cmp(&source_rank(&right.path))
            .then_with(|| left.path.cmp(&right.path))
    });
    let candidate_files = candidates.len();
    let mut additional = Vec::new();
    let mut selected_bytes = 0usize;
    let mut skipped_existing = 0usize;
    let mut issues = Vec::new();
    for candidate in candidates {
        if existing.get(&candidate.path).is_some() {
            skipped_existing = skipped_existing.saturating_add(1);
            continue;
        }
        if additional.len() >= options.max_files {
            issues.push(issue(&candidate.path, UnknownReason::LimitExceeded));
            continue;
        }
        let declared = usize::try_from(candidate.size_bytes).unwrap_or(usize::MAX);
        let remaining = options
            .max_total_source_bytes
            .saturating_sub(selected_bytes);
        if declared > options.max_source_bytes || declared > remaining {
            issues.push(issue(&candidate.path, UnknownReason::LimitExceeded));
            continue;
        }
        if !is_safe_relative(&candidate.path) {
            issues.push(issue(&candidate.path, UnknownReason::Unavailable));
            continue;
        }
        let limit = options.max_source_bytes.min(remaining);
        match read_bounded(&repo_root.join(&candidate.path), limit) {
            Ok(bytes) => {
                selected_bytes = selected_bytes.saturating_add(bytes.len());
                additional.push(SourceDocument::from_bytes(candidate.path.clone(), bytes));
            }
            Err(ReadFailure::LimitExceeded) => {
                issues.push(issue(&candidate.path, UnknownReason::LimitExceeded));
            }
            Err(ReadFailure::Io(error)) => {
                issues.push(issue(&candidate.path, reason_for_io(&error)));
            }
        }
    }
    additional.sort_by(|left, right| left.path().cmp(right.path()));
    let selected_files = additional.len();
    let store = existing.try_extend(additional)?;
    let coverage = if repository_complete && issues.is_empty() {
        CoverageStatus::Complete
    } else {
        CoverageStatus::Partial(CoverageIncompleteReason::LimitExceeded)
    };
    issues.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(ProgramSourceSession {
        store,
        report: ProgramSourceReport {
            coverage,
            candidate_files,
            selected_files,
            selected_bytes,
            skipped_existing,
            issues,
        },
    })
}

#[must_use]
pub fn analyze_capabilities_l0(
    store: &SourceStore,
    source_report: &ProgramSourceReport,
    options: &ProgramAnalysisOptions,
) -> RepositoryCapabilityIR {
    let mut builder = CapabilityBuilder::new(source_report.coverage, *options);
    seed_path_capabilities(store, &mut builder);
    builder.finish()
}

#[must_use]
pub fn analyze_rust_capabilities(
    store: &SourceStore,
    source_report: &ProgramSourceReport,
    options: &ProgramAnalysisOptions,
) -> RepositoryCapabilityIR {
    let mut builder = CapabilityBuilder::new(source_report.coverage, *options);
    seed_path_capabilities(store, &mut builder);
    for source in store
        .documents()
        .iter()
        .filter(|source| source.path().to_ascii_lowercase().ends_with(".rs"))
    {
        let Some(text) = source.text() else {
            builder.unknown(UnknownReason::InvalidUtf8);
            continue;
        };
        analyze_rust_source(source.path(), text, &mut builder);
        if builder.limit_reached {
            break;
        }
    }
    builder.finish()
}

fn seed_path_capabilities(store: &SourceStore, builder: &mut CapabilityBuilder) {
    let mut rust_paths = Vec::new();
    let mut manifest_ids = Vec::new();
    for source in store.documents() {
        let lower = source.path().to_ascii_lowercase();
        if lower.ends_with("cargo.toml") {
            if let Some(id) = builder.add_observed(
                CapabilityKind::Manifest,
                source.path(),
                source.path(),
                CapabilityProvenanceKind::Manifest,
                None,
            ) {
                manifest_ids.push(id);
            }
            seed_manifest_features(source, builder);
        }
        if lower.ends_with(".rs") {
            rust_paths.push(source.path().to_string());
            let kind = if is_entrypoint_path(&lower) {
                Some(CapabilityKind::Entrypoint)
            } else if lower.starts_with("examples/") || lower.contains("/examples/") {
                Some(CapabilityKind::Example)
            } else if lower.starts_with("tests/") || lower.contains("/tests/") {
                Some(CapabilityKind::Test)
            } else {
                None
            };
            if let Some(kind) = kind {
                builder.add_observed(
                    kind,
                    source.path(),
                    source.path(),
                    match kind {
                        CapabilityKind::Example => CapabilityProvenanceKind::ExamplePath,
                        CapabilityKind::Test => CapabilityProvenanceKind::TestPath,
                        _ => CapabilityProvenanceKind::SourceSyntax,
                    },
                    None,
                );
            }
        }
    }
    if let Some(first_path) = rust_paths.first() {
        if let Some(language_id) = builder.add_observed(
            CapabilityKind::ProgramLanguage,
            "rust",
            first_path,
            CapabilityProvenanceKind::SourceSyntax,
            None,
        ) {
            for manifest_id in manifest_ids {
                builder.add_edge(manifest_id, language_id, CapabilityRelation::Offers);
            }
        }
    }
}

fn seed_manifest_features(source: &SourceDocument, builder: &mut CapabilityBuilder) {
    let Some(text) = source.text() else {
        builder.unknown(UnknownReason::InvalidUtf8);
        return;
    };
    let mut in_features = false;
    let mut byte_start = 0usize;
    for (line_index, line) in text.split_inclusive('\n').enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_features = trimmed == "[features]";
        } else if in_features && !trimmed.is_empty() && !trimmed.starts_with('#') {
            if let Some((name, _)) = trimmed.split_once('=') {
                let name = name.trim().trim_matches(['\'', '"']);
                if !name.is_empty() {
                    let column = line.find(name).unwrap_or(0);
                    builder.add_observed(
                        CapabilityKind::Feature,
                        name,
                        source.path(),
                        CapabilityProvenanceKind::Manifest,
                        Some(SourceSpan::new(
                            line_index + 1,
                            column + 1,
                            byte_start + column,
                            byte_start + column + name.len(),
                        )),
                    );
                }
            }
        }
        byte_start = byte_start.saturating_add(line.len());
    }
}

fn analyze_rust_source(path: &str, text: &str, builder: &mut CapabilityBuilder) {
    let masked = mask_rust_dead_zones(text);
    let mut byte_start = 0usize;
    let mut pending_attributes = Vec::new();
    for (line_index, line) in masked.split_inclusive('\n').enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("#[") {
            pending_attributes.push(trimmed.to_string());
            byte_start = byte_start.saturating_add(line.len());
            continue;
        }
        if trimmed.contains("macro_rules!") || trimmed.contains("include!") {
            builder.unknown(UnknownReason::UnsupportedSyntax);
        }
        let is_test = pending_attributes
            .iter()
            .any(|attribute| attribute.starts_with("#[test"));
        let feature = pending_attributes.iter().find_map(|attribute| {
            attribute
                .split_once("feature")
                .and_then(|(_, tail)| tail.split('"').nth(1))
        });
        let span_base = line
            .find(|character: char| !character.is_whitespace())
            .unwrap_or(0);
        let line_number = line_index + 1;
        let mut item_id = None;
        if let Some(name) = function_name(trimmed, true) {
            let span = symbol_span(line, name, line_number, byte_start);
            item_id = builder.add_observed(
                CapabilityKind::Operation,
                name,
                path,
                CapabilityProvenanceKind::SourceSyntax,
                span,
            );
            if trimmed.contains("extern ") {
                builder.unknown(UnknownReason::UnsupportedSyntax);
            }
            if let Some(function_id) = item_id {
                add_function_shape(
                    path,
                    line,
                    name,
                    line_number,
                    byte_start,
                    function_id,
                    builder,
                );
            }
        } else if let Some((kind, name)) = public_item(trimmed) {
            item_id = builder.add_observed(
                kind,
                name,
                path,
                CapabilityProvenanceKind::SourceSyntax,
                symbol_span(line, name, line_number, byte_start),
            );
        } else if let Some(name) = function_name(trimmed, false) {
            if name == "main" || is_test {
                item_id = builder.add_observed(
                    if is_test {
                        CapabilityKind::Test
                    } else {
                        CapabilityKind::Entrypoint
                    },
                    name,
                    path,
                    if is_test {
                        CapabilityProvenanceKind::TestPath
                    } else {
                        CapabilityProvenanceKind::SourceSyntax
                    },
                    symbol_span(line, name, line_number, byte_start),
                );
            }
        } else if trimmed.starts_with("pub fn") || trimmed.starts_with("pub async fn") {
            builder.unknown(UnknownReason::ParseFailed);
        }
        if let (Some(item_id), Some(feature)) = (item_id, feature) {
            if let Some(feature_id) = builder.add_observed(
                CapabilityKind::Feature,
                feature,
                path,
                CapabilityProvenanceKind::SourceSyntax,
                Some(SourceSpan::new(
                    line_number.saturating_sub(pending_attributes.len()),
                    1,
                    byte_start
                        .saturating_sub(pending_attributes.iter().map(String::len).sum::<usize>()),
                    byte_start + span_base,
                )),
            ) {
                builder.add_edge(item_id, feature_id, CapabilityRelation::ConditionedBy);
            }
        }
        if !trimmed.is_empty() {
            pending_attributes.clear();
        }
        byte_start = byte_start.saturating_add(line.len());
        if builder.limit_reached {
            break;
        }
    }
}

fn add_function_shape(
    path: &str,
    line: &str,
    name: &str,
    line_number: usize,
    byte_start: usize,
    function_id: CapabilityNodeId,
    builder: &mut CapabilityBuilder,
) {
    let trimmed = line.trim();
    if let Some((_, after_open)) = trimmed.split_once('(') {
        if let Some((parameters, after_close)) = after_open.split_once(')') {
            let meaningful = parameters.split(',').map(str::trim).any(|parameter| {
                !parameter.is_empty() && !matches!(parameter, "self" | "&self" | "&mut self")
            });
            if meaningful {
                if let Some(input_id) = builder.add_observed(
                    CapabilityKind::Input,
                    &format!("{name}::input"),
                    path,
                    CapabilityProvenanceKind::SourceSyntax,
                    symbol_span(line, name, line_number, byte_start),
                ) {
                    builder.add_edge(function_id, input_id, CapabilityRelation::Accepts);
                }
            }
            if let Some((_, output)) = after_close.split_once("->") {
                let output = output.trim().trim_end_matches(['{', ';']).trim();
                if !output.is_empty() && output != "()" {
                    if let Some(output_id) = builder.add_observed(
                        CapabilityKind::Output,
                        &format!("{name}::output"),
                        path,
                        CapabilityProvenanceKind::SourceSyntax,
                        symbol_span(line, name, line_number, byte_start),
                    ) {
                        builder.add_edge(function_id, output_id, CapabilityRelation::Produces);
                    }
                }
            }
        }
    }
}

fn function_name(line: &str, public_only: bool) -> Option<&str> {
    let mut rest = line.trim_start();
    let is_public = rest.starts_with("pub ");
    if public_only && !is_public {
        return None;
    }
    if is_public {
        rest = rest.strip_prefix("pub ")?;
    }
    for modifier in ["async ", "const ", "unsafe ", "extern "] {
        if rest.starts_with(modifier) {
            rest = rest.strip_prefix(modifier)?;
            if modifier == "extern " && rest.starts_with('"') {
                rest = rest.split_once('"')?.1.split_once('"')?.1.trim_start();
            }
        }
    }
    rest = rest.strip_prefix("fn ")?;
    identifier(rest)
}

fn public_item(line: &str) -> Option<(CapabilityKind, &str)> {
    let rest = line.trim_start().strip_prefix("pub ")?;
    for keyword in [
        "struct ", "enum ", "trait ", "type ", "mod ", "const ", "static ",
    ] {
        if let Some(tail) = rest.strip_prefix(keyword) {
            return identifier(tail).map(|name| (CapabilityKind::PublicApi, name));
        }
    }
    None
}

fn identifier(value: &str) -> Option<&str> {
    let end = value
        .char_indices()
        .take_while(|(_, character)| character.is_alphanumeric() || *character == '_')
        .map(|(index, character)| index + character.len_utf8())
        .last()?;
    Some(&value[..end])
}

fn symbol_span(
    line: &str,
    symbol: &str,
    line_number: usize,
    byte_start: usize,
) -> Option<SourceSpan> {
    let column = line.find(symbol)?;
    Some(SourceSpan::new(
        line_number,
        column + 1,
        byte_start + column,
        byte_start + column + symbol.len(),
    ))
}

fn mask_rust_dead_zones(text: &str) -> String {
    #[derive(Clone, Copy)]
    enum State {
        Normal,
        LineComment,
        BlockComment(usize),
        String(bool),
    }
    let bytes = text.as_bytes();
    let mut output = bytes.to_vec();
    let mut state = State::Normal;
    let mut index = 0usize;
    while index < bytes.len() {
        let current = bytes[index];
        let next = bytes.get(index + 1).copied();
        match state {
            State::Normal if current == b'/' && next == Some(b'/') => {
                output[index] = b' ';
                output[index + 1] = b' ';
                state = State::LineComment;
                index += 2;
                continue;
            }
            State::Normal if current == b'/' && next == Some(b'*') => {
                output[index] = b' ';
                output[index + 1] = b' ';
                state = State::BlockComment(1);
                index += 2;
                continue;
            }
            State::Normal if current == b'"' => {
                output[index] = b' ';
                state = State::String(false);
            }
            State::LineComment if current == b'\n' => state = State::Normal,
            State::LineComment => output[index] = b' ',
            State::BlockComment(depth) if current == b'/' && next == Some(b'*') => {
                output[index] = b' ';
                output[index + 1] = b' ';
                state = State::BlockComment(depth.saturating_add(1));
                index += 2;
                continue;
            }
            State::BlockComment(depth) if current == b'*' && next == Some(b'/') => {
                output[index] = b' ';
                output[index + 1] = b' ';
                state = if depth == 1 {
                    State::Normal
                } else {
                    State::BlockComment(depth - 1)
                };
                index += 2;
                continue;
            }
            State::BlockComment(_) if current != b'\n' => output[index] = b' ',
            State::String(escaped) => {
                if current != b'\n' {
                    output[index] = b' ';
                }
                if current == b'"' && !escaped {
                    state = State::Normal;
                } else {
                    state = State::String(current == b'\\' && !escaped);
                }
            }
            State::Normal | State::BlockComment(_) => {}
        }
        index += 1;
    }
    String::from_utf8(output).expect("mask preserves UTF-8 bytes outside ASCII dead zones")
}

struct CapabilityBuilder {
    coverage: CoverageStatus,
    options: ProgramAnalysisOptions,
    nodes: Vec<CapabilityNode>,
    edges: Vec<CapabilityEdge>,
    keys: BTreeMap<(CapabilityKind, String, String), CapabilityNodeId>,
    unknown_reasons: Vec<UnknownReason>,
    limit_reached: bool,
}

impl CapabilityBuilder {
    fn new(coverage: CoverageStatus, options: ProgramAnalysisOptions) -> Self {
        Self {
            coverage,
            options,
            nodes: Vec::new(),
            edges: Vec::new(),
            keys: BTreeMap::new(),
            unknown_reasons: Vec::new(),
            limit_reached: false,
        }
    }

    fn add_observed(
        &mut self,
        kind: CapabilityKind,
        symbol: &str,
        path: &str,
        provenance_kind: CapabilityProvenanceKind,
        span: Option<SourceSpan>,
    ) -> Option<CapabilityNodeId> {
        let key = (kind, symbol.to_string(), path.to_string());
        if let Some(id) = self.keys.get(&key) {
            return Some(*id);
        }
        if self.nodes.len() >= self.options.max_nodes || self.nodes.len() >= u32::MAX as usize {
            self.limit_reached = true;
            self.unknown(UnknownReason::LimitExceeded);
            return None;
        }
        let id = CapabilityNodeId::new(
            NonZeroU32::new((self.nodes.len() + 1) as u32).expect("node limit checked"),
        );
        self.nodes.push(CapabilityNode {
            id,
            kind,
            support: CapabilitySupport::Observed,
            symbol: symbol.to_string(),
            provenance: vec![CapabilityProvenance {
                path: path.to_string(),
                kind: provenance_kind,
                span,
            }],
        });
        self.keys.insert(key, id);
        Some(id)
    }

    fn add_edge(
        &mut self,
        from: CapabilityNodeId,
        to: CapabilityNodeId,
        relation: CapabilityRelation,
    ) {
        if self.edges.len() >= self.options.max_edges {
            self.limit_reached = true;
            self.unknown(UnknownReason::LimitExceeded);
            return;
        }
        let edge = CapabilityEdge { from, to, relation };
        if !self.edges.contains(&edge) {
            self.edges.push(edge);
        }
    }

    fn unknown(&mut self, reason: UnknownReason) {
        if !self.unknown_reasons.contains(&reason) {
            self.unknown_reasons.push(reason);
        }
    }

    fn finish(mut self) -> RepositoryCapabilityIR {
        if self.limit_reached {
            self.coverage = CoverageStatus::Partial(CoverageIncompleteReason::LimitExceeded);
        } else if self.coverage == CoverageStatus::Complete {
            if let Some(reason) = self
                .unknown_reasons
                .iter()
                .copied()
                .min_by_key(|reason| unknown_rank(*reason))
            {
                self.coverage = CoverageStatus::Partial(coverage_reason(reason));
            }
        }
        self.edges
            .sort_by_key(|edge| (edge.from, edge.to, relation_rank(edge.relation)));
        RepositoryCapabilityIR::try_new(self.coverage, self.nodes, self.edges, self.unknown_reasons)
            .expect("builder maintains canonical capability IR")
    }
}

const fn unknown_rank(reason: UnknownReason) -> u8 {
    match reason {
        UnknownReason::LimitExceeded => 0,
        UnknownReason::InvalidUtf8 => 1,
        UnknownReason::ParseFailed => 2,
        UnknownReason::UnsupportedSyntax => 3,
        UnknownReason::PermissionDenied => 4,
        UnknownReason::RateLimited => 5,
        UnknownReason::Unavailable => 6,
        UnknownReason::NotRequested => 7,
    }
}

const fn coverage_reason(reason: UnknownReason) -> CoverageIncompleteReason {
    match reason {
        UnknownReason::LimitExceeded => CoverageIncompleteReason::LimitExceeded,
        UnknownReason::InvalidUtf8 => CoverageIncompleteReason::InvalidUtf8,
        UnknownReason::ParseFailed => CoverageIncompleteReason::ParseFailed,
        UnknownReason::UnsupportedSyntax => CoverageIncompleteReason::UnsupportedSyntax,
        UnknownReason::PermissionDenied => CoverageIncompleteReason::PermissionDenied,
        UnknownReason::RateLimited => CoverageIncompleteReason::RateLimited,
        UnknownReason::Unavailable | UnknownReason::NotRequested => {
            CoverageIncompleteReason::Unavailable
        }
    }
}

const fn relation_rank(relation: CapabilityRelation) -> u8 {
    match relation {
        CapabilityRelation::Offers => 0,
        CapabilityRelation::Accepts => 1,
        CapabilityRelation::Produces => 2,
        CapabilityRelation::DemonstratedBy => 3,
        CapabilityRelation::ConstrainedBy => 4,
        CapabilityRelation::Exposes => 5,
        CapabilityRelation::ConditionedBy => 6,
    }
}

fn is_program_source_path(path: &str) -> bool {
    let lower = path.replace('\\', "/").to_ascii_lowercase();
    lower.ends_with("cargo.toml") || lower.ends_with(".rs")
}

fn source_rank(path: &str) -> u8 {
    let lower = path.replace('\\', "/").to_ascii_lowercase();
    if lower == "cargo.toml" {
        0
    } else if matches!(lower.as_str(), "src/main.rs" | "src/lib.rs") {
        1
    } else if lower.starts_with("src/") || lower.contains("/src/") {
        2
    } else if lower.starts_with("examples/") || lower.contains("/examples/") {
        3
    } else if lower.starts_with("tests/") || lower.contains("/tests/") {
        4
    } else if lower.ends_with("cargo.toml") {
        5
    } else {
        6
    }
}

fn is_entrypoint_path(path: &str) -> bool {
    path == "src/main.rs"
        || path.ends_with("/src/main.rs")
        || path.starts_with("src/bin/")
        || path.contains("/src/bin/")
}

fn is_safe_relative(path: &str) -> bool {
    let path = Path::new(path);
    !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
}

fn issue(path: &str, reason: UnknownReason) -> ProgramReadIssue {
    ProgramReadIssue {
        path: path.to_string(),
        reason,
    }
}

enum ReadFailure {
    LimitExceeded,
    Io(io::Error),
}

fn read_bounded(path: &Path, limit: usize) -> Result<Vec<u8>, ReadFailure> {
    let file = File::open(path).map_err(ReadFailure::Io)?;
    let take_limit = u64::try_from(limit)
        .unwrap_or(u64::MAX - 1)
        .saturating_add(1);
    let mut bytes = Vec::new();
    file.take(take_limit)
        .read_to_end(&mut bytes)
        .map_err(ReadFailure::Io)?;
    if bytes.len() > limit {
        Err(ReadFailure::LimitExceeded)
    } else {
        Ok(bytes)
    }
}

fn reason_for_io(error: &io::Error) -> UnknownReason {
    if error.kind() == io::ErrorKind::PermissionDenied {
        UnknownReason::PermissionDenied
    } else {
        UnknownReason::Unavailable
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str, size: usize) -> FileRecord {
        FileRecord {
            path: path.to_string(),
            kind: FileKind::File,
            size_bytes: size as u64,
        }
    }

    #[test]
    fn source_session_skips_existing_documents_without_rereading() {
        let root = tempfile::tempdir().expect("tempdir");
        let existing = SourceStore::try_new(vec![SourceDocument::from_bytes(
            "Cargo.toml".into(),
            b"[package]\nname='x'".to_vec(),
        )])
        .expect("store");
        let session = scan_program_source_session(
            root.path(),
            &[file("Cargo.toml", 20)],
            true,
            existing,
            &ProgramSourceOptions::default(),
        )
        .expect("session");
        assert_eq!(session.report().skipped_existing, 1);
        assert_eq!(session.report().selected_files, 0);
        assert_eq!(session.store().documents().len(), 1);
    }

    #[test]
    fn source_budget_is_partial_not_absent() {
        let root = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir(root.path().join("src")).expect("src");
        std::fs::write(root.path().join("src/lib.rs"), "pub fn audit() {}").expect("source");
        let session = scan_program_source_session(
            root.path(),
            &[file("src/lib.rs", 17)],
            true,
            SourceStore::default(),
            &ProgramSourceOptions {
                max_files: 0,
                ..ProgramSourceOptions::default()
            },
        )
        .expect("session");
        assert_eq!(
            session.report().coverage,
            CoverageStatus::Partial(CoverageIncompleteReason::LimitExceeded)
        );
        assert_eq!(
            session.report().issues[0].reason,
            UnknownReason::LimitExceeded
        );
    }

    #[test]
    fn rust_frontend_extracts_public_shape_and_ignores_comments() {
        let store = SourceStore::try_new(vec![SourceDocument::from_bytes(
            "src/lib.rs".into(),
            b"// pub fn fake() -> bool {}\npub fn audit(path: &str) -> Result<(), ()> { Ok(()) }\n"
                .to_vec(),
        )])
        .expect("store");
        let report = ProgramSourceReport {
            coverage: CoverageStatus::Complete,
            candidate_files: 1,
            selected_files: 1,
            selected_bytes: 80,
            skipped_existing: 0,
            issues: Vec::new(),
        };
        let ir = analyze_rust_capabilities(&store, &report, &ProgramAnalysisOptions::default());
        assert!(ir
            .nodes
            .iter()
            .any(|node| { node.kind == CapabilityKind::Operation && node.symbol == "audit" }));
        assert!(!ir.nodes.iter().any(|node| node.symbol == "fake"));
        assert!(ir
            .nodes
            .iter()
            .any(|node| node.kind == CapabilityKind::Input));
        assert!(ir
            .nodes
            .iter()
            .any(|node| node.kind == CapabilityKind::Output));
    }

    #[test]
    fn unsupported_program_syntax_makes_unobserved_capability_unknown() {
        let store = SourceStore::try_new(vec![SourceDocument::from_bytes(
            "src/lib.rs".into(),
            b"include!(concat!(env!(\"OUT_DIR\"), \"/generated.rs\"));\n".to_vec(),
        )])
        .expect("store");
        let report = ProgramSourceReport {
            coverage: CoverageStatus::Complete,
            candidate_files: 1,
            selected_files: 1,
            selected_bytes: store.total_bytes(),
            skipped_existing: 0,
            issues: Vec::new(),
        };

        let ir = analyze_rust_capabilities(&store, &report, &ProgramAnalysisOptions::default());

        assert_eq!(
            ir.coverage,
            CoverageStatus::Partial(CoverageIncompleteReason::UnsupportedSyntax)
        );
        assert!(ir
            .unknown_reasons
            .contains(&UnknownReason::UnsupportedSyntax));
        assert!(!ir
            .nodes
            .iter()
            .any(|node| node.kind == CapabilityKind::Operation));
    }

    #[test]
    fn l0_reads_manifest_feature_and_entrypoint_from_bounded_store() {
        let store = SourceStore::try_new(vec![
            SourceDocument::from_bytes(
                "Cargo.toml".into(),
                b"[package]\nname='x'\n[features]\njson=[]\n".to_vec(),
            ),
            SourceDocument::from_bytes("src/main.rs".into(), b"fn main() {}".to_vec()),
        ])
        .expect("store");
        let report = ProgramSourceReport {
            coverage: CoverageStatus::Complete,
            candidate_files: 2,
            selected_files: 2,
            selected_bytes: store.total_bytes(),
            skipped_existing: 0,
            issues: Vec::new(),
        };
        let ir = analyze_capabilities_l0(&store, &report, &ProgramAnalysisOptions::default());
        assert!(ir
            .nodes
            .iter()
            .any(|node| node.kind == CapabilityKind::Manifest));
        assert!(ir
            .nodes
            .iter()
            .any(|node| node.kind == CapabilityKind::Feature));
        assert!(ir
            .nodes
            .iter()
            .any(|node| node.kind == CapabilityKind::Entrypoint));
    }
}

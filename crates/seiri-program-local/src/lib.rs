#![forbid(unsafe_code)]

mod rust_frontend;

use rust_frontend::{
    analyze_rust_logical_items, RustByteSpan, RustCfgCondition, RustFrontendLimits,
    RustFrontendUnknownKind, RustLogicalItem, RustLogicalItemKind,
};

use seiri_core::{
    CapabilityDiagnostic, CapabilityEdge, CapabilityKind, CapabilityNode, CapabilityNodeId,
    CapabilityProvenance, CapabilityProvenanceKind, CapabilityRelation,
    CapabilitySemanticSignature, CapabilitySupport, ClaimPolarity, CoverageIncompleteReason,
    CoverageStatus, FileKind, FileRecord, RepositoryCapabilityIR, SourceDocument, SourceSpan,
    SourceStore, SourceStoreError, UnknownReason,
};
use std::collections::{BTreeMap, BTreeSet};
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
        let reason = issues
            .iter()
            .map(|issue| issue.reason)
            .min_by_key(|reason| unknown_rank(*reason))
            .unwrap_or(UnknownReason::LimitExceeded);
        CoverageStatus::Partial(coverage_reason(reason))
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
    let mut builder = CapabilityBuilder::new(source_report, *options);
    seed_path_capabilities(store, &mut builder);
    builder.finish()
}

#[must_use]
pub fn analyze_rust_capabilities(
    store: &SourceStore,
    source_report: &ProgramSourceReport,
    options: &ProgramAnalysisOptions,
) -> RepositoryCapabilityIR {
    let mut builder = CapabilityBuilder::new(source_report, *options);
    seed_path_capabilities(store, &mut builder);
    for source in store
        .documents()
        .iter()
        .filter(|source| source.path().to_ascii_lowercase().ends_with(".rs"))
    {
        let Some(text) = source.text() else {
            builder.unknown_at(source.path(), None, UnknownReason::InvalidUtf8);
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
        builder.unknown_at(source.path(), None, UnknownReason::InvalidUtf8);
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
    let limits = RustFrontendLimits {
        max_items: builder.options.max_nodes,
        ..RustFrontendLimits::default()
    };
    let report = analyze_rust_logical_items(text, &limits);
    let has_limit_diagnostic = report
        .unknowns
        .iter()
        .any(|unknown| frontend_unknown_reason(unknown.kind) == UnknownReason::LimitExceeded);
    for unknown in &report.unknowns {
        builder.unknown_at(
            path,
            rust_source_span(text, unknown.span),
            frontend_unknown_reason(unknown.kind),
        );
    }
    if report.truncated && !has_limit_diagnostic {
        builder.unknown_at(
            path,
            rust_source_span(
                text,
                RustByteSpan {
                    byte_start: 0,
                    byte_end: text.len(),
                },
            ),
            UnknownReason::LimitExceeded,
        );
    }

    for item in &report.items {
        let provenance_kind = if item.kind == RustLogicalItemKind::TestFunction {
            CapabilityProvenanceKind::TestPath
        } else {
            CapabilityProvenanceKind::SourceSyntax
        };
        let kind = match item.kind {
            RustLogicalItemKind::Function => Some(CapabilityKind::Operation),
            RustLogicalItemKind::TestFunction => Some(CapabilityKind::Test),
            RustLogicalItemKind::Reexport
            | RustLogicalItemKind::Struct
            | RustLogicalItemKind::Enum
            | RustLogicalItemKind::Trait
            | RustLogicalItemKind::TypeAlias
            | RustLogicalItemKind::Module
            | RustLogicalItemKind::Const
            | RustLogicalItemKind::Static => Some(CapabilityKind::PublicApi),
        };
        let Some(kind) = kind else {
            continue;
        };
        let names = if item.kind == RustLogicalItemKind::Reexport {
            reexported_names(&item.name)
        } else {
            vec![item.name.clone()]
        };
        if names.is_empty() {
            builder.unknown_at(
                path,
                rust_source_span(text, item.span),
                UnknownReason::UnsupportedSyntax,
            );
            continue;
        }
        for name in names {
            let item_id = builder.add_observed(
                kind,
                &name,
                path,
                provenance_kind,
                rust_source_span(text, item.name_span),
            );
            if let Some(item_id) = item_id {
                if item.kind == RustLogicalItemKind::Function {
                    add_logical_function_shape(path, text, item, item_id, builder);
                }
                add_cfg_conditions(path, text, item_id, &item.cfg_conditions, builder);
            }
        }
        if builder.limit_reached {
            break;
        }
    }
}

const fn frontend_unknown_reason(kind: RustFrontendUnknownKind) -> UnknownReason {
    match kind {
        RustFrontendUnknownKind::SourceLimitExceeded
        | RustFrontendUnknownKind::ItemLimitExceeded
        | RustFrontendUnknownKind::ItemTooLarge
        | RustFrontendUnknownKind::CfgLimitExceeded => UnknownReason::LimitExceeded,
        RustFrontendUnknownKind::UnsupportedItem => UnknownReason::ParseFailed,
        RustFrontendUnknownKind::UnsupportedMacro
        | RustFrontendUnknownKind::IncludeMacro
        | RustFrontendUnknownKind::UnterminatedBlockComment
        | RustFrontendUnknownKind::UnterminatedString
        | RustFrontendUnknownKind::UnterminatedRawString => UnknownReason::UnsupportedSyntax,
    }
}

fn rust_source_span(source: &str, span: RustByteSpan) -> Option<SourceSpan> {
    if span.byte_start > span.byte_end
        || span.byte_end > source.len()
        || !source.is_char_boundary(span.byte_start)
        || !source.is_char_boundary(span.byte_end)
    {
        return None;
    }
    let prefix = &source[..span.byte_start];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let line_start = prefix.rfind('\n').map_or(0, |index| index + 1);
    Some(SourceSpan::new(
        line,
        source[line_start..span.byte_start].chars().count() + 1,
        span.byte_start,
        span.byte_end,
    ))
}

fn add_logical_function_shape(
    path: &str,
    source: &str,
    item: &RustLogicalItem,
    function_id: CapabilityNodeId,
    builder: &mut CapabilityBuilder,
) {
    let Some(header) = source.get(item.span.byte_start..item.span.byte_end) else {
        builder.unknown_at(path, None, UnknownReason::ParseFailed);
        return;
    };
    let Some(open) = header.find('(') else {
        builder.unknown_at(
            path,
            rust_source_span(source, item.span),
            UnknownReason::ParseFailed,
        );
        return;
    };
    let Some(close) = matching_parenthesis(header, open) else {
        builder.unknown_at(
            path,
            rust_source_span(source, item.span),
            UnknownReason::ParseFailed,
        );
        return;
    };
    let parameters = &header[open + 1..close];
    let meaningful_input = parameters.split(',').map(str::trim).any(|parameter| {
        !parameter.is_empty() && !matches!(parameter, "self" | "&self" | "&mut self")
    });
    let name_span = rust_source_span(source, item.name_span);
    if meaningful_input {
        if let Some(input_id) = builder.add_observed(
            CapabilityKind::Input,
            &format!("{}::input", item.name),
            path,
            CapabilityProvenanceKind::SourceSyntax,
            name_span,
        ) {
            builder.add_edge(function_id, input_id, CapabilityRelation::Accepts);
        }
    }
    let after_close = &header[close + 1..];
    if let Some((_, output)) = after_close.split_once("->") {
        let output = output
            .split_once("where")
            .map_or(output, |(before, _)| before)
            .trim()
            .trim_end_matches(['{', ';'])
            .trim();
        if !output.is_empty() && output != "()" {
            if let Some(output_id) = builder.add_observed(
                CapabilityKind::Output,
                &format!("{}::output", item.name),
                path,
                CapabilityProvenanceKind::SourceSyntax,
                name_span,
            ) {
                builder.add_edge(function_id, output_id, CapabilityRelation::Produces);
            }
        }
    }
}

fn matching_parenthesis(value: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (offset, byte) in value.as_bytes()[open..].iter().copied().enumerate() {
        match byte {
            b'(' => depth = depth.saturating_add(1),
            b')' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(open + offset);
                }
            }
            _ => {}
        }
    }
    None
}

fn add_cfg_conditions(
    path: &str,
    source: &str,
    item_id: CapabilityNodeId,
    conditions: &[RustCfgCondition],
    builder: &mut CapabilityBuilder,
) {
    for condition in conditions {
        let span = rust_source_span(source, condition.span);
        if let Some(constraint_id) = builder.add_observed(
            CapabilityKind::Constraint,
            &format!("cfg:{}", condition.expression),
            path,
            CapabilityProvenanceKind::SourceSyntax,
            span,
        ) {
            builder.add_edge(item_id, constraint_id, CapabilityRelation::ConstrainedBy);
        }
        for feature in cfg_feature_names(&condition.expression) {
            if let Some(feature_id) = builder.add_observed(
                CapabilityKind::Feature,
                &feature,
                path,
                CapabilityProvenanceKind::SourceSyntax,
                span,
            ) {
                builder.add_edge(item_id, feature_id, CapabilityRelation::ConditionedBy);
            }
        }
    }
}

fn cfg_feature_names(expression: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut rest = expression;
    while let Some(index) = rest.find("feature") {
        rest = &rest[index + "feature".len()..];
        let Some(after_equals) = rest.split_once('=').map(|(_, after)| after.trim_start()) else {
            break;
        };
        let Some(quoted) = after_equals.strip_prefix('"') else {
            continue;
        };
        let Some(end) = quoted.find('"') else {
            break;
        };
        if end > 0 {
            names.push(quoted[..end].to_string());
        }
        rest = &quoted[end + 1..];
    }
    names.sort();
    names.dedup();
    names
}

fn reexported_names(target: &str) -> Vec<String> {
    let mut names = if let Some((prefix, group)) = target.split_once('{') {
        let Some(group) = group.strip_suffix('}') else {
            return Vec::new();
        };
        group
            .split(',')
            .filter_map(|entry| {
                let entry = entry.trim();
                if entry == "self" {
                    reexport_name(prefix.trim_end_matches(':'))
                } else {
                    reexport_name(entry)
                }
            })
            .collect::<Vec<_>>()
    } else {
        reexport_name(target).into_iter().collect::<Vec<_>>()
    };
    names.sort();
    names.dedup();
    names
}

fn reexport_name(target: &str) -> Option<String> {
    let target = target.trim();
    if target.is_empty() || target == "*" || target.ends_with("::*") {
        return None;
    }
    let visible = target
        .rsplit_once(" as ")
        .map_or(target, |(_, alias)| alias.trim());
    let name = visible.rsplit("::").next()?.trim().trim_start_matches("r#");
    (!name.is_empty()).then(|| name.to_string())
}

struct CapabilityBuilder {
    coverage: CoverageStatus,
    options: ProgramAnalysisOptions,
    nodes: Vec<CapabilityNode>,
    edges: Vec<CapabilityEdge>,
    edge_keys: BTreeSet<(CapabilityNodeId, CapabilityNodeId, u8)>,
    diagnostics: Vec<CapabilityDiagnostic>,
    keys: BTreeMap<(CapabilityKind, String, String, usize), CapabilityNodeId>,
    unknown_reasons: Vec<UnknownReason>,
    limit_reached: bool,
}

impl CapabilityBuilder {
    fn new(source_report: &ProgramSourceReport, options: ProgramAnalysisOptions) -> Self {
        let mut unknown_reasons = unknown_from_coverage(source_report.coverage)
            .into_iter()
            .chain(source_report.issues.iter().map(|issue| issue.reason))
            .collect::<Vec<_>>();
        unknown_reasons.sort_unstable_by_key(|reason| unknown_rank(*reason));
        unknown_reasons.dedup();
        Self {
            coverage: source_report.coverage,
            options,
            nodes: Vec::new(),
            edges: Vec::new(),
            edge_keys: BTreeSet::new(),
            diagnostics: source_report
                .issues
                .iter()
                .map(|issue| CapabilityDiagnostic {
                    path: issue.path.clone(),
                    span: None,
                    reason: issue.reason,
                })
                .collect(),
            keys: BTreeMap::new(),
            unknown_reasons,
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
        let identity_offset = match kind {
            CapabilityKind::PublicApi
            | CapabilityKind::Operation
            | CapabilityKind::Input
            | CapabilityKind::Output
            | CapabilityKind::FirstResult
            | CapabilityKind::Constraint
            | CapabilityKind::ComparisonReceipt => span.map_or(0, |value| value.byte_start + 1),
            CapabilityKind::Manifest
            | CapabilityKind::ProgramLanguage
            | CapabilityKind::Entrypoint
            | CapabilityKind::Example
            | CapabilityKind::Test
            | CapabilityKind::Feature => 0,
        };
        let key = (kind, symbol.to_string(), path.to_string(), identity_offset);
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
        let key = (from, to, relation_rank(relation));
        if self.edge_keys.contains(&key) {
            return;
        }
        if self.edges.len() >= self.options.max_edges {
            self.limit_reached = true;
            self.unknown(UnknownReason::LimitExceeded);
            return;
        }
        self.edge_keys.insert(key);
        self.edges.push(CapabilityEdge { from, to, relation });
    }

    fn unknown(&mut self, reason: UnknownReason) {
        if !self.unknown_reasons.contains(&reason) {
            self.unknown_reasons.push(reason);
        }
    }

    fn unknown_at(&mut self, path: &str, span: Option<SourceSpan>, reason: UnknownReason) {
        self.unknown(reason);
        self.diagnostics.push(CapabilityDiagnostic {
            path: path.to_string(),
            span,
            reason,
        });
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
        self.diagnostics.sort_by(|left, right| {
            capability_diagnostic_key(left).cmp(&capability_diagnostic_key(right))
        });
        self.diagnostics.dedup();
        let semantic_signatures = capability_semantic_signatures(&self.nodes, &self.edges);
        RepositoryCapabilityIR::try_new_with_semantic_signatures_and_diagnostics(
            self.coverage,
            self.nodes,
            self.edges,
            semantic_signatures,
            self.diagnostics,
            self.unknown_reasons,
        )
        .expect("builder maintains canonical capability IR")
    }
}

fn capability_semantic_signatures(
    nodes: &[CapabilityNode],
    edges: &[CapabilityEdge],
) -> Vec<CapabilitySemanticSignature> {
    nodes
        .iter()
        .filter(|node| node.kind == CapabilityKind::Operation)
        .filter_map(|node| {
            let (action, object) = operation_signature_terms(&node.symbol)?;
            let references = |relations: &[CapabilityRelation]| {
                let mut ids = edges
                    .iter()
                    .filter(|edge| edge.from == node.id && relations.contains(&edge.relation))
                    .map(|edge| edge.to)
                    .collect::<Vec<_>>();
                ids.sort_unstable();
                ids.dedup();
                ids
            };
            Some(CapabilitySemanticSignature {
                capability_node: node.id,
                dimension: node.kind.dimension(),
                subject: None,
                action: Some(action),
                object,
                qualifiers: Vec::new(),
                polarity: ClaimPolarity::Positive,
                input_nodes: references(&[CapabilityRelation::Accepts]),
                output_nodes: references(&[CapabilityRelation::Produces]),
                condition_nodes: references(&[
                    CapabilityRelation::ConditionedBy,
                    CapabilityRelation::ConstrainedBy,
                ]),
                semantic_support: match node.support {
                    CapabilitySupport::Observed | CapabilitySupport::Inferred => {
                        CapabilitySupport::Inferred
                    }
                    CapabilitySupport::Unknown(reason) => CapabilitySupport::Unknown(reason),
                },
            })
        })
        .collect()
}

fn operation_signature_terms(symbol: &str) -> Option<(String, Option<String>)> {
    let mut terms = symbol
        .trim_start_matches("r#")
        .split('_')
        .filter(|term| !term.is_empty());
    let action = canonical_operation_action(terms.next()?).to_string();
    let object = terms.collect::<Vec<_>>().join(" ");
    Some((action, (!object.is_empty()).then_some(object)))
}

fn canonical_operation_action(action: &str) -> &str {
    match action {
        "analyzes" | "analyzing" | "analysis" => "analyze",
        "audits" | "auditing" => "audit",
        "deletes" | "deleting" => "delete",
        "detects" | "detecting" => "detect",
        "generates" | "generating" => "generate",
        "inspects" | "inspecting" => "inspect",
        "organizes" | "organizing" => "organize",
        "reviews" | "reviewing" => "review",
        "scans" | "scanning" => "scan",
        other => other,
    }
}

fn capability_diagnostic_key(
    diagnostic: &CapabilityDiagnostic,
) -> (&str, usize, usize, usize, usize, u8) {
    let (line, column, byte_start, byte_end) = diagnostic.span.map_or((0, 0, 0, 0), |span| {
        (span.line, span.column, span.byte_start, span.byte_end)
    });
    (
        diagnostic.path.as_str(),
        line,
        column,
        byte_start,
        byte_end,
        unknown_rank(diagnostic.reason),
    )
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

const fn unknown_from_coverage(coverage: CoverageStatus) -> Option<UnknownReason> {
    match coverage {
        CoverageStatus::Complete => None,
        CoverageStatus::NotRequested => Some(UnknownReason::NotRequested),
        CoverageStatus::Partial(reason) => Some(match reason {
            CoverageIncompleteReason::LimitExceeded => UnknownReason::LimitExceeded,
            CoverageIncompleteReason::InvalidUtf8 => UnknownReason::InvalidUtf8,
            CoverageIncompleteReason::ParseFailed => UnknownReason::ParseFailed,
            CoverageIncompleteReason::UnsupportedSyntax => UnknownReason::UnsupportedSyntax,
            CoverageIncompleteReason::PermissionDenied => UnknownReason::PermissionDenied,
            CoverageIncompleteReason::RateLimited => UnknownReason::RateLimited,
            CoverageIncompleteReason::Unavailable => UnknownReason::Unavailable,
        }),
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

    #[test]
    fn duplicate_edge_does_not_consume_budget_or_create_false_unknown() {
        let report = ProgramSourceReport {
            coverage: CoverageStatus::Complete,
            candidate_files: 0,
            selected_files: 0,
            selected_bytes: 0,
            skipped_existing: 0,
            issues: Vec::new(),
        };
        let mut builder = CapabilityBuilder::new(
            &report,
            ProgramAnalysisOptions {
                max_nodes: 8,
                max_edges: 1,
            },
        );
        let manifest = builder
            .add_observed(
                CapabilityKind::Manifest,
                "Cargo.toml",
                "Cargo.toml",
                CapabilityProvenanceKind::Manifest,
                None,
            )
            .expect("manifest");
        let language = builder
            .add_observed(
                CapabilityKind::ProgramLanguage,
                "rust",
                "src/lib.rs",
                CapabilityProvenanceKind::SourceSyntax,
                None,
            )
            .expect("language");
        builder.add_edge(manifest, language, CapabilityRelation::Offers);
        builder.add_edge(manifest, language, CapabilityRelation::Offers);

        let ir = builder.finish();
        assert_eq!(ir.edges.len(), 1);
        assert_eq!(ir.coverage, CoverageStatus::Complete);
        assert!(!ir.unknown_reasons.contains(&UnknownReason::LimitExceeded));
    }

    #[test]
    fn rust_source_column_counts_unicode_scalars_not_utf8_bytes() {
        let source = "/*日*/ pub fn audit() {}";
        let byte_start = source.find("pub").expect("public function");
        let span = rust_source_span(
            source,
            RustByteSpan {
                byte_start,
                byte_end: byte_start + "pub".len(),
            },
        )
        .expect("source span");
        assert_eq!(span.line, 1);
        assert_eq!(span.column, 7);
        assert_eq!(span.byte_start, 8);
    }
}

#![forbid(unsafe_code)]

use seiri_core::{
    calibrate_content_claim, project_content_claim, stable_id, ClaimDraftPlanState, ClaimMode,
    ClaimStrength, CodexAction, CodexCommand, ContentClaim, ContentClaimProjection,
    CoverageIncompleteReason, CoverageIndex, CoverageScope, CoverageStatus,
    DocumentConsistencyReport, DocumentIndex, DocumentSelectionSummary, EvidenceKernel,
    FacetReport, FreshnessReport, GithubLocalDocuments, GithubSemanticsReport,
    MissingRoutePriorityReport, Observation, PatchPlan, ProfileKind, RemoteEvidenceReport,
    RepositoryAnalysis, RepositoryScopeReport, RouteAssessment, RouteContentReport, UnknownReason,
    WordingLintReport, CODEX_SCHEMA_VERSION, CONTRACT_SCHEMA_VERSION, PATCH_PLAN_SCHEMA_VERSION,
    PORTABLE_AUDIT_SCHEMA_VERSION,
};
use serde::Serialize;
use std::fmt::{Display, Formatter};
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CodexQueryKind {
    Summary,
    Routes,
    Evidence,
    Documents,
    Governance,
    Patches,
    Linter,
    Actions,
    Remote,
    PrBody,
}

impl CodexQueryKind {
    pub const ALL: [Self; 10] = [
        Self::Summary,
        Self::Routes,
        Self::Evidence,
        Self::Documents,
        Self::Governance,
        Self::Patches,
        Self::Linter,
        Self::Actions,
        Self::Remote,
        Self::PrBody,
    ];

    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Summary => "summary",
            Self::Routes => "routes",
            Self::Evidence => "evidence",
            Self::Documents => "documents",
            Self::Governance => "governance",
            Self::Patches => "patches",
            Self::Linter => "linter",
            Self::Actions => "actions",
            Self::Remote => "remote",
            Self::PrBody => "pr-body",
        }
    }
}

impl Display for CodexQueryKind {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.slug())
    }
}

impl FromStr for CodexQueryKind {
    type Err = CodexQueryParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.slug() == value)
            .ok_or_else(|| CodexQueryParseError {
                value: value.to_string(),
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexQueryParseError {
    value: String,
}

impl Display for CodexQueryParseError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "unknown Codex query `{}`; expected one of: {}",
            self.value,
            CodexQueryKind::ALL
                .iter()
                .map(|kind| kind.slug())
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

impl std::error::Error for CodexQueryParseError {}

#[derive(Debug)]
pub struct CodexView<'a> {
    analysis: &'a RepositoryAnalysis,
    plan: &'a PatchPlan,
    wording_lint: Option<&'a WordingLintReport>,
}

impl<'a> CodexView<'a> {
    #[must_use]
    pub const fn new(
        analysis: &'a RepositoryAnalysis,
        plan: &'a PatchPlan,
        wording_lint: Option<&'a WordingLintReport>,
    ) -> Self {
        Self {
            analysis,
            plan,
            wording_lint,
        }
    }

    #[must_use]
    pub fn query(&self, kind: CodexQueryKind) -> CodexQueryView<'_> {
        let query = match kind {
            CodexQueryKind::Summary => {
                CodexQuery::Summary(Box::new(summary(self.analysis, self.plan)))
            }
            CodexQueryKind::Routes => CodexQuery::Routes(CodexRoutesQuery {
                assessments: &self.analysis.route_assessments,
                priorities: &self.analysis.missing_route_priority,
            }),
            CodexQueryKind::Evidence => CodexQuery::Evidence(CodexEvidenceQuery {
                kernel: &self.analysis.evidence_kernel,
                coverage: &self.analysis.coverage,
            }),
            CodexQueryKind::Documents => CodexQuery::Documents(CodexDocumentsQuery {
                index: &self.analysis.document_index,
                github: &self.analysis.github_local_documents,
            }),
            CodexQueryKind::Governance => CodexQuery::Governance(CodexGovernanceQuery {
                facets: &self.analysis.facets,
                route_content: &self.analysis.route_content,
                consistency: &self.analysis.document_consistency,
                github: &self.analysis.github_semantics,
                scope: &self.analysis.repository_scope,
                freshness: &self.analysis.freshness,
                claims: &self.analysis.claims,
                readme_grammar: &self.analysis.readme_grammar,
                repository_capabilities: &self.analysis.repository_capabilities,
                value_coverage: &self.analysis.value_coverage,
                claim_capability_membrane: &self.analysis.claim_capability_membrane,
                claim_projections: self
                    .analysis
                    .claims
                    .iter()
                    .map(project_content_claim)
                    .collect(),
            }),
            CodexQueryKind::Patches => CodexQuery::Patches(self.plan),
            CodexQueryKind::Linter => CodexQuery::Linter(CodexLinterQuery {
                report: self.wording_lint,
                boundary: linter_boundary(),
            }),
            CodexQueryKind::Actions => CodexQuery::Actions(build_actions(self.analysis)),
            CodexQueryKind::Remote => CodexQuery::Remote(&self.analysis.remote_evidence),
            CodexQueryKind::PrBody => CodexQuery::PrBody(build_pr_body(self.analysis, self.plan)),
        };
        CodexQueryView {
            schema_version: CODEX_SCHEMA_VERSION,
            repo_root: ".",
            profile: self.analysis.profile.as_ref().map(|profile| profile.profile),
            query,
            boundary: "Codex queries are bounded projections of canonical local analysis. They do not write files, execute commands, call GitHub, adopt policy, or guarantee popularity, trust, security, quality, or publication readiness.",
        }
    }
}

#[derive(Debug, Serialize)]
pub struct CodexQueryView<'a> {
    pub schema_version: &'static str,
    pub repo_root: &'a str,
    pub profile: Option<ProfileKind>,
    pub query: CodexQuery<'a>,
    pub boundary: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum CodexQuery<'a> {
    Summary(Box<CodexSummary>),
    Routes(CodexRoutesQuery<'a>),
    Evidence(CodexEvidenceQuery<'a>),
    Documents(CodexDocumentsQuery<'a>),
    Governance(CodexGovernanceQuery<'a>),
    Patches(&'a PatchPlan),
    Linter(CodexLinterQuery<'a>),
    Actions(Vec<CodexAction>),
    Remote(&'a RemoteEvidenceReport),
    PrBody(CodexPrBody),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CodexSummary {
    pub contract_schema_version: &'static str,
    pub portable_audit_schema_version: &'static str,
    pub patch_plan_schema_version: &'static str,
    pub source_session_digest: seiri_core::SourceSessionDigest,
    pub entries_scanned: usize,
    pub document_events: usize,
    pub document_diagnostics: usize,
    pub evidence_facts: usize,
    pub route_assessments: usize,
    pub route_content_slots: usize,
    pub readme_grammar_nodes: usize,
    pub repository_capability_nodes: usize,
    pub program_unknown_reasons: usize,
    pub underclaim_opportunities: usize,
    pub overclaim_risks: usize,
    pub claims: usize,
    pub findings: usize,
    pub pattern_matches: usize,
    pub profile_fit_score_x100: Option<u8>,
    pub profile_branches: usize,
    pub top_profile: Option<ProfileKind>,
    pub top_profile_rank_score_x100: Option<u8>,
    pub missing_route_priorities: usize,
    pub review_priorities: usize,
    pub top_review_route: Option<seiri_core::RouteKind>,
    pub top_review_authority: Option<seiri_core::ReviewAuthority>,
    pub top_review_recommendation: Option<&'static str>,
    pub patch_operations: usize,
    pub patch_holds: usize,
    pub claim_draft_state: ClaimDraftPlanState,
    pub claim_drafts: usize,
    pub claim_draft_baseline_unknown_count: usize,
    pub maximum_claim_draft_ceiling: ClaimMode,
    pub writes_files: bool,
    pub documents: DocumentSelectionSummary,
    pub coverage: CodexCoverageSummary,
    pub observations: CodexObservationSummary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CodexCoverageSummary {
    pub complete_scopes: usize,
    pub partial_scopes: usize,
    pub not_requested_scopes: usize,
    pub limit_exceeded_scopes: usize,
    pub markdown_documents: CoverageStatus,
    pub conflict_coverage: CoverageStatus,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct CodexObservationSummary {
    pub present: usize,
    pub absent: usize,
    pub unknown: usize,
    pub unacknowledged_unknown: usize,
    pub conflict: usize,
    pub limit_exceeded: usize,
}

#[derive(Debug, Serialize)]
pub struct CodexRoutesQuery<'a> {
    pub assessments: &'a [RouteAssessment],
    pub priorities: &'a MissingRoutePriorityReport,
}

#[derive(Debug, Serialize)]
pub struct CodexEvidenceQuery<'a> {
    pub kernel: &'a EvidenceKernel,
    pub coverage: &'a CoverageIndex,
}

#[derive(Debug, Serialize)]
pub struct CodexDocumentsQuery<'a> {
    pub index: &'a DocumentIndex,
    pub github: &'a GithubLocalDocuments,
}

#[derive(Debug, Serialize)]
pub struct CodexGovernanceQuery<'a> {
    pub facets: &'a FacetReport,
    pub route_content: &'a RouteContentReport,
    pub consistency: &'a DocumentConsistencyReport,
    pub github: &'a GithubSemanticsReport,
    pub scope: &'a RepositoryScopeReport,
    pub freshness: &'a FreshnessReport,
    pub claims: &'a [ContentClaim],
    pub readme_grammar: &'a seiri_core::ReadmeGrammarIR,
    pub repository_capabilities: &'a seiri_core::RepositoryCapabilityIR,
    pub value_coverage: &'a seiri_core::ReadmeValueCoverageReport,
    pub claim_capability_membrane: &'a seiri_core::ClaimCapabilityMembrane,
    pub claim_projections: Vec<ContentClaimProjection>,
}

#[derive(Debug, Serialize)]
pub struct CodexLinterQuery<'a> {
    pub report: Option<&'a WordingLintReport>,
    pub boundary: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CodexPrBody {
    pub title: String,
    pub body: String,
    pub draft: bool,
}

fn summary(analysis: &RepositoryAnalysis, plan: &PatchPlan) -> CodexSummary {
    let (document_events, document_diagnostics) = analysis
        .document_index
        .scanned_documents()
        .filter_map(|entry| entry.scan.as_ref())
        .fold((0usize, 0usize), |(events, diagnostics), document| {
            (
                events.saturating_add(document.events().len()),
                diagnostics.saturating_add(document.diagnostics().len()),
            )
        });
    let mut observations = CodexObservationSummary::default();
    for assessment in &analysis.route_content.assessments {
        observations.record(&assessment.observation);
    }
    for assessment in &analysis.facets.facets {
        observations.record(&assessment.observation);
    }
    for obligation in &analysis.document_consistency.obligations {
        observations.record(&obligation.observation);
    }
    let coverage = coverage_summary(analysis);
    let top_review = analysis.review_priority.priorities.first();
    CodexSummary {
        contract_schema_version: CONTRACT_SCHEMA_VERSION,
        portable_audit_schema_version: PORTABLE_AUDIT_SCHEMA_VERSION,
        patch_plan_schema_version: PATCH_PLAN_SCHEMA_VERSION,
        source_session_digest: analysis.analysis_configuration.source_session_digest,
        entries_scanned: analysis.entry_count,
        document_events,
        document_diagnostics,
        evidence_facts: analysis.evidence_kernel.len(),
        route_assessments: analysis.route_assessments.len(),
        route_content_slots: analysis.route_content.assessments.len(),
        readme_grammar_nodes: analysis.readme_grammar.nodes.len(),
        repository_capability_nodes: analysis.repository_capabilities.nodes.len(),
        program_unknown_reasons: analysis.repository_capabilities.unknown_reasons.len(),
        underclaim_opportunities: analysis.claim_capability_membrane.opportunities.len(),
        overclaim_risks: analysis.claim_capability_membrane.risks.len(),
        claims: analysis.claims.len(),
        findings: analysis.findings.len(),
        pattern_matches: analysis.pattern_matches.len(),
        profile_fit_score_x100: analysis
            .profile
            .as_ref()
            .map(|profile| profile.score.score_x100),
        profile_branches: analysis
            .profile
            .as_ref()
            .map_or(0, |profile| profile.branches.len()),
        top_profile: analysis
            .profile
            .as_ref()
            .and_then(|profile| profile.branch_summary.top_profile),
        top_profile_rank_score_x100: analysis
            .profile
            .as_ref()
            .and_then(|profile| profile.branch_summary.top_rank_score_x100),
        missing_route_priorities: analysis.missing_route_priority.priorities.len(),
        review_priorities: analysis.review_priority.priorities.len(),
        top_review_route: top_review.and_then(|priority| priority.gap.route()),
        top_review_authority: top_review.map(seiri_core::ReviewPriority::authority),
        top_review_recommendation: top_review.map(seiri_core::ReviewPriority::recommendation),
        patch_operations: plan.operations.len(),
        patch_holds: plan.held.len(),
        claim_draft_state: plan.claim_draft_state,
        claim_drafts: plan.claim_drafts.drafts.len(),
        claim_draft_baseline_unknown_count: plan.claim_drafts.baseline_unknown_count,
        maximum_claim_draft_ceiling: plan
            .claim_drafts
            .drafts
            .iter()
            .map(|draft| draft.claim_ceiling)
            .max()
            .unwrap_or(ClaimMode::Omitted),
        writes_files: plan.writes_files,
        documents: analysis.document_index.selection(),
        coverage,
        observations,
    }
}

impl CodexObservationSummary {
    fn record<T>(&mut self, observation: &Observation<T>) {
        match observation {
            Observation::Present { .. } => self.present = self.present.saturating_add(1),
            Observation::Absent { .. } => self.absent = self.absent.saturating_add(1),
            Observation::Unknown(reason) => {
                self.unknown = self.unknown.saturating_add(1);
                if *reason != UnknownReason::NotRequested {
                    self.unacknowledged_unknown = self.unacknowledged_unknown.saturating_add(1);
                }
                if *reason == UnknownReason::LimitExceeded {
                    self.limit_exceeded = self.limit_exceeded.saturating_add(1);
                }
            }
            Observation::Conflict { .. } => self.conflict = self.conflict.saturating_add(1),
        }
    }
}

fn coverage_summary(analysis: &RepositoryAnalysis) -> CodexCoverageSummary {
    let mut complete_scopes = 0usize;
    let mut partial_scopes = 0usize;
    let mut not_requested_scopes = 0usize;
    let mut limit_exceeded_scopes = 0usize;
    for record in analysis.coverage.records() {
        match record.status {
            CoverageStatus::Complete => complete_scopes = complete_scopes.saturating_add(1),
            CoverageStatus::Partial(reason) => {
                partial_scopes = partial_scopes.saturating_add(1);
                if reason == CoverageIncompleteReason::LimitExceeded {
                    limit_exceeded_scopes = limit_exceeded_scopes.saturating_add(1);
                }
            }
            CoverageStatus::NotRequested => {
                not_requested_scopes = not_requested_scopes.saturating_add(1);
            }
        }
    }
    CodexCoverageSummary {
        complete_scopes,
        partial_scopes,
        not_requested_scopes,
        limit_exceeded_scopes,
        markdown_documents: analysis
            .coverage
            .record(CoverageScope::MarkdownDocuments)
            .map_or(CoverageStatus::NotRequested, |record| record.status),
        conflict_coverage: analysis.document_consistency.conflict_coverage,
    }
}

fn build_actions(analysis: &RepositoryAnalysis) -> Vec<CodexAction> {
    let profile = analysis
        .profile
        .as_ref()
        .map_or(ProfileKind::Common, |profile| profile.profile)
        .to_string();
    [
        ("Render audit report", "audit", None),
        ("Render dry-run patch plan", "plan", None),
        ("Render Codex PR body", "codex", Some("pr-body")),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (label, subcommand, query))| {
        let mut args = vec![
            subcommand.to_string(),
            "--path".to_string(),
            ".".to_string(),
            "--profile".to_string(),
            profile.clone(),
            "--format".to_string(),
            "markdown".to_string(),
        ];
        if let Some(query) = query {
            args.extend(["--query".to_string(), query.to_string()]);
        }
        CodexAction {
            id: stable_id("codex-action", index + 1),
            label: label.to_string(),
            command: CodexCommand::new("seiri", args).expect("built-in argv is valid"),
            runtime: seiri_core::CodexRuntimeRequirement::default(),
            mutates_files: false,
            requires_confirmation: false,
            detail: "Review command only; RepoSeiri does not execute this argv.".to_string(),
        }
    })
    .collect()
}

fn build_pr_body(analysis: &RepositoryAnalysis, plan: &PatchPlan) -> CodexPrBody {
    let summary = summary(analysis, plan);
    let observed_claims = analysis
        .claims
        .iter()
        .filter(|claim| claim.strength() == ClaimStrength::Observed)
        .count();
    let examples = analysis
        .claims
        .iter()
        .filter(|claim| claim.strength() == ClaimStrength::Observed)
        .take(3)
        .map(|claim| {
            format!(
                "- {}",
                calibrate_content_claim(claim).assertion.render_sentence()
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let body = format!(
        "## Summary\n\n- Reviewed {} repository entries and {} typed evidence facts.\n- Recorded {} route assessments, {} content slots, and {} ordered review items.\n- Prepared {} dry-run patch operations; {} items remain held.\n\n## Evidence-backed observations\n\nThe audit emitted {observed_claims} observed claims. Examples:\n\n{examples}\n\n## Boundaries\n\nRepoSeiri did not write files, execute commands, call GitHub, create policy text, or establish popularity, trust, security, quality, or publication readiness.\n",
        summary.entries_scanned,
        summary.evidence_facts,
        summary.route_assessments,
        summary.route_content_slots,
        summary.review_priorities,
        summary.patch_operations,
        summary.patch_holds,
    );
    CodexPrBody {
        title: "Organize repository routes with RepoSeiri".to_string(),
        body,
        draft: true,
    }
}

fn linter_boundary() -> &'static str {
    "Wording findings are evidence-scoped review hints, not legal, security, quality, trust, or publication-readiness judgments."
}

#[must_use]
pub fn render_query_markdown(view: &CodexQueryView<'_>) -> String {
    let mut out = format!(
        "# RepoSeiri Codex Query\n\n- Schema: `{}`\n- Repository: `{}`\n- Query: `{}`\n",
        view.schema_version,
        view.repo_root,
        query_kind(&view.query).slug(),
    );
    match &view.query {
        CodexQuery::Summary(summary) => {
            out.push_str(&format!(
                "\n- Contract schema: `{}`; portable audit schema: `{}`; patch-plan schema: `{}`\n- Source session digest: `{}`\n- Entries: `{}`\n- Document events: `{}`; diagnostics: `{}`\n- Evidence facts: `{}`\n- Route assessments: `{}`\n- Content slots: `{}`\n- README grammar nodes: `{}`\n- Repository capability nodes: `{}`; program unknown reasons: `{}`\n- Underclaim opportunities: `{}`; overclaim risks: `{}`\n- Claims: `{}`; findings: `{}`; pattern matches: `{}`\n- Profile fit score x100: `{:?}`; branches: `{}`; top profile `{:?}` / rank `{:?}`\n- Missing route priorities: `{}`\n- Documents: `{}` selected / `{}` candidates; primary `{}` / `{}`\n- Document budget skips: `{}`; byte budget skips: `{}`; selected bytes: `{}`\n- Primary document budget skips: `{}`; byte budget skips: `{}`; selected bytes: `{}`\n- Coverage: `{}` complete / `{}` partial / `{}` not requested; limit exceeded `{}`\n- Markdown coverage: `{:?}`; conflict coverage: `{:?}`\n- Observations: `{}` present / `{}` absent / `{}` unknown (`{}` unacknowledged; `{}` limit-exceeded) / `{}` conflict\n- Review priorities: `{}`; top route `{:?}` / authority `{:?}`\n- Top recommendation: {}\n- Patch operations: `{}`\n- Patch holds: `{}`\n- Claim draft state: `{:?}`\n- Claim drafts: `{}`; baseline unknown: `{}`; maximum claim ceiling: `{:?}`\n- Writes files: `{}`\n",
                summary.contract_schema_version,
                summary.portable_audit_schema_version,
                summary.patch_plan_schema_version,
                summary.source_session_digest.get(),
                summary.entries_scanned,
                summary.document_events,
                summary.document_diagnostics,
                summary.evidence_facts,
                summary.route_assessments,
                summary.route_content_slots,
                summary.readme_grammar_nodes,
                summary.repository_capability_nodes,
                summary.program_unknown_reasons,
                summary.underclaim_opportunities,
                summary.overclaim_risks,
                summary.claims,
                summary.findings,
                summary.pattern_matches,
                summary.profile_fit_score_x100,
                summary.profile_branches,
                summary.top_profile,
                summary.top_profile_rank_score_x100,
                summary.missing_route_priorities,
                summary.documents.selected,
                summary.documents.candidates,
                summary.documents.primary_selected,
                summary.documents.primary_candidates,
                summary.documents.skipped_document_budget,
                summary.documents.skipped_byte_budget,
                summary.documents.selected_source_bytes,
                summary.documents.primary_skipped_document_budget,
                summary.documents.primary_skipped_byte_budget,
                summary.documents.primary_selected_source_bytes,
                summary.coverage.complete_scopes,
                summary.coverage.partial_scopes,
                summary.coverage.not_requested_scopes,
                summary.coverage.limit_exceeded_scopes,
                summary.coverage.markdown_documents,
                summary.coverage.conflict_coverage,
                summary.observations.present,
                summary.observations.absent,
                summary.observations.unknown,
                summary.observations.unacknowledged_unknown,
                summary.observations.limit_exceeded,
                summary.observations.conflict,
                summary.review_priorities,
                summary.top_review_route,
                summary.top_review_authority,
                summary.top_review_recommendation.unwrap_or("No bounded review item."),
                summary.patch_operations,
                summary.patch_holds,
                summary.claim_draft_state,
                summary.claim_drafts,
                summary.claim_draft_baseline_unknown_count,
                summary.maximum_claim_draft_ceiling,
                summary.writes_files,
            ));
        }
        CodexQuery::Routes(routes) => {
            out.push_str("\n## Routes\n");
            for assessment in routes.assessments {
                let state = assessment.summary_projection();
                let axes = assessment.axes();
                out.push_str(&format!(
                    "- `{:?}`: `{:?}`; artifact(root={}, inherited={}), entrypoint={}, local-targets={}, freshness=`{:?}`, conflicts={}, policy=`{:?}`\n",
                    assessment.route(),
                    state.state,
                    axes.artifact.root_structured(),
                    axes.artifact.inherited(),
                    axes.entrypoint.is_present(),
                    axes.reachability.repository_local_present(),
                    axes.freshness,
                    axes.conflict.shared_target_count(),
                    axes.policy,
                ));
                out.push_str(&format!("  Reason: {}\n", state.reason));
            }
            out.push_str("\n## Missing Route Priorities\n");
            out.push_str(&format!(
                "\n- Candidates: `{}`; co-occurrence gaps: `{}`; top route: `{:?}`; top priority x100: `{:?}`\n- Gates: `{}` safe / `{}` guarded / `{}` manual\n",
                routes.priorities.summary.candidates,
                routes.priorities.summary.co_occurrence_gaps,
                routes.priorities.summary.top_route,
                routes.priorities.summary.top_priority_x100,
                routes.priorities.summary.safe_gated,
                routes.priorities.summary.guarded_gated,
                routes.priorities.summary.manual_gated,
            ));
            for priority in &routes.priorities.priorities {
                out.push_str(&format!(
                    "- Rank `{}` `{:?}`: state `{:?}`, gate `{:?}`, severity `{:?}`, priority `{:?}` / score `{}`. Reason: {}\n",
                    priority.rank,
                    priority.route,
                    priority.state,
                    priority.gate,
                    priority.severity,
                    priority.priority,
                    priority.priority_score_x100,
                    priority.reason,
                ));
            }
            out.push_str(&format!(
                "- Priority boundary: {}\n",
                routes.priorities.boundary
            ));
        }
        CodexQuery::Evidence(evidence) => {
            out.push_str(&format!(
                "\n- Documents: `{}`\n- Facts: `{}`\n",
                evidence.kernel.documents().len(),
                evidence.kernel.facts().len(),
            ));
            out.push_str("\n## Coverage\n");
            for record in evidence.coverage.records() {
                out.push_str(&format!(
                    "- Scope `{:?}`: `{:?}`\n",
                    record.scope, record.status,
                ));
            }
            out.push_str("\n## Evidence Facts\n");
            for fact in evidence.kernel.facts() {
                out.push_str(&format!(
                    "- `{:?}`: atom `{:?}`, confidence `{:?}`, domain `{:?}`, producer `{:?}`, document `{:?}`, span `{:?}`\n",
                    fact.id,
                    fact.atom,
                    fact.confidence,
                    fact.provenance.domain,
                    fact.provenance.producer,
                    fact.provenance.document,
                    fact.provenance.span,
                ));
            }
        }
        CodexQuery::Documents(documents) => {
            out.push_str(&format!(
                "\n- Indexed documents: `{}`\n- Structured GitHub documents: `{}`\n",
                documents.index.entries().len(),
                documents.github.documents().len(),
            ));
            out.push_str("\n## Document States\n");
            for document in documents.index.entries() {
                out.push_str(&format!(
                    "- `{}`: role `{:?}`, status `{:?}`, declared bytes `{}`, encoding `{:?}`\n",
                    document.path,
                    document.role,
                    document.status,
                    document.declared_bytes,
                    document.encoding,
                ));
            }
            out.push_str("\n## Structured GitHub Document States\n");
            for document in documents.github.documents() {
                out.push_str(&format!(
                    "- `{}`: kind `{:?}`, status `{:?}`, diagnostics `{}`\n",
                    document.path,
                    document.kind,
                    document.status,
                    document.diagnostics.len(),
                ));
            }
        }
        CodexQuery::Governance(governance) => {
            out.push_str(&format!(
                "\n- Facets: `{}`\n- Content slots: `{}`\n- Target conflicts: `{}`\n- Proposition conflicts: `{}`\n- Claims: `{}`\n- README grammar coverage: `{:?}`; nodes: `{}`; diagnostics: `{}`\n- Translation sections: `{}`; alignments: `{}`; Unknown reasons: `{:?}`\n- Repository capability coverage: `{:?}`; nodes: `{}`; diagnostics: `{}`; Unknown reasons: `{:?}`\n- Underclaim opportunities: `{}`\n- Overclaim risks: `{}`\n\n## Value Coverage\n",
                governance.facets.facets.len(),
                governance.route_content.assessments.len(),
                governance.consistency.conflicts.len(),
                governance.consistency.proposition_conflicts.len(),
                governance.claims.len(),
                governance.readme_grammar.coverage,
                governance.readme_grammar.nodes.len(),
                governance.readme_grammar.diagnostics.len(),
                governance
                    .readme_grammar
                    .translation_alignment
                    .sections
                    .len(),
                governance
                    .readme_grammar
                    .translation_alignment
                    .alignments
                    .len(),
                governance
                    .readme_grammar
                    .translation_alignment
                    .unknown_reasons,
                governance.repository_capabilities.coverage,
                governance.repository_capabilities.nodes.len(),
                governance.repository_capabilities.diagnostics.len(),
                governance.repository_capabilities.unknown_reasons,
                governance.claim_capability_membrane.opportunities.len(),
                governance.claim_capability_membrane.risks.len(),
            ));
            for (dimension, state) in &governance.value_coverage.dimensions {
                out.push_str(&format!("- `{:?}`: `{:?}`\n", dimension, state));
            }

            out.push_str("\n## Claim-Capability Support\n");
            for relation in &governance.claim_capability_membrane.relations {
                out.push_str(&format!(
                    "- `{:?}`: state `{:?}`, evidence `{}`, grammar nodes `{:?}`, capability nodes `{:?}`\n",
                    relation.dimension,
                    relation.state,
                    relation.evidence_count,
                    relation.grammar_nodes,
                    relation.capability_nodes,
                ));
            }
            for alignment in &governance.claim_capability_membrane.alignments {
                out.push_str(&format!(
                    "- Claim `{:?}`: state `{:?}`, evidence `{}`, ceiling `{:?}`, capability nodes `{:?}`\n",
                    alignment.claim_atom,
                    alignment.state,
                    alignment.evidence_count,
                    alignment.claim_ceiling,
                    alignment.capability_nodes,
                ));
            }

            out.push_str("\n## Underclaim Opportunities\n");
            for opportunity in &governance.claim_capability_membrane.opportunities {
                out.push_str(&format!(
                    "- `{:?}` / `{:?}`: gate `{:?}`, grammar nodes `{:?}`, capability nodes `{:?}`\n",
                    opportunity.dimension,
                    opportunity.kind,
                    opportunity.gate,
                    opportunity.grammar_nodes,
                    opportunity.capability_nodes,
                ));
            }
            out.push_str("\n## Overclaim Risks\n");
            for risk in &governance.claim_capability_membrane.risks {
                out.push_str(&format!(
                    "- `{:?}` / `{:?}`: grammar nodes `{:?}`\n",
                    risk.dimension, risk.kind, risk.grammar_nodes,
                ));
            }

            out.push_str("\n## Evidence-Backed Claims\n");
            for claim in governance.claims {
                let projection = calibrate_content_claim(claim);
                let boundaries = projection
                    .boundaries
                    .iter()
                    .map(|boundary| format!("`{boundary:?}`"))
                    .collect::<Vec<_>>()
                    .join(", ");
                out.push_str(&format!(
                    "- `{}`: {} Evidence: `{}`. Claim-local boundaries: {}.\n",
                    claim.id(),
                    projection.assertion.render_sentence(),
                    claim.evidence_ids().len(),
                    boundaries,
                ));
            }
        }
        CodexQuery::Patches(plan) => {
            out.push_str(&format!(
                "\n- Patch schema: `{}`\n- Edit-existing previews: `{}`\n- Create-skeleton review items: `{}`\n- Manual decisions: `{}`\n- Appeal suggestions: `{}` (`{}` safe / `{}` guarded / `{}` manual)\n- Held items: `{}`\n- Claim draft state: `{:?}`\n- Claim drafts: `{}`; baseline unknown: `{}`\n- Writes files: `{}`\n- Plan boundary: {}\n",
                plan.schema_version,
                plan.proposal_count(seiri_core::PatchProposalKind::EditExisting),
                plan.proposal_count(seiri_core::PatchProposalKind::CreateSkeleton),
                plan.proposal_count(seiri_core::PatchProposalKind::ManualDecision),
                plan.appeal_suggestions.len(),
                plan.appeal_suggestion_count(seiri_core::GateKind::Safe),
                plan.appeal_suggestion_count(seiri_core::GateKind::Guarded),
                plan.appeal_suggestion_count(seiri_core::GateKind::Manual),
                plan.held.len(),
                plan.claim_draft_state,
                plan.claim_drafts.drafts.len(),
                plan.claim_drafts.baseline_unknown_count,
                plan.writes_files,
                plan.boundary,
            ));
            out.push_str("\n## Patch Holds\n");
            for hold in &plan.held {
                out.push_str(&format!(
                    "- `{:?}` at `{:?}`: reason `{:?}`, gate `{:?}`\n",
                    hold.route, hold.target_path, hold.reason, hold.decision_basis.gate,
                ));
            }
            out.push_str("\n## Appeal Suggestions\n");
            for suggestion in &plan.appeal_suggestions {
                out.push_str(&format!(
                    "- `{}` `{:?}` / `{:?}`: gate `{:?}`, support `{:?}`, ceiling `{:?}`\n",
                    suggestion.id,
                    suggestion.dimension,
                    suggestion.opportunity,
                    suggestion.gate,
                    suggestion.support_state,
                    suggestion.claim_ceiling,
                ));
            }
            out.push_str("\n## Claim Drafts\n");
            if !plan.claim_drafts.is_empty() {
                out.push_str(&format!(
                    "- Source: `{}`; digest `{}`; bytes `{}`; baseline unknown `{}`\n",
                    plan.claim_drafts.path,
                    plan.claim_drafts.source_digest,
                    plan.claim_drafts.source_byte_len,
                    plan.claim_drafts.baseline_unknown_count,
                ));
            }
            for draft in &plan.claim_drafts.drafts {
                out.push_str(&format!(
                    "- Draft `{:?}`: dimension `{:?}`, polarity `{:?}`, modality `{:?}`, realization `{:?}`, ceiling `{:?}`, source claims `{:?}`, capability nodes `{:?}`, target span `{:?}`\n",
                    draft.id,
                    draft.semantics.dimension,
                    draft.semantics.polarity,
                    draft.semantics.modality,
                    draft.realization_mode,
                    draft.claim_ceiling,
                    draft.source_claims,
                    draft.capability_nodes,
                    draft.target_span,
                ));
            }
        }
        CodexQuery::Linter(linter) => {
            out.push_str(&format!(
                "\n- Available: `{}`\n- Findings: `{}`\n- Linter boundary: {}\n",
                linter.report.is_some(),
                linter.report.map_or(0, |report| report.findings.len()),
                linter.boundary,
            ));
            if let Some(report) = linter.report {
                out.push_str(&format!(
                    "- Schema: `{}`; files scanned: `{}`; generated surfaces: `{}`; suppressed boundary exceptions: `{}`\n- Report boundary: {}\n",
                    report.schema_version,
                    report.summary.files_scanned,
                    report.summary.generated_surfaces,
                    report.summary.suppressed_boundary_exceptions,
                    report.boundary,
                ));
                for finding in &report.findings {
                    out.push_str(&format!(
                        "- `{}` at `{}:{}:{}` bytes `{}..{}`: rule `{:?}`, boundary `{:?}`, source `{:?}`\n",
                        finding.id,
                        finding.path,
                        finding.line,
                        finding.column,
                        finding.byte_start,
                        finding.byte_end,
                        finding.rule,
                        finding.boundary,
                        finding.source,
                    ));
                }
            }
        }
        CodexQuery::Actions(actions) => {
            out.push_str("\n## Review Commands\n");
            for action in actions {
                out.push_str(&format!(
                    "- `{}` {}: `{}` {:?}; runtime `{:?}`; mutates files `{}`; requires confirmation `{}`. {}\n",
                    action.id,
                    action.label,
                    action.command.program(),
                    action.command.args(),
                    action.runtime,
                    action.mutates_files,
                    action.requires_confirmation,
                    action.detail,
                ));
            }
        }
        CodexQuery::Remote(remote) => {
            out.push_str(&format!(
                "\n- Repository: `{:?}`\n- Remote status: `{:?}`\n- Coverage: `{:?}`\n- Metadata: `{:?}`\n- Remote boundary: {}\n",
                remote.repository, remote.status, remote.coverage, remote.metadata, remote.boundary,
            ));
        }
        CodexQuery::PrBody(pr) => {
            out.push_str(&format!(
                "\n- PR title: {}\n- Draft: `{}`\n\n",
                pr.title, pr.draft,
            ));
            out.push_str(&pr.body);
        }
    }
    out.push_str(&format!("\n- Boundary: {}\n", view.boundary));
    out
}

const fn query_kind(query: &CodexQuery<'_>) -> CodexQueryKind {
    match query {
        CodexQuery::Summary(_) => CodexQueryKind::Summary,
        CodexQuery::Routes(_) => CodexQueryKind::Routes,
        CodexQuery::Evidence(_) => CodexQueryKind::Evidence,
        CodexQuery::Documents(_) => CodexQueryKind::Documents,
        CodexQuery::Governance(_) => CodexQueryKind::Governance,
        CodexQuery::Patches(_) => CodexQueryKind::Patches,
        CodexQuery::Linter(_) => CodexQueryKind::Linter,
        CodexQuery::Actions(_) => CodexQueryKind::Actions,
        CodexQuery::Remote(_) => CodexQueryKind::Remote,
        CodexQuery::PrBody(_) => CodexQueryKind::PrBody,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use seiri_core::{
        CapabilityNodeId, ClaimAtomId, ClaimDraft, ClaimDraftIR, ClaimDraftId, ClaimDraftPlanState,
        ClaimDraftSemantics, ClaimModality, ClaimPolarity, DocumentLanguage, PatchBaseDigest,
        SourceSpan, ValueDimension,
    };
    use std::num::NonZeroU32;

    fn nonzero(value: u32) -> NonZeroU32 {
        NonZeroU32::new(value).expect("non-zero fixture ID")
    }

    fn claim_plan() -> PatchPlan {
        let source = b"# RepoSeiri\n";
        let semantics = ClaimDraftSemantics::try_new(
            ValueDimension::Capability,
            Some("reposeiri".to_string()),
            Some("audit".to_string()),
            Some("repositories".to_string()),
            vec!["locally".to_string()],
            None,
            ClaimPolarity::Positive,
            ClaimModality::Qualified,
            DocumentLanguage::English,
        )
        .expect("canonical semantics");
        let draft = ClaimDraft::try_new(
            ClaimDraftId::new(nonzero(1)),
            SourceSpan::new(1, 1, 0, source.len()),
            vec![ClaimAtomId::new(nonzero(1))],
            vec![CapabilityNodeId::new(nonzero(1))],
            semantics,
            ClaimMode::Qualified,
            ClaimMode::Qualified,
        )
        .expect("canonical draft");
        let plan = PatchPlan {
            claim_drafts: ClaimDraftIR::try_new(
                "README.md",
                PatchBaseDigest::from_bytes(source),
                source.len(),
                2,
                vec![draft],
            )
            .expect("source-bound draft IR"),
            claim_draft_state: ClaimDraftPlanState::Ready,
            ..PatchPlan::default()
        };
        plan.validate().expect("preview-only claim plan");
        plan
    }

    #[test]
    fn all_ten_queries_keep_outer_contract_and_decision_markers() {
        let analysis = RepositoryAnalysis::new(".");
        let plan = claim_plan();
        let adapter = CodexView::new(&analysis, &plan, None);
        let expected = [
            (CodexQueryKind::Summary, "summary"),
            (CodexQueryKind::Routes, "routes"),
            (CodexQueryKind::Evidence, "evidence"),
            (CodexQueryKind::Documents, "documents"),
            (CodexQueryKind::Governance, "governance"),
            (CodexQueryKind::Patches, "patches"),
            (CodexQueryKind::Linter, "linter"),
            (CodexQueryKind::Actions, "actions"),
            (CodexQueryKind::Remote, "remote"),
            (CodexQueryKind::PrBody, "pr-body"),
        ];
        for (kind, slug) in expected {
            assert_eq!(kind.slug(), slug);
            let view = adapter.query(kind);
            let json = serde_json::to_value(&view).expect("query JSON");
            assert_eq!(json["schema_version"], CODEX_SCHEMA_VERSION);
            assert_eq!(json["repo_root"], ".");
            assert_eq!(
                json.as_object()
                    .expect("outer object")
                    .keys()
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
                [
                    "boundary",
                    "profile",
                    "query",
                    "repo_root",
                    "schema_version"
                ]
            );
            let markdown = render_query_markdown(&view);
            for marker in [
                format!("- Schema: `{CODEX_SCHEMA_VERSION}`"),
                format!("- Query: `{slug}`"),
                "- Boundary:".to_string(),
            ] {
                assert!(markdown.contains(&marker), "{slug}: missing {marker}");
            }
        }
    }

    #[test]
    fn json_and_markdown_preserve_claim_draft_unknown_ceiling_and_no_write_decisions() {
        let analysis = RepositoryAnalysis::new(".");
        let plan = claim_plan();
        let adapter = CodexView::new(&analysis, &plan, None);

        let summary = adapter.query(CodexQueryKind::Summary);
        let summary_json = serde_json::to_value(&summary).expect("summary JSON");
        let summary_markdown = render_query_markdown(&summary);
        assert_eq!(
            summary_json["query"]["data"]["claim_draft_state"]["state"],
            "ready"
        );
        assert_eq!(summary_json["query"]["data"]["claim_drafts"], 1);
        assert_eq!(
            summary_json["query"]["data"]["claim_draft_baseline_unknown_count"],
            2
        );
        assert_eq!(
            summary_json["query"]["data"]["maximum_claim_draft_ceiling"],
            "qualified"
        );
        assert_eq!(summary_json["query"]["data"]["writes_files"], false);
        for marker in [
            "Claim draft state: `Ready`",
            "Claim drafts: `1`; baseline unknown: `2`; maximum claim ceiling: `Qualified`",
            "Writes files: `false`",
        ] {
            assert!(summary_markdown.contains(marker), "missing {marker}");
        }

        let patches = adapter.query(CodexQueryKind::Patches);
        let patches_json = serde_json::to_value(&patches).expect("patches JSON");
        let patches_markdown = render_query_markdown(&patches);
        assert_eq!(
            patches_json["query"]["data"]["claim_draft_state"]["state"],
            "ready"
        );
        assert_eq!(patches_json["query"]["data"]["writes_files"], false);
        assert_eq!(
            patches_json["query"]["data"]["claim_drafts"]["drafts"][0]["claim_ceiling"],
            "qualified"
        );
        for marker in [
            "Claim draft state: `Ready`",
            "baseline unknown `2`",
            "ceiling `Qualified`",
            "Writes files: `false`",
            "Plan boundary:",
        ] {
            assert!(patches_markdown.contains(marker), "missing {marker}");
        }
        assert!(!patches_json["query"]["data"]["claim_drafts"]
            .as_object()
            .expect("claim draft IR")
            .contains_key("writes_files"));
    }

    #[test]
    fn every_query_markdown_exposes_its_decision_bearing_surface() {
        let analysis = RepositoryAnalysis::new(".");
        let plan = claim_plan();
        let adapter = CodexView::new(&analysis, &plan, None);
        let expected = [
            (CodexQueryKind::Summary, "Observations:"),
            (CodexQueryKind::Routes, "## Missing Route Priorities"),
            (CodexQueryKind::Evidence, "## Coverage"),
            (CodexQueryKind::Documents, "## Document States"),
            (CodexQueryKind::Governance, "## Claim-Capability Support"),
            (CodexQueryKind::Patches, "## Patch Holds"),
            (CodexQueryKind::Linter, "Linter boundary:"),
            (CodexQueryKind::Actions, "mutates files `false`"),
            (CodexQueryKind::Remote, "Remote boundary:"),
            (CodexQueryKind::PrBody, "Draft: `true`"),
        ];
        for (kind, marker) in expected {
            let markdown = render_query_markdown(&adapter.query(kind));
            assert!(
                markdown.contains(marker),
                "{} omitted {marker}",
                kind.slug()
            );
        }
    }
}

use crate::{
    AnalysisScope, ClaimDraftIR, ClaimId, ContentSlotId, CoverageStatus, Digest32, DocumentId,
    DocumentRole, DocumentScanStatus, GateKind, ProfileKind, RepositoryFacet, RouteFreshness,
    RouteKind, RoutePolicyBoundary, RouteTargetRole, ScopeReadBudget, TargetRelation, TextEncoding,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt::{Display, Formatter};

pub const PORTABLE_AUDIT_SCHEMA_VERSION: &str = "seiri.portable-audit.v3";
pub const AUDIT_DELTA_SCHEMA_VERSION: &str = "seiri.audit-delta.v2";
pub const PATCH_PLAN_SCHEMA_VERSION: &str = "seiri.patch-plan.v2";
pub const PATCH_PLANNER_SEMANTIC_REVISION: &str = "seiri.patch-planner.v9";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortableContractError {
    UnsupportedPortableAuditSchema,
    UnsupportedAnalysisSchema,
    SnapshotSchemaMismatch,
    SourceSessionDigestMismatch,
    NonCanonicalRoutes,
    NonCanonicalContentSlots,
    NonCanonicalCoverage,
    NonCanonicalConflicts,
    NonCanonicalObligations,
    NonCanonicalFacets,
    NonCanonicalDocuments,
    NonCanonicalEvidence,
    InvalidRepositoryPath,
    EmptyBoundary,
    ZeroDigest,
    UnsupportedPatchPlanSchema,
    PatchPlanWritesFiles,
    InvalidPatchTargetPath,
    InvalidPatchBinding,
    InvalidClaimDrafts,
    InvalidClaimDraftState,
    ClaimDraftSourceMismatch,
}

impl Display for PortableContractError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::UnsupportedPortableAuditSchema => "unsupported portable audit schema revision",
            Self::UnsupportedAnalysisSchema => "unsupported analysis schema revision",
            Self::SnapshotSchemaMismatch => "snapshot digest schema does not match its container",
            Self::SourceSessionDigestMismatch => {
                "snapshot source-session digest does not match its configuration"
            }
            Self::NonCanonicalRoutes => "portable routes are not sorted and unique",
            Self::NonCanonicalContentSlots => "portable content slots are not sorted and unique",
            Self::NonCanonicalCoverage => "portable coverage records are not sorted and unique",
            Self::NonCanonicalConflicts => "portable conflicts are not sorted and unique",
            Self::NonCanonicalObligations => "portable obligations are not sorted and unique",
            Self::NonCanonicalFacets => "portable facets are not sorted and unique",
            Self::NonCanonicalDocuments => "portable documents are not sorted and unique",
            Self::NonCanonicalEvidence => {
                "portable evidence fingerprints are not sorted and unique"
            }
            Self::InvalidRepositoryPath => "portable record contains a non-canonical path",
            Self::EmptyBoundary => "portable contract boundary must not be empty",
            Self::ZeroDigest => "portable contract contains an uninitialized digest",
            Self::UnsupportedPatchPlanSchema => "unsupported patch-plan schema revision",
            Self::PatchPlanWritesFiles => "patch plan must remain preview-only",
            Self::InvalidPatchTargetPath => "patch plan contains a non-canonical target path",
            Self::InvalidPatchBinding => "patch proposal binding is inconsistent",
            Self::InvalidClaimDrafts => "patch plan contains invalid claim drafts",
            Self::InvalidClaimDraftState => {
                "patch plan claim-draft state conflicts with its typed draft IR"
            }
            Self::ClaimDraftSourceMismatch => {
                "claim drafts do not match the bound README proposal source"
            }
        })
    }
}

impl std::error::Error for PortableContractError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EvidenceIdentityDigest(Digest32);

impl EvidenceIdentityDigest {
    #[must_use]
    pub const fn new(value: Digest32) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> Digest32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EvidenceStateDigest(Digest32);

impl EvidenceStateDigest {
    #[must_use]
    pub const fn new(value: Digest32) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> Digest32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EvidenceOccurrenceDigest(Digest32);

impl EvidenceOccurrenceDigest {
    #[must_use]
    pub const fn new(value: Digest32) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> Digest32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SourceSessionDigest(Digest32);

impl SourceSessionDigest {
    #[must_use]
    pub const fn new(value: Digest32) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> Digest32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct EvidenceFingerprint {
    pub identity: EvidenceIdentityDigest,
    pub state: EvidenceStateDigest,
    pub occurrence: EvidenceOccurrenceDigest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchDecisionBasis {
    pub gate: GateKind,
    pub priority_rank: Option<usize>,
    pub claim_ids: Vec<ClaimId>,
    pub evidence_fingerprints: Vec<EvidenceFingerprint>,
    pub claim_semantic_revision: String,
    pub planner_semantic_revision: String,
    #[serde(default)]
    pub source_session_digest: SourceSessionDigest,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisVisibility {
    #[default]
    Standard,
    PublicSyntheticCalibration,
    LocalPrivateCalibration,
    RedactedCalibration,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisBudgetConfiguration {
    pub filesystem_max_depth: usize,
    pub filesystem_max_entries: usize,
    #[serde(default = "default_filesystem_max_directory_entries")]
    pub filesystem_max_directory_entries: usize,
    pub filesystem_max_ignored_records: usize,
    pub filesystem_additional_ignored_names: Vec<String>,
    pub document_max_documents: usize,
    pub document_max_total_source_bytes: usize,
    pub document_max_source_bytes: usize,
    pub document_max_events: usize,
    pub document_max_diagnostics: usize,
    #[serde(default = "default_program_max_files")]
    pub program_max_files: usize,
    #[serde(default = "default_program_max_total_source_bytes")]
    pub program_max_total_source_bytes: usize,
    #[serde(default = "default_program_max_source_bytes")]
    pub program_max_source_bytes: usize,
    #[serde(default = "default_program_max_nodes")]
    pub program_max_nodes: usize,
    #[serde(default = "default_program_max_edges")]
    pub program_max_edges: usize,
    pub git_max_refs: u32,
    pub git_max_tags: u32,
    pub git_max_commit_headers: u32,
    pub scope: ScopeReadBudget,
}

impl Default for AnalysisBudgetConfiguration {
    fn default() -> Self {
        Self {
            filesystem_max_depth: 32,
            filesystem_max_entries: 100_000,
            filesystem_max_directory_entries: default_filesystem_max_directory_entries(),
            filesystem_max_ignored_records: 4_096,
            filesystem_additional_ignored_names: Vec::new(),
            document_max_documents: 32,
            document_max_total_source_bytes: 4 * 1024 * 1024,
            document_max_source_bytes: 2 * 1024 * 1024,
            document_max_events: 65_536,
            document_max_diagnostics: 1_024,
            program_max_files: default_program_max_files(),
            program_max_total_source_bytes: default_program_max_total_source_bytes(),
            program_max_source_bytes: default_program_max_source_bytes(),
            program_max_nodes: default_program_max_nodes(),
            program_max_edges: default_program_max_edges(),
            git_max_refs: 4_096,
            git_max_tags: 2_048,
            git_max_commit_headers: 10_000,
            scope: ScopeReadBudget::default(),
        }
    }
}

const fn default_filesystem_max_directory_entries() -> usize {
    16_384
}

const fn default_program_max_files() -> usize {
    512
}

const fn default_program_max_total_source_bytes() -> usize {
    8 * 1024 * 1024
}

const fn default_program_max_source_bytes() -> usize {
    1024 * 1024
}

const fn default_program_max_nodes() -> usize {
    65_536
}

const fn default_program_max_edges() -> usize {
    131_072
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisConfiguration {
    pub schema_version: String,
    pub scope: AnalysisScope,
    pub profile: ProfileKind,
    pub budgets: AnalysisBudgetConfiguration,
    pub pattern_registry_fingerprint: String,
    pub visibility: AnalysisVisibility,
    pub calibration_binding: Option<String>,
    #[serde(default)]
    pub source_session_digest: SourceSessionDigest,
}

impl Default for AnalysisConfiguration {
    fn default() -> Self {
        Self {
            schema_version: crate::ANALYSIS_SCHEMA_VERSION.to_string(),
            scope: AnalysisScope::Repository,
            profile: ProfileKind::Common,
            budgets: AnalysisBudgetConfiguration::default(),
            pattern_registry_fingerprint: String::new(),
            visibility: AnalysisVisibility::Standard,
            calibration_binding: None,
            source_session_digest: SourceSessionDigest::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortableObservationState {
    Present,
    Absent,
    Conflict,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortableRouteRecord {
    pub route: RouteKind,
    pub root_structured: bool,
    pub inherited: bool,
    pub readme_routed: bool,
    pub repository_local_targets: usize,
    pub shared_target_conflicts: usize,
    pub freshness: RouteFreshness,
    pub policy: RoutePolicyBoundary,
    pub missing_pattern: bool,
    pub observation: PortableObservationState,
    pub coverage: CoverageStatus,
    pub evidence: Vec<EvidenceFingerprint>,
    pub digest: Digest32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortableContentSlotRecord {
    pub slot: ContentSlotId,
    pub code: String,
    pub route: RouteKind,
    pub observation: PortableObservationState,
    pub coverage: CoverageStatus,
    pub evidence: Vec<EvidenceFingerprint>,
    pub digest: Digest32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortableCoverageRecord {
    pub key: String,
    pub status: CoverageStatus,
    pub digest: Digest32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortableConflictRecord {
    pub id: String,
    pub route: RouteKind,
    pub relation: TargetRelation,
    pub evidence: Vec<EvidenceFingerprint>,
    pub digest: Digest32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortableObligationRecord {
    pub id: String,
    pub route: RouteKind,
    pub facet: RepositoryFacet,
    pub observation: PortableObservationState,
    pub evidence: Vec<EvidenceFingerprint>,
    pub coverage: CoverageStatus,
    pub digest: Digest32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortableFacetRecord {
    pub facet: RepositoryFacet,
    pub observation: PortableObservationState,
    pub evidence: Vec<EvidenceFingerprint>,
    pub coverage: CoverageStatus,
    pub digest: Digest32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortableDocumentRecord {
    pub path: String,
    pub role: DocumentRole,
    pub declared_bytes: u64,
    pub status: DocumentScanStatus,
    pub base_digest: Option<Digest32>,
    pub encoding: Option<TextEncoding>,
    pub coverage: CoverageStatus,
    pub digest: Digest32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditSnapshotDigest {
    pub schema: String,
    pub configuration: Digest32,
    pub source_session: SourceSessionDigest,
    pub evidence: Digest32,
    pub routes: Digest32,
    pub documents: Digest32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "PortableAuditSnapshotWire")]
pub struct PortableAuditSnapshot {
    pub schema_version: String,
    pub configuration: AnalysisConfiguration,
    pub digest: AuditSnapshotDigest,
    pub routes: Vec<PortableRouteRecord>,
    pub content_slots: Vec<PortableContentSlotRecord>,
    pub coverage: Vec<PortableCoverageRecord>,
    pub conflicts: Vec<PortableConflictRecord>,
    pub obligations: Vec<PortableObligationRecord>,
    pub facets: Vec<PortableFacetRecord>,
    pub documents: Vec<PortableDocumentRecord>,
    pub boundary: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PortableAuditSnapshotWire {
    schema_version: String,
    configuration: AnalysisConfiguration,
    digest: AuditSnapshotDigest,
    routes: Vec<PortableRouteRecord>,
    content_slots: Vec<PortableContentSlotRecord>,
    coverage: Vec<PortableCoverageRecord>,
    conflicts: Vec<PortableConflictRecord>,
    obligations: Vec<PortableObligationRecord>,
    facets: Vec<PortableFacetRecord>,
    documents: Vec<PortableDocumentRecord>,
    boundary: String,
}

impl TryFrom<PortableAuditSnapshotWire> for PortableAuditSnapshot {
    type Error = PortableContractError;

    fn try_from(wire: PortableAuditSnapshotWire) -> Result<Self, Self::Error> {
        let snapshot = Self {
            schema_version: wire.schema_version,
            configuration: wire.configuration,
            digest: wire.digest,
            routes: wire.routes,
            content_slots: wire.content_slots,
            coverage: wire.coverage,
            conflicts: wire.conflicts,
            obligations: wire.obligations,
            facets: wire.facets,
            documents: wire.documents,
            boundary: wire.boundary,
        };
        snapshot.validate()?;
        Ok(snapshot)
    }
}

impl PortableAuditSnapshot {
    pub fn validate(&self) -> Result<(), PortableContractError> {
        if self.schema_version != PORTABLE_AUDIT_SCHEMA_VERSION {
            return Err(PortableContractError::UnsupportedPortableAuditSchema);
        }
        if self.configuration.schema_version != crate::ANALYSIS_SCHEMA_VERSION {
            return Err(PortableContractError::UnsupportedAnalysisSchema);
        }
        if self.digest.schema != self.schema_version {
            return Err(PortableContractError::SnapshotSchemaMismatch);
        }
        if self.digest.source_session != self.configuration.source_session_digest {
            return Err(PortableContractError::SourceSessionDigestMismatch);
        }
        if self.boundary.trim().is_empty() {
            return Err(PortableContractError::EmptyBoundary);
        }
        if [
            self.digest.configuration,
            self.digest.evidence,
            self.digest.routes,
            self.digest.documents,
        ]
        .into_iter()
        .any(is_zero_digest)
        {
            return Err(PortableContractError::ZeroDigest);
        }
        if !self
            .routes
            .windows(2)
            .all(|pair| pair[0].route < pair[1].route)
        {
            return Err(PortableContractError::NonCanonicalRoutes);
        }
        if !self
            .content_slots
            .windows(2)
            .all(|pair| pair[0].slot < pair[1].slot)
        {
            return Err(PortableContractError::NonCanonicalContentSlots);
        }
        if !self
            .coverage
            .windows(2)
            .all(|pair| pair[0].key < pair[1].key)
            || self.coverage.iter().any(|record| record.key.is_empty())
        {
            return Err(PortableContractError::NonCanonicalCoverage);
        }
        if !self
            .conflicts
            .windows(2)
            .all(|pair| pair[0].id < pair[1].id)
            || self.conflicts.iter().any(|record| record.id.is_empty())
        {
            return Err(PortableContractError::NonCanonicalConflicts);
        }
        if !self
            .obligations
            .windows(2)
            .all(|pair| pair[0].id < pair[1].id)
            || self.obligations.iter().any(|record| record.id.is_empty())
        {
            return Err(PortableContractError::NonCanonicalObligations);
        }
        if !self
            .facets
            .windows(2)
            .all(|pair| pair[0].facet < pair[1].facet)
        {
            return Err(PortableContractError::NonCanonicalFacets);
        }
        if !self
            .documents
            .windows(2)
            .all(|pair| pair[0].path < pair[1].path)
        {
            return Err(PortableContractError::NonCanonicalDocuments);
        }
        if self
            .documents
            .iter()
            .any(|record| !is_canonical_repository_path(&record.path, false))
            || self.coverage.iter().any(|record| {
                record
                    .key
                    .strip_prefix("document:")
                    .is_some_and(|path| !is_canonical_repository_path(path, false))
            })
        {
            return Err(PortableContractError::InvalidRepositoryPath);
        }
        if self
            .documents
            .iter()
            .any(|record| record.coverage != record.status.coverage_status())
        {
            return Err(PortableContractError::NonCanonicalDocuments);
        }
        if self
            .documents
            .iter()
            .filter_map(|record| record.base_digest)
            .any(is_zero_digest)
        {
            return Err(PortableContractError::ZeroDigest);
        }
        if self.all_record_digests().any(is_zero_digest) {
            return Err(PortableContractError::ZeroDigest);
        }
        if self
            .evidence_sets()
            .any(|evidence| !evidence.windows(2).all(|pair| pair[0] < pair[1]))
        {
            return Err(PortableContractError::NonCanonicalEvidence);
        }
        Ok(())
    }

    fn all_record_digests(&self) -> impl Iterator<Item = Digest32> + '_ {
        self.routes
            .iter()
            .map(|record| record.digest)
            .chain(self.content_slots.iter().map(|record| record.digest))
            .chain(self.coverage.iter().map(|record| record.digest))
            .chain(self.conflicts.iter().map(|record| record.digest))
            .chain(self.obligations.iter().map(|record| record.digest))
            .chain(self.facets.iter().map(|record| record.digest))
            .chain(self.documents.iter().map(|record| record.digest))
    }

    fn evidence_sets(&self) -> impl Iterator<Item = &[EvidenceFingerprint]> {
        self.routes
            .iter()
            .map(|record| record.evidence.as_slice())
            .chain(
                self.content_slots
                    .iter()
                    .map(|record| record.evidence.as_slice()),
            )
            .chain(
                self.conflicts
                    .iter()
                    .map(|record| record.evidence.as_slice()),
            )
            .chain(
                self.obligations
                    .iter()
                    .map(|record| record.evidence.as_slice()),
            )
            .chain(self.facets.iter().map(|record| record.evidence.as_slice()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeltaState {
    Added,
    Removed,
    Changed,
    Unchanged,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeltaUnknownReason {
    SchemaMismatch,
    ScopeMismatch,
    ConfigurationMismatch,
    ContractViolation,
    PartialCoverage,
    MissingComparableRecord,
    UnknownPrivateBinding,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "reason", rename_all = "snake_case")]
pub enum DeltaCompatibility {
    Comparable,
    Unknown(DeltaUnknownReason),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteDelta {
    pub route: RouteKind,
    pub state: DeltaState,
    pub before: Option<PortableRouteRecord>,
    pub after: Option<PortableRouteRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactDelta {
    pub key: String,
    pub state: DeltaState,
    pub before: Option<Digest32>,
    pub after: Option<Digest32>,
    pub before_coverage: CoverageStatus,
    pub after_coverage: CoverageStatus,
    pub evidence: Vec<EvidenceFingerprint>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegressionCandidate {
    pub domain: String,
    pub key: String,
    pub state: DeltaState,
    pub evidence: Vec<EvidenceFingerprint>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImprovementCandidate {
    pub domain: String,
    pub key: String,
    pub state: DeltaState,
    pub evidence: Vec<EvidenceFingerprint>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditDeltaReport {
    pub schema_version: String,
    pub compatibility: DeltaCompatibility,
    pub before: AuditSnapshotDigest,
    pub after: AuditSnapshotDigest,
    pub routes: Vec<RouteDelta>,
    pub content_slots: Vec<ArtifactDelta>,
    pub coverage: Vec<ArtifactDelta>,
    pub conflicts: Vec<ArtifactDelta>,
    pub obligations: Vec<ArtifactDelta>,
    pub facets: Vec<ArtifactDelta>,
    pub regressions: Vec<RegressionCandidate>,
    pub improvements: Vec<ImprovementCandidate>,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatchHoldReason {
    NoExistingTarget,
    TargetNotRepositoryLocal,
    CanonicalConflict,
    UnknownTargetRelation,
    MissingReadme,
    StaleBase,
    StaleAnchor,
    PairedLanguageIncomplete,
    UnsupportedEncoding,
    EvidenceContractInvalid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchProposalKind {
    EditExisting,
    CreateSkeleton,
    ManualDecision,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ExistingTargetId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddExistingRouteLink {
    pub route: RouteKind,
    pub target: ExistingTargetId,
    pub target_path: String,
    pub target_role: RouteTargetRole,
    pub document: DocumentId,
    pub insertion_anchor: crate::PatchAnchorContext,
    pub analysis_run: crate::PatchAnalysisRun,
    pub proposal: crate::PatchProposal,
    pub binding: crate::PatchProposalBinding,
    pub paired_language: bool,
    pub decision_basis: PatchDecisionBasis,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchHold {
    pub route: RouteKind,
    pub target_path: Option<String>,
    pub reason: PatchHoldReason,
    pub decision_basis: PatchDecisionBasis,
}

impl PatchHold {
    #[must_use]
    pub const fn proposal_kind(&self) -> PatchProposalKind {
        match self.reason {
            PatchHoldReason::NoExistingTarget
                if matches!(
                    self.route.spec().policy,
                    crate::RoutePolicyBoundary::Suggestible
                ) =>
            {
                PatchProposalKind::CreateSkeleton
            }
            _ => PatchProposalKind::ManualDecision,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimDraftPlanHoldReason {
    MissingReadme,
    StaleSource,
    InvalidContract,
    UnsupportedEncoding,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "reason", rename_all = "snake_case")]
pub enum ClaimDraftPlanState {
    #[default]
    NotRequested,
    Ready,
    Held(ClaimDraftPlanHoldReason),
}

impl ClaimDraftPlanState {
    #[must_use]
    pub const fn is_not_requested(&self) -> bool {
        matches!(self, Self::NotRequested)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "PatchPlanWire")]
pub struct PatchPlan {
    pub schema_version: String,
    pub operations: Vec<AddExistingRouteLink>,
    pub held: Vec<PatchHold>,
    pub appeal_suggestions: Vec<crate::AppealPlanItem>,
    pub appeal_presentation: crate::AppealPresentationReport,
    #[serde(default, skip_serializing_if = "ClaimDraftIR::is_empty")]
    pub claim_drafts: ClaimDraftIR,
    #[serde(default, skip_serializing_if = "ClaimDraftPlanState::is_not_requested")]
    pub claim_draft_state: ClaimDraftPlanState,
    pub writes_files: bool,
    pub boundary: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PatchPlanWire {
    schema_version: String,
    operations: Vec<AddExistingRouteLink>,
    held: Vec<PatchHold>,
    appeal_suggestions: Vec<crate::AppealPlanItem>,
    appeal_presentation: crate::AppealPresentationReport,
    #[serde(default)]
    claim_drafts: ClaimDraftIR,
    #[serde(default)]
    claim_draft_state: ClaimDraftPlanState,
    writes_files: bool,
    boundary: String,
}

impl TryFrom<PatchPlanWire> for PatchPlan {
    type Error = PortableContractError;

    fn try_from(wire: PatchPlanWire) -> Result<Self, Self::Error> {
        let plan = Self {
            schema_version: wire.schema_version,
            operations: wire.operations,
            held: wire.held,
            appeal_suggestions: wire.appeal_suggestions,
            appeal_presentation: wire.appeal_presentation,
            claim_drafts: wire.claim_drafts,
            claim_draft_state: wire.claim_draft_state,
            writes_files: wire.writes_files,
            boundary: wire.boundary,
        };
        plan.validate()?;
        Ok(plan)
    }
}

impl Default for PatchPlan {
    fn default() -> Self {
        Self {
            schema_version: PATCH_PLAN_SCHEMA_VERSION.to_string(),
            operations: Vec::new(),
            held: Vec::new(),
            appeal_suggestions: Vec::new(),
            appeal_presentation: crate::AppealPresentationReport::default(),
            claim_drafts: ClaimDraftIR::default(),
            claim_draft_state: ClaimDraftPlanState::NotRequested,
            writes_files: false,
            boundary: "Patch planning emits dry-run links and source-bound, prose-free appeal suggestions. It does not write files, generate policy or promotional claims, exceed the claim-capability ceiling, execute Git or GitHub operations, or establish authenticity, safety, or correctness.".to_string(),
        }
    }
}

impl PatchPlan {
    pub fn validate(&self) -> Result<(), PortableContractError> {
        if self.schema_version != PATCH_PLAN_SCHEMA_VERSION {
            return Err(PortableContractError::UnsupportedPatchPlanSchema);
        }
        if self.writes_files {
            return Err(PortableContractError::PatchPlanWritesFiles);
        }
        if self.boundary.trim().is_empty() {
            return Err(PortableContractError::EmptyBoundary);
        }
        let mut proposal_ids = BTreeSet::new();
        for operation in &self.operations {
            if !is_canonical_repository_path(&operation.target_path, true)
                || !is_canonical_repository_path(&operation.proposal.path, false)
                || !is_canonical_repository_path(&operation.binding.path, false)
            {
                return Err(PortableContractError::InvalidPatchTargetPath);
            }
            if !proposal_ids.insert(operation.proposal.id.as_str())
                || operation.proposal.schema_version != crate::PATCH_PROPOSAL_SCHEMA_VERSION
                || operation.analysis_run != operation.binding.analysis_run
                || operation.proposal.id != operation.binding.proposal_id
                || operation.proposal.path != operation.binding.path
                || operation.proposal.base.digest() != operation.binding.base_digest
                || operation.binding.anchors.len() != operation.proposal.edits.len()
                || !operation.binding.anchors.iter().all(|anchor| {
                    operation
                        .proposal
                        .edits
                        .iter()
                        .any(|edit| edit.id == anchor.edit_id && edit.span == anchor.span)
                })
                || !operation
                    .binding
                    .anchors
                    .iter()
                    .any(|anchor| anchor == &operation.insertion_anchor)
            {
                return Err(PortableContractError::InvalidPatchBinding);
            }
        }
        if self.held.iter().any(|hold| {
            hold.target_path
                .as_deref()
                .is_some_and(|path| !is_canonical_repository_path(path, true))
        }) {
            return Err(PortableContractError::InvalidPatchTargetPath);
        }
        match (self.claim_drafts.is_empty(), self.claim_draft_state) {
            (true, ClaimDraftPlanState::NotRequested | ClaimDraftPlanState::Held(_))
            | (false, ClaimDraftPlanState::Ready) => {}
            (true, ClaimDraftPlanState::Ready)
            | (false, ClaimDraftPlanState::NotRequested | ClaimDraftPlanState::Held(_)) => {
                return Err(PortableContractError::InvalidClaimDraftState);
            }
        }
        if !self.claim_drafts.is_empty() {
            ClaimDraftIR::try_new(
                self.claim_drafts.path.clone(),
                self.claim_drafts.source_digest,
                self.claim_drafts.source_byte_len,
                self.claim_drafts.baseline_unknown_count,
                self.claim_drafts.drafts.clone(),
            )
            .map_err(|_| PortableContractError::InvalidClaimDrafts)?;
            if self.operations.iter().any(|operation| {
                operation.proposal.path != self.claim_drafts.path
                    || operation.proposal.base.digest() != self.claim_drafts.source_digest
                    || operation.proposal.base.byte_len() != self.claim_drafts.source_byte_len
            }) {
                return Err(PortableContractError::ClaimDraftSourceMismatch);
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn proposal_count(&self, kind: PatchProposalKind) -> usize {
        match kind {
            PatchProposalKind::EditExisting => self.operations.len(),
            PatchProposalKind::CreateSkeleton | PatchProposalKind::ManualDecision => self
                .held
                .iter()
                .filter(|hold| hold.proposal_kind() == kind)
                .count(),
        }
    }

    #[must_use]
    pub fn appeal_suggestion_count(&self, gate: GateKind) -> usize {
        self.appeal_suggestions
            .iter()
            .filter(|suggestion| suggestion.gate == gate)
            .count()
    }
}

fn is_zero_digest(digest: Digest32) -> bool {
    digest.bytes() == [0; 32]
}

fn is_canonical_repository_path(path: &str, allow_trailing_slash: bool) -> bool {
    if path.is_empty()
        || path.len() > 4_096
        || path != path.trim()
        || path.starts_with('/')
        || path.starts_with('\\')
        || path.contains(['\\', '\0', ':'])
        || path.chars().any(char::is_control)
        || (!allow_trailing_slash && path.ends_with('/'))
    {
        return false;
    }
    let trimmed = if allow_trailing_slash {
        path.strip_suffix('/').unwrap_or(path)
    } else {
        path
    };
    !trimmed.is_empty()
        && trimmed
            .split('/')
            .all(|component| !component.is_empty() && component != "." && component != "..")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PatchBaseDigest;

    #[test]
    fn patch_plan_is_preview_only_and_claim_draft_state_is_explicit() {
        let plan = PatchPlan::default();
        assert_eq!(plan.validate(), Ok(()));

        let mut writes = plan.clone();
        writes.writes_files = true;
        assert_eq!(
            writes.validate(),
            Err(PortableContractError::PatchPlanWritesFiles)
        );

        let mut missing_ready_ir = plan.clone();
        missing_ready_ir.claim_draft_state = ClaimDraftPlanState::Ready;
        assert_eq!(
            missing_ready_ir.validate(),
            Err(PortableContractError::InvalidClaimDraftState)
        );

        let mut ready = plan;
        ready.claim_drafts = ClaimDraftIR::try_new(
            "README.md",
            PatchBaseDigest::from_bytes(b"# README\n"),
            b"# README\n".len(),
            0,
            Vec::new(),
        )
        .unwrap();
        ready.claim_draft_state = ClaimDraftPlanState::Ready;
        assert_eq!(ready.validate(), Ok(()));

        ready.claim_draft_state =
            ClaimDraftPlanState::Held(ClaimDraftPlanHoldReason::InvalidContract);
        assert_eq!(
            ready.validate(),
            Err(PortableContractError::InvalidClaimDraftState)
        );
    }

    #[test]
    fn patch_plan_rejects_escaped_held_target_path() {
        let mut plan = PatchPlan::default();
        plan.held.push(PatchHold {
            route: RouteKind::Docs,
            target_path: Some("../docs/".to_string()),
            reason: PatchHoldReason::TargetNotRepositoryLocal,
            decision_basis: PatchDecisionBasis {
                gate: GateKind::Manual,
                priority_rank: None,
                claim_ids: Vec::new(),
                evidence_fingerprints: Vec::new(),
                claim_semantic_revision: crate::CLAIM_SEMANTIC_REVISION.to_string(),
                planner_semantic_revision: PATCH_PLANNER_SEMANTIC_REVISION.to_string(),
                source_session_digest: SourceSessionDigest::default(),
            },
        });
        assert_eq!(
            plan.validate(),
            Err(PortableContractError::InvalidPatchTargetPath)
        );

        for non_portable in [" docs/", "docs/\nprivate/"] {
            plan.held[0].target_path = Some(non_portable.to_string());
            assert_eq!(
                plan.validate(),
                Err(PortableContractError::InvalidPatchTargetPath)
            );
        }
    }
}

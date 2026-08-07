use crate::{CoverageStatus, DocumentLanguage, GateKind, SourceSpan, UnknownReason};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{Display, Formatter};
use std::num::NonZeroU32;

pub const README_GRAMMAR_REVISION: &str = "seiri.readme-grammar.v1";
pub const REPOSITORY_CAPABILITY_REVISION: &str = "seiri.repository-capability.v1";
pub const CLAIM_CAPABILITY_MEMBRANE_REVISION: &str = "seiri.claim-capability-membrane.v1";
pub const VALUE_COVERAGE_REVISION: &str = "seiri.value-coverage.v1";
pub const NARRATIVE_TOPOLOGY_REVISION: &str = "seiri.narrative-topology.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GrammarNodeId(NonZeroU32);

impl GrammarNodeId {
    #[must_use]
    pub const fn new(value: NonZeroU32) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0.get()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CapabilityNodeId(NonZeroU32);

impl CapabilityNodeId {
    #[must_use]
    pub const fn new(value: NonZeroU32) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0.get()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValueDimension {
    Identity,
    Audience,
    Problem,
    Capability,
    Outcome,
    FirstAction,
    FirstResult,
    Evidence,
    Constraint,
    Differentiation,
}

impl ValueDimension {
    pub const ALL: [Self; 10] = [
        Self::Identity,
        Self::Audience,
        Self::Problem,
        Self::Capability,
        Self::Outcome,
        Self::FirstAction,
        Self::FirstResult,
        Self::Evidence,
        Self::Constraint,
        Self::Differentiation,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "reason", rename_all = "snake_case")]
pub enum AnswerState {
    Explicit,
    Inferred,
    Missing,
    Contested,
    Unknown(UnknownReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GrammarPredicate {
    DefinesIdentity,
    TargetsAudience,
    StatesProblem,
    PerformsOperation,
    ProducesOutcome,
    DescribesFirstAction,
    DescribesFirstResult,
    ProvidesEvidence,
    StatesConstraint,
    StatesDifferentiation,
    RequestsAction,
}

impl GrammarPredicate {
    #[must_use]
    pub const fn dimension(self) -> ValueDimension {
        match self {
            Self::DefinesIdentity => ValueDimension::Identity,
            Self::TargetsAudience => ValueDimension::Audience,
            Self::StatesProblem => ValueDimension::Problem,
            Self::PerformsOperation => ValueDimension::Capability,
            Self::ProducesOutcome => ValueDimension::Outcome,
            Self::DescribesFirstAction => ValueDimension::FirstAction,
            Self::DescribesFirstResult => ValueDimension::FirstResult,
            Self::ProvidesEvidence => ValueDimension::Evidence,
            Self::StatesConstraint => ValueDimension::Constraint,
            Self::StatesDifferentiation => ValueDimension::Differentiation,
            Self::RequestsAction => ValueDimension::FirstAction,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimModality {
    Asserted,
    Qualified,
    Possible,
    Required,
    Prohibited,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrammarNode {
    pub id: GrammarNodeId,
    pub predicate: GrammarPredicate,
    pub state: AnswerState,
    pub language: DocumentLanguage,
    pub modality: ClaimModality,
    pub negated: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<SourceSpan>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NarrativeRelation {
    Precedes,
    Supports,
    Explains,
    Qualifies,
    Translates,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrammarEdge {
    pub from: GrammarNodeId,
    pub to: GrammarNodeId,
    pub relation: NarrativeRelation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GrammarDiagnosticKind {
    SourceLimitExceeded,
    NodeLimitExceeded,
    AmbiguousLanguage,
    UnsupportedConstruct,
    ConflictingClause,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrammarDiagnostic {
    pub kind: GrammarDiagnosticKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<SourceSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadmeGrammarIR {
    pub semantic_revision: String,
    pub path: String,
    pub coverage: CoverageStatus,
    pub nodes: Vec<GrammarNode>,
    pub edges: Vec<GrammarEdge>,
    pub diagnostics: Vec<GrammarDiagnostic>,
}

impl Default for ReadmeGrammarIR {
    fn default() -> Self {
        Self {
            semantic_revision: README_GRAMMAR_REVISION.to_string(),
            path: String::new(),
            coverage: CoverageStatus::NotRequested,
            nodes: Vec::new(),
            edges: Vec::new(),
            diagnostics: Vec::new(),
        }
    }
}

impl ReadmeGrammarIR {
    pub fn try_new(
        path: impl Into<String>,
        coverage: CoverageStatus,
        nodes: Vec<GrammarNode>,
        edges: Vec<GrammarEdge>,
        diagnostics: Vec<GrammarDiagnostic>,
    ) -> Result<Self, AppealIrError> {
        let path = path.into();
        if path.is_empty() {
            return Err(AppealIrError::EmptyPath);
        }
        let mut ids = BTreeSet::new();
        let mut previous = None;
        for node in &nodes {
            if !ids.insert(node.id) || previous.is_some_and(|id| id >= node.id) {
                return Err(AppealIrError::NonCanonicalGrammarNodes);
            }
            if node.state == AnswerState::Explicit && node.span.is_none() {
                return Err(AppealIrError::ExplicitGrammarWithoutSpan(node.id));
            }
            previous = Some(node.id);
        }
        if edges
            .iter()
            .any(|edge| !ids.contains(&edge.from) || !ids.contains(&edge.to))
        {
            return Err(AppealIrError::DanglingGrammarEdge);
        }
        Ok(Self {
            semantic_revision: README_GRAMMAR_REVISION.to_string(),
            path,
            coverage,
            nodes,
            edges,
            diagnostics,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityKind {
    Manifest,
    ProgramLanguage,
    Entrypoint,
    PublicApi,
    Operation,
    Input,
    Output,
    FirstResult,
    Example,
    Test,
    Feature,
    Constraint,
}

impl CapabilityKind {
    #[must_use]
    pub const fn dimension(self) -> ValueDimension {
        match self {
            Self::Manifest | Self::ProgramLanguage => ValueDimension::Identity,
            Self::Entrypoint | Self::PublicApi | Self::Operation | Self::Input | Self::Feature => {
                ValueDimension::Capability
            }
            Self::Output => ValueDimension::Outcome,
            Self::FirstResult | Self::Example => ValueDimension::FirstResult,
            Self::Test => ValueDimension::Evidence,
            Self::Constraint => ValueDimension::Constraint,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "reason", rename_all = "snake_case")]
pub enum CapabilitySupport {
    Observed,
    Inferred,
    Unknown(UnknownReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityProvenanceKind {
    Manifest,
    SourceSyntax,
    ExamplePath,
    TestPath,
    Documentation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityProvenance {
    pub path: String,
    pub kind: CapabilityProvenanceKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<SourceSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityNode {
    pub id: CapabilityNodeId,
    pub kind: CapabilityKind,
    pub support: CapabilitySupport,
    pub symbol: String,
    pub provenance: Vec<CapabilityProvenance>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityRelation {
    Offers,
    Accepts,
    Produces,
    DemonstratedBy,
    ConstrainedBy,
    Exposes,
    ConditionedBy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityEdge {
    pub from: CapabilityNodeId,
    pub to: CapabilityNodeId,
    pub relation: CapabilityRelation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepositoryCapabilityIR {
    pub semantic_revision: String,
    pub coverage: CoverageStatus,
    pub nodes: Vec<CapabilityNode>,
    pub edges: Vec<CapabilityEdge>,
    pub unknown_reasons: Vec<UnknownReason>,
}

impl Default for RepositoryCapabilityIR {
    fn default() -> Self {
        Self {
            semantic_revision: REPOSITORY_CAPABILITY_REVISION.to_string(),
            coverage: CoverageStatus::NotRequested,
            nodes: Vec::new(),
            edges: Vec::new(),
            unknown_reasons: Vec::new(),
        }
    }
}

impl RepositoryCapabilityIR {
    pub fn try_new(
        coverage: CoverageStatus,
        nodes: Vec<CapabilityNode>,
        edges: Vec<CapabilityEdge>,
        mut unknown_reasons: Vec<UnknownReason>,
    ) -> Result<Self, AppealIrError> {
        let mut ids = BTreeSet::new();
        let mut previous = None;
        for node in &nodes {
            if !ids.insert(node.id) || previous.is_some_and(|id| id >= node.id) {
                return Err(AppealIrError::NonCanonicalCapabilityNodes);
            }
            if matches!(
                node.support,
                CapabilitySupport::Observed | CapabilitySupport::Inferred
            ) && node.provenance.is_empty()
            {
                return Err(AppealIrError::CapabilityWithoutProvenance(node.id));
            }
            if node
                .provenance
                .iter()
                .any(|provenance| provenance.path.is_empty())
            {
                return Err(AppealIrError::EmptyPath);
            }
            previous = Some(node.id);
        }
        if edges
            .iter()
            .any(|edge| !ids.contains(&edge.from) || !ids.contains(&edge.to))
        {
            return Err(AppealIrError::DanglingCapabilityEdge);
        }
        unknown_reasons.sort_unstable_by_key(|reason| unknown_reason_rank(*reason));
        unknown_reasons.dedup();
        Ok(Self {
            semantic_revision: REPOSITORY_CAPABILITY_REVISION.to_string(),
            coverage,
            nodes,
            edges,
            unknown_reasons,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimMode {
    Omitted,
    Generic,
    Qualified,
    Direct,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimFloor {
    pub requirements: BTreeMap<ValueDimension, ClaimMode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimCeiling {
    pub limits: BTreeMap<ValueDimension, ClaimMode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "reason", rename_all = "snake_case")]
pub enum SupportState {
    Supported,
    InsufficientEvidence,
    Contradicted,
    Unknown(UnknownReason),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupportRelation {
    pub dimension: ValueDimension,
    pub grammar_nodes: Vec<GrammarNodeId>,
    pub capability_nodes: Vec<CapabilityNodeId>,
    pub evidence_count: usize,
    pub state: SupportState,
}

impl SupportRelation {
    pub fn try_new(
        dimension: ValueDimension,
        mut grammar_nodes: Vec<GrammarNodeId>,
        mut capability_nodes: Vec<CapabilityNodeId>,
        evidence_count: usize,
        state: SupportState,
    ) -> Result<Self, AppealIrError> {
        grammar_nodes.sort_unstable();
        grammar_nodes.dedup();
        capability_nodes.sort_unstable();
        capability_nodes.dedup();
        if state == SupportState::Supported && (capability_nodes.is_empty() || evidence_count == 0)
        {
            return Err(AppealIrError::SupportedWithoutEvidence);
        }
        Ok(Self {
            dimension,
            grammar_nodes,
            capability_nodes,
            evidence_count,
            state,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnderclaimOpportunityKind {
    MissingSupportedValue,
    ModeBelowFloor,
    GenericVerbCollapse,
    JargonOcclusion,
    CapabilityOutcomeDisconnect,
    FirstValueDisconnect,
    EvidenceDisconnect,
    QualifierDominance,
    BuriedPrimaryValue,
    FragmentedValue,
    TranslationDivergence,
    UnexpressedConstraintBackedAdvantage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnderclaimOpportunity {
    pub kind: UnderclaimOpportunityKind,
    pub dimension: ValueDimension,
    pub gate: GateKind,
    pub grammar_nodes: Vec<GrammarNodeId>,
    pub capability_nodes: Vec<CapabilityNodeId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppealPlanAction {
    MoveExistingValueEarlier,
    ConnectExistingValuePath,
    AssociateExistingEvidence,
    ExpressSupportedValue,
    RaiseSupportedSpecificity,
    ClarifyExistingValue,
    ReviewTranslationAlignment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppealAnchorKind {
    ReadmeGrammar,
    CapabilityProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppealPlanAnchor {
    pub kind: AppealAnchorKind,
    pub path: String,
    pub span: Option<SourceSpan>,
    pub grammar_node: Option<GrammarNodeId>,
    pub capability_node: Option<CapabilityNodeId>,
}

/// A source-bound, prose-free review suggestion. It never raises the membrane ceiling.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppealPlanItem {
    pub id: String,
    pub opportunity: UnderclaimOpportunityKind,
    pub dimension: ValueDimension,
    pub gate: GateKind,
    pub action: AppealPlanAction,
    pub support_state: SupportState,
    pub claim_ceiling: ClaimMode,
    pub anchors: Vec<AppealPlanAnchor>,
    pub source_session_digest: crate::SourceSessionDigest,
    pub membrane_semantic_revision: String,
    pub planner_semantic_revision: String,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppealPresentationMethod {
    Canonical,
    GeometryShadowV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppealPresentationSignal {
    pub suggestion_id: String,
    pub grammar_nodes: Vec<GrammarNodeId>,
    pub first_value_distance: Option<usize>,
    pub local_degree: usize,
    pub forman_curvature: i32,
    pub presentation_priority: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppealPresentationReport {
    pub method: AppealPresentationMethod,
    pub coverage: CoverageStatus,
    pub ordered_suggestion_ids: Vec<String>,
    pub signals: Vec<AppealPresentationSignal>,
    pub membrane_semantic_digest: crate::Digest32,
    pub boundary: String,
}

impl Default for AppealPresentationReport {
    fn default() -> Self {
        Self {
            method: AppealPresentationMethod::Canonical,
            coverage: CoverageStatus::NotRequested,
            ordered_suggestion_ids: Vec::new(),
            signals: Vec::new(),
            membrane_semantic_digest: crate::Digest32::default(),
            boundary: "Presentation ranking is disabled. No presentation signal is evidence and no signal may change a gate, support relation, claim ceiling, opportunity, or risk."
                .to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OverclaimRiskKind {
    UnsupportedCapability,
    UnsupportedOutcome,
    RuntimeSuccessNotObserved,
    PerformanceNotMeasured,
    TrustNotEstablished,
    SecurityNotEstablished,
    AudienceNotObserved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverclaimRisk {
    pub kind: OverclaimRiskKind,
    pub dimension: ValueDimension,
    pub grammar_nodes: Vec<GrammarNodeId>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppealLossVector {
    pub missing_supported_values: usize,
    pub below_floor_dimensions: usize,
    pub above_ceiling_dimensions: usize,
    pub disconnected_value_paths: usize,
    pub translation_divergences: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NarrativeTopologyReport {
    pub semantic_revision: String,
    pub first_value_path: Vec<GrammarNodeId>,
    pub disconnected_dimensions: Vec<ValueDimension>,
    pub translation_divergences: usize,
}

impl Default for NarrativeTopologyReport {
    fn default() -> Self {
        Self {
            semantic_revision: NARRATIVE_TOPOLOGY_REVISION.to_string(),
            first_value_path: Vec::new(),
            disconnected_dimensions: Vec::new(),
            translation_divergences: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadmeValueCoverageReport {
    pub semantic_revision: String,
    pub dimensions: BTreeMap<ValueDimension, AnswerState>,
    pub opportunities: Vec<UnderclaimOpportunity>,
    pub risks: Vec<OverclaimRisk>,
    pub narrative: NarrativeTopologyReport,
}

impl Default for ReadmeValueCoverageReport {
    fn default() -> Self {
        Self {
            semantic_revision: VALUE_COVERAGE_REVISION.to_string(),
            dimensions: ValueDimension::ALL
                .into_iter()
                .map(|dimension| (dimension, AnswerState::Unknown(UnknownReason::NotRequested)))
                .collect(),
            opportunities: Vec::new(),
            risks: Vec::new(),
            narrative: NarrativeTopologyReport::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimCapabilityMembrane {
    pub semantic_revision: String,
    pub floor: ClaimFloor,
    pub ceiling: ClaimCeiling,
    pub relations: Vec<SupportRelation>,
    pub opportunities: Vec<UnderclaimOpportunity>,
    pub risks: Vec<OverclaimRisk>,
    pub losses: AppealLossVector,
}

impl Default for ClaimCapabilityMembrane {
    fn default() -> Self {
        Self {
            semantic_revision: CLAIM_CAPABILITY_MEMBRANE_REVISION.to_string(),
            floor: ClaimFloor {
                requirements: BTreeMap::new(),
            },
            ceiling: ClaimCeiling {
                limits: BTreeMap::new(),
            },
            relations: Vec::new(),
            opportunities: Vec::new(),
            risks: Vec::new(),
            losses: AppealLossVector::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppealIrError {
    EmptyPath,
    NonCanonicalGrammarNodes,
    ExplicitGrammarWithoutSpan(GrammarNodeId),
    DanglingGrammarEdge,
    NonCanonicalCapabilityNodes,
    CapabilityWithoutProvenance(CapabilityNodeId),
    DanglingCapabilityEdge,
    SupportedWithoutEvidence,
}

impl Display for AppealIrError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyPath => formatter.write_str("appeal IR path must not be empty"),
            Self::NonCanonicalGrammarNodes => {
                formatter.write_str("grammar nodes must be sorted and unique")
            }
            Self::ExplicitGrammarWithoutSpan(id) => write!(
                formatter,
                "explicit grammar node {} requires a source span",
                id.get()
            ),
            Self::DanglingGrammarEdge => {
                formatter.write_str("grammar edge references unknown node")
            }
            Self::NonCanonicalCapabilityNodes => {
                formatter.write_str("capability nodes must be sorted and unique")
            }
            Self::CapabilityWithoutProvenance(id) => write!(
                formatter,
                "capability node {} requires provenance",
                id.get()
            ),
            Self::DanglingCapabilityEdge => {
                formatter.write_str("capability edge references unknown node")
            }
            Self::SupportedWithoutEvidence => {
                formatter.write_str("supported relation requires capability and evidence")
            }
        }
    }
}

impl std::error::Error for AppealIrError {}

const fn unknown_reason_rank(reason: UnknownReason) -> u8 {
    match reason {
        UnknownReason::NotRequested => 0,
        UnknownReason::LimitExceeded => 1,
        UnknownReason::InvalidUtf8 => 2,
        UnknownReason::ParseFailed => 3,
        UnknownReason::UnsupportedSyntax => 4,
        UnknownReason::PermissionDenied => 5,
        UnknownReason::RateLimited => 6,
        UnknownReason::Unavailable => 7,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grammar_id(value: u32) -> GrammarNodeId {
        GrammarNodeId::new(NonZeroU32::new(value).expect("non-zero"))
    }

    fn capability_id(value: u32) -> CapabilityNodeId {
        CapabilityNodeId::new(NonZeroU32::new(value).expect("non-zero"))
    }

    #[test]
    fn explicit_grammar_requires_source_span() {
        let result = ReadmeGrammarIR::try_new(
            "README.md",
            CoverageStatus::Complete,
            vec![GrammarNode {
                id: grammar_id(1),
                predicate: GrammarPredicate::DefinesIdentity,
                state: AnswerState::Explicit,
                language: DocumentLanguage::English,
                modality: ClaimModality::Asserted,
                negated: false,
                span: None,
            }],
            Vec::new(),
            Vec::new(),
        );
        assert_eq!(
            result,
            Err(AppealIrError::ExplicitGrammarWithoutSpan(grammar_id(1)))
        );
    }

    #[test]
    fn observed_capability_requires_provenance() {
        let result = RepositoryCapabilityIR::try_new(
            CoverageStatus::Complete,
            vec![CapabilityNode {
                id: capability_id(1),
                kind: CapabilityKind::Entrypoint,
                support: CapabilitySupport::Observed,
                symbol: "seiri".to_string(),
                provenance: Vec::new(),
            }],
            Vec::new(),
            Vec::new(),
        );
        assert_eq!(
            result,
            Err(AppealIrError::CapabilityWithoutProvenance(capability_id(1)))
        );
    }

    #[test]
    fn supported_relation_requires_capability_and_evidence() {
        assert_eq!(
            SupportRelation::try_new(
                ValueDimension::Capability,
                vec![grammar_id(1)],
                Vec::new(),
                0,
                SupportState::Supported,
            ),
            Err(AppealIrError::SupportedWithoutEvidence)
        );
    }
}

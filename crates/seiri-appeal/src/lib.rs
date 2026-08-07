#![forbid(unsafe_code)]

mod geometry;
mod incremental;

pub use geometry::{
    rank_appeal_presentation, GeometryShadowOptions, GeometryShadowRanking, GeometryShadowSignal,
    GEOMETRY_SHADOW_REVISION,
};

pub use incremental::{
    membrane_semantic_digest, update_incremental_membrane, verify_incremental_against_scalar,
    AppealDependencyIndex, IncrementalEquivalence, IncrementalMembraneState,
    IncrementalMembraneUpdate, IncrementalUpdateMode,
};

use seiri_core::{
    AnswerState, AppealLossVector, CapabilityKind, CapabilityNodeId, ClaimCapabilityMembrane,
    ClaimCeiling, ClaimFloor, ClaimModality, ClaimMode, CoverageIncompleteReason, CoverageStatus,
    DocumentLanguage, GateKind, GrammarNode, GrammarNodeId, NarrativeTopologyReport, OverclaimRisk,
    OverclaimRiskKind, ProfileKind, ReadmeGrammarIR, ReadmeValueCoverageReport,
    RepositoryCapabilityIR, SupportRelation, SupportState, UnderclaimOpportunity,
    UnderclaimOpportunityKind, UnknownReason, ValueDimension, CLAIM_CAPABILITY_MEMBRANE_REVISION,
    VALUE_COVERAGE_REVISION,
};
use std::collections::{BTreeMap, BTreeSet};

/// Computes README-internal value coverage without treating README prose as repository evidence.
#[must_use]
pub fn analyze_document_value_coverage(grammar: &ReadmeGrammarIR) -> ReadmeValueCoverageReport {
    let dimensions = ValueDimension::ALL
        .into_iter()
        .map(|dimension| {
            let matching = grammar
                .nodes
                .iter()
                .filter(|node| node.predicate.dimension() == dimension)
                .collect::<Vec<_>>();
            (
                dimension,
                answer_state(grammar.coverage, dimension, &matching),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut opportunities = Vec::new();
    add_disconnect_opportunities(grammar, &dimensions, &mut opportunities);
    add_narrative_opportunities(grammar, &mut opportunities);
    add_translation_opportunities(grammar, &mut opportunities);
    opportunities.sort_by_key(opportunity_key);
    opportunities.dedup_by(|left, right| opportunity_key(left) == opportunity_key(right));
    let narrative = narrative_topology(grammar, &dimensions);
    ReadmeValueCoverageReport {
        semantic_revision: VALUE_COVERAGE_REVISION.to_string(),
        dimensions,
        opportunities,
        risks: Vec::new(),
        narrative,
    }
}

/// Compares README realization with repository-observable capability without allowing prose to
/// promote its own evidence ceiling.
#[must_use]
pub fn evaluate_claim_capability_membrane(
    grammar: &ReadmeGrammarIR,
    capabilities: &RepositoryCapabilityIR,
    profile: ProfileKind,
) -> ClaimCapabilityMembrane {
    let document = analyze_document_value_coverage(grammar);
    let floor = profile_floor(profile);
    let ceiling = capability_ceiling(capabilities);
    let mut relations = Vec::new();
    let mut opportunities = document.opportunities.clone();
    let mut risks = Vec::new();

    for dimension in ValueDimension::ALL {
        let evaluation = evaluate_dimension(
            grammar,
            capabilities,
            &floor,
            &ceiling,
            dimension,
            document
                .dimensions
                .get(&dimension)
                .copied()
                .unwrap_or(AnswerState::Unknown(UnknownReason::ParseFailed)),
        );
        relations.push(evaluation.relation);
        opportunities.extend(evaluation.opportunities);
        risks.extend(evaluation.risk);
    }

    opportunities.sort_by_key(|opportunity| {
        (
            opportunity.kind,
            opportunity.dimension,
            gate_rank(opportunity.gate),
        )
    });
    opportunities.dedup_by(|left, right| {
        (left.kind, left.dimension, left.gate) == (right.kind, right.dimension, right.gate)
    });
    risks.sort_by_key(|risk| (risk.kind, risk.dimension));
    risks.dedup_by_key(|risk| (risk.kind, risk.dimension));
    let losses = AppealLossVector {
        missing_supported_values: opportunities
            .iter()
            .filter(|opportunity| {
                opportunity.kind == UnderclaimOpportunityKind::MissingSupportedValue
            })
            .count(),
        below_floor_dimensions: opportunities
            .iter()
            .filter(|opportunity| opportunity.kind == UnderclaimOpportunityKind::ModeBelowFloor)
            .count(),
        above_ceiling_dimensions: risks.len(),
        disconnected_value_paths: document.narrative.disconnected_dimensions.len(),
        translation_divergences: document.narrative.translation_divergences,
    };
    ClaimCapabilityMembrane {
        semantic_revision: CLAIM_CAPABILITY_MEMBRANE_REVISION.to_string(),
        floor,
        ceiling,
        relations,
        opportunities,
        risks,
        losses,
    }
}

struct DimensionEvaluation {
    relation: SupportRelation,
    opportunities: Vec<UnderclaimOpportunity>,
    risk: Option<OverclaimRisk>,
}

fn evaluate_dimension(
    grammar: &ReadmeGrammarIR,
    capabilities: &RepositoryCapabilityIR,
    floor: &ClaimFloor,
    ceiling: &ClaimCeiling,
    dimension: ValueDimension,
    document_state: AnswerState,
) -> DimensionEvaluation {
    let grammar_nodes = nodes_for(grammar, dimension);
    let capability_nodes = capability_nodes_for(capabilities, dimension);
    let evidence_count = capability_nodes
        .iter()
        .filter_map(|id| capabilities.nodes.iter().find(|node| node.id == *id))
        .map(|node| node.provenance.len())
        .sum();
    let state = support_state(capabilities, &capability_nodes, evidence_count);
    let relation = SupportRelation::try_new(
        dimension,
        grammar_nodes.clone(),
        capability_nodes.clone(),
        evidence_count,
        state,
    )
    .expect("membrane creates evidence-closed support relations");
    let realized = realized_mode(grammar, dimension);
    let required = floor
        .requirements
        .get(&dimension)
        .copied()
        .unwrap_or(ClaimMode::Omitted);
    let allowed = ceiling
        .limits
        .get(&dimension)
        .copied()
        .unwrap_or(ClaimMode::Omitted);
    let mut opportunities = Vec::new();
    if state == SupportState::Supported && document_state == AnswerState::Missing {
        opportunities.push(UnderclaimOpportunity {
            kind: UnderclaimOpportunityKind::MissingSupportedValue,
            dimension,
            gate: automatic_gate(dimension),
            grammar_nodes: Vec::new(),
            capability_nodes: capability_nodes.clone(),
        });
    }
    if realized < required {
        opportunities.push(UnderclaimOpportunity {
            kind: UnderclaimOpportunityKind::ModeBelowFloor,
            dimension,
            gate: if state == SupportState::Supported {
                automatic_gate(dimension)
            } else {
                GateKind::Manual
            },
            grammar_nodes: grammar_nodes.clone(),
            capability_nodes,
        });
    }
    let risk = if realized > allowed && realized != ClaimMode::Omitted {
        overclaim_kind(dimension).map(|kind| OverclaimRisk {
            kind,
            dimension,
            grammar_nodes,
        })
    } else {
        None
    };
    DimensionEvaluation {
        relation,
        opportunities,
        risk,
    }
}

fn profile_floor(profile: ProfileKind) -> ClaimFloor {
    let mut requirements = BTreeMap::from([
        (ValueDimension::Identity, ClaimMode::Qualified),
        (ValueDimension::Capability, ClaimMode::Qualified),
        (ValueDimension::FirstAction, ClaimMode::Generic),
        (ValueDimension::Evidence, ClaimMode::Generic),
        (ValueDimension::Constraint, ClaimMode::Generic),
    ]);
    match profile {
        ProfileKind::Library => {
            requirements.insert(ValueDimension::Capability, ClaimMode::Direct);
            requirements.insert(ValueDimension::Outcome, ClaimMode::Generic);
        }
        ProfileKind::Cli => {
            requirements.insert(ValueDimension::Capability, ClaimMode::Direct);
            requirements.insert(ValueDimension::FirstAction, ClaimMode::Qualified);
            requirements.insert(ValueDimension::FirstResult, ClaimMode::Generic);
        }
        ProfileKind::Infra | ProfileKind::Runtime => {
            requirements.insert(ValueDimension::Constraint, ClaimMode::Qualified);
            requirements.insert(ValueDimension::Outcome, ClaimMode::Generic);
        }
        ProfileKind::Product => {
            requirements.insert(ValueDimension::Audience, ClaimMode::Qualified);
            requirements.insert(ValueDimension::Problem, ClaimMode::Generic);
            requirements.insert(ValueDimension::Outcome, ClaimMode::Qualified);
        }
        ProfileKind::Docs | ProfileKind::Tutorial => {
            requirements.insert(ValueDimension::Audience, ClaimMode::Generic);
            requirements.insert(ValueDimension::FirstResult, ClaimMode::Qualified);
        }
        ProfileKind::Ml | ProfileKind::Research => {
            requirements.insert(ValueDimension::Outcome, ClaimMode::Qualified);
            requirements.insert(ValueDimension::Evidence, ClaimMode::Qualified);
            requirements.insert(ValueDimension::Constraint, ClaimMode::Qualified);
        }
        ProfileKind::Template => {
            requirements.insert(ValueDimension::Audience, ClaimMode::Generic);
            requirements.insert(ValueDimension::Outcome, ClaimMode::Generic);
        }
        ProfileKind::Common => {}
    }
    ClaimFloor { requirements }
}

fn capability_ceiling(capabilities: &RepositoryCapabilityIR) -> ClaimCeiling {
    let mut limits = ValueDimension::ALL
        .into_iter()
        .map(|dimension| (dimension, ClaimMode::Omitted))
        .collect::<BTreeMap<_, _>>();
    for node in capabilities
        .nodes
        .iter()
        .filter(|node| node.support == seiri_core::CapabilitySupport::Observed)
    {
        let (dimension, mode) = match node.kind {
            CapabilityKind::Manifest | CapabilityKind::ProgramLanguage => {
                (ValueDimension::Identity, ClaimMode::Direct)
            }
            CapabilityKind::Entrypoint
            | CapabilityKind::PublicApi
            | CapabilityKind::Operation
            | CapabilityKind::Input
            | CapabilityKind::Feature => (ValueDimension::Capability, ClaimMode::Direct),
            CapabilityKind::Output => (ValueDimension::Outcome, ClaimMode::Qualified),
            CapabilityKind::FirstResult => (ValueDimension::FirstResult, ClaimMode::Qualified),
            CapabilityKind::Example => (ValueDimension::FirstAction, ClaimMode::Qualified),
            CapabilityKind::Test => (ValueDimension::Evidence, ClaimMode::Direct),
            CapabilityKind::Constraint => (ValueDimension::Constraint, ClaimMode::Qualified),
        };
        let current = limits
            .get(&dimension)
            .copied()
            .unwrap_or(ClaimMode::Omitted);
        limits.insert(dimension, current.max(mode));
    }
    ClaimCeiling { limits }
}

fn support_state(
    capabilities: &RepositoryCapabilityIR,
    capability_nodes: &[CapabilityNodeId],
    evidence_count: usize,
) -> SupportState {
    if !capability_nodes.is_empty() && evidence_count > 0 {
        SupportState::Supported
    } else {
        match capabilities.coverage {
            CoverageStatus::Complete => SupportState::InsufficientEvidence,
            CoverageStatus::NotRequested => SupportState::Unknown(UnknownReason::NotRequested),
            CoverageStatus::Partial(reason) => SupportState::Unknown(unknown_reason(reason)),
        }
    }
}

fn capability_nodes_for(
    capabilities: &RepositoryCapabilityIR,
    dimension: ValueDimension,
) -> Vec<CapabilityNodeId> {
    capabilities
        .nodes
        .iter()
        .filter(|node| {
            node.kind.dimension() == dimension
                || (dimension == ValueDimension::FirstAction
                    && node.kind == CapabilityKind::Example)
                || (dimension == ValueDimension::Evidence && node.kind == CapabilityKind::Example)
        })
        .map(|node| node.id)
        .collect()
}

fn realized_mode(grammar: &ReadmeGrammarIR, dimension: ValueDimension) -> ClaimMode {
    grammar
        .nodes
        .iter()
        .filter(|node| node.predicate.dimension() == dimension && !node.negated)
        .map(|node| match (node.state, node.modality) {
            (AnswerState::Explicit, ClaimModality::Asserted) => ClaimMode::Direct,
            (AnswerState::Explicit, _) => ClaimMode::Qualified,
            (AnswerState::Inferred, _) => ClaimMode::Generic,
            (AnswerState::Missing | AnswerState::Contested | AnswerState::Unknown(_), _) => {
                ClaimMode::Omitted
            }
        })
        .max()
        .unwrap_or(ClaimMode::Omitted)
}

const fn automatic_gate(dimension: ValueDimension) -> GateKind {
    match dimension {
        ValueDimension::Identity
        | ValueDimension::Capability
        | ValueDimension::FirstAction
        | ValueDimension::FirstResult
        | ValueDimension::Evidence
        | ValueDimension::Constraint => GateKind::Guarded,
        ValueDimension::Audience
        | ValueDimension::Problem
        | ValueDimension::Outcome
        | ValueDimension::Differentiation => GateKind::Manual,
    }
}

const fn overclaim_kind(dimension: ValueDimension) -> Option<OverclaimRiskKind> {
    match dimension {
        ValueDimension::Audience => Some(OverclaimRiskKind::AudienceNotObserved),
        ValueDimension::Capability | ValueDimension::FirstAction => {
            Some(OverclaimRiskKind::UnsupportedCapability)
        }
        ValueDimension::Outcome | ValueDimension::FirstResult => {
            Some(OverclaimRiskKind::UnsupportedOutcome)
        }
        ValueDimension::Differentiation => Some(OverclaimRiskKind::PerformanceNotMeasured),
        ValueDimension::Identity
        | ValueDimension::Problem
        | ValueDimension::Evidence
        | ValueDimension::Constraint => None,
    }
}

fn answer_state(
    coverage: CoverageStatus,
    dimension: ValueDimension,
    nodes: &[&GrammarNode],
) -> AnswerState {
    if nodes.is_empty() {
        return match coverage {
            CoverageStatus::Complete => AnswerState::Missing,
            CoverageStatus::NotRequested => AnswerState::Unknown(UnknownReason::NotRequested),
            CoverageStatus::Partial(reason) => AnswerState::Unknown(unknown_reason(reason)),
        };
    }
    let positive = nodes.iter().any(|node| !node.negated);
    let negative = nodes.iter().any(|node| node.negated);
    if dimension != ValueDimension::Constraint && positive && negative {
        AnswerState::Contested
    } else if nodes.iter().any(|node| node.state == AnswerState::Explicit) {
        AnswerState::Explicit
    } else {
        AnswerState::Inferred
    }
}

const fn unknown_reason(reason: CoverageIncompleteReason) -> UnknownReason {
    match reason {
        CoverageIncompleteReason::LimitExceeded => UnknownReason::LimitExceeded,
        CoverageIncompleteReason::InvalidUtf8 => UnknownReason::InvalidUtf8,
        CoverageIncompleteReason::ParseFailed => UnknownReason::ParseFailed,
        CoverageIncompleteReason::UnsupportedSyntax => UnknownReason::UnsupportedSyntax,
        CoverageIncompleteReason::PermissionDenied => UnknownReason::PermissionDenied,
        CoverageIncompleteReason::RateLimited => UnknownReason::RateLimited,
        CoverageIncompleteReason::Unavailable => UnknownReason::Unavailable,
    }
}

fn add_disconnect_opportunities(
    grammar: &ReadmeGrammarIR,
    dimensions: &BTreeMap<ValueDimension, AnswerState>,
    opportunities: &mut Vec<UnderclaimOpportunity>,
) {
    if is_present(dimensions, ValueDimension::Capability)
        && is_missing(dimensions, ValueDimension::Outcome)
    {
        opportunities.push(opportunity(
            UnderclaimOpportunityKind::CapabilityOutcomeDisconnect,
            ValueDimension::Outcome,
            GateKind::Manual,
            nodes_for(grammar, ValueDimension::Capability),
        ));
    }
    if is_present(dimensions, ValueDimension::FirstAction)
        && is_missing(dimensions, ValueDimension::FirstResult)
    {
        opportunities.push(opportunity(
            UnderclaimOpportunityKind::FirstValueDisconnect,
            ValueDimension::FirstResult,
            GateKind::Manual,
            nodes_for(grammar, ValueDimension::FirstAction),
        ));
    }
    if is_present(dimensions, ValueDimension::Evidence)
        && !is_present(dimensions, ValueDimension::Capability)
        && !is_present(dimensions, ValueDimension::Outcome)
    {
        opportunities.push(opportunity(
            UnderclaimOpportunityKind::EvidenceDisconnect,
            ValueDimension::Evidence,
            GateKind::Safe,
            nodes_for(grammar, ValueDimension::Evidence),
        ));
    }
}

fn add_narrative_opportunities(
    grammar: &ReadmeGrammarIR,
    opportunities: &mut Vec<UnderclaimOpportunity>,
) {
    let primary = grammar
        .nodes
        .iter()
        .filter(|node| {
            matches!(
                node.predicate.dimension(),
                ValueDimension::Identity
                    | ValueDimension::Capability
                    | ValueDimension::Outcome
                    | ValueDimension::FirstResult
            )
        })
        .collect::<Vec<_>>();
    if let Some(first) = primary.first() {
        if first.span.is_some_and(|span| span.line > 20) {
            opportunities.push(opportunity(
                UnderclaimOpportunityKind::BuriedPrimaryValue,
                first.predicate.dimension(),
                GateKind::Safe,
                vec![first.id],
            ));
        }
    }

    let constraints = nodes_for(grammar, ValueDimension::Constraint);
    let value_count = nodes_for(grammar, ValueDimension::Capability)
        .len()
        .saturating_add(nodes_for(grammar, ValueDimension::Outcome).len());
    if !constraints.is_empty() && constraints.len() > value_count {
        opportunities.push(opportunity(
            UnderclaimOpportunityKind::QualifierDominance,
            ValueDimension::Capability,
            GateKind::Safe,
            constraints,
        ));
    }

    if let (Some(first), Some(last)) = (primary.first(), primary.last()) {
        if first
            .span
            .zip(last.span)
            .is_some_and(|(left, right)| right.line.saturating_sub(left.line) > 60)
        {
            opportunities.push(opportunity(
                UnderclaimOpportunityKind::FragmentedValue,
                ValueDimension::Outcome,
                GateKind::Safe,
                primary.iter().map(|node| node.id).collect(),
            ));
        }
    }
}

fn add_translation_opportunities(
    grammar: &ReadmeGrammarIR,
    opportunities: &mut Vec<UnderclaimOpportunity>,
) {
    let japanese = dimensions_for_language(grammar, DocumentLanguage::Japanese);
    let english = dimensions_for_language(grammar, DocumentLanguage::English);
    if japanese.is_empty() || english.is_empty() {
        return;
    }
    for dimension in japanese.symmetric_difference(&english).copied() {
        opportunities.push(opportunity(
            UnderclaimOpportunityKind::TranslationDivergence,
            dimension,
            GateKind::Guarded,
            nodes_for(grammar, dimension),
        ));
    }
}

fn narrative_topology(
    grammar: &ReadmeGrammarIR,
    dimensions: &BTreeMap<ValueDimension, AnswerState>,
) -> NarrativeTopologyReport {
    let mut first_value_path = [
        ValueDimension::Identity,
        ValueDimension::Audience,
        ValueDimension::Problem,
        ValueDimension::Capability,
        ValueDimension::Outcome,
        ValueDimension::FirstAction,
        ValueDimension::FirstResult,
        ValueDimension::Evidence,
        ValueDimension::Constraint,
    ]
    .into_iter()
    .filter_map(|dimension| {
        grammar
            .nodes
            .iter()
            .filter(|node| node.predicate.dimension() == dimension)
            .min_by_key(|node| node.span.map_or(usize::MAX, |span| span.byte_start))
            .map(|node| node.id)
    })
    .collect::<Vec<_>>();
    first_value_path.sort_by_key(|id| {
        grammar
            .nodes
            .iter()
            .find(|node| node.id == *id)
            .and_then(|node| node.span)
            .map_or(usize::MAX, |span| span.byte_start)
    });

    let mut disconnected_dimensions = Vec::new();
    if is_present(dimensions, ValueDimension::Capability)
        && is_missing(dimensions, ValueDimension::Outcome)
    {
        disconnected_dimensions.push(ValueDimension::Outcome);
    }
    if is_present(dimensions, ValueDimension::FirstAction)
        && is_missing(dimensions, ValueDimension::FirstResult)
    {
        disconnected_dimensions.push(ValueDimension::FirstResult);
    }
    let japanese = dimensions_for_language(grammar, DocumentLanguage::Japanese);
    let english = dimensions_for_language(grammar, DocumentLanguage::English);
    let translation_divergences = if japanese.is_empty() || english.is_empty() {
        0
    } else {
        japanese.symmetric_difference(&english).count()
    };
    NarrativeTopologyReport {
        semantic_revision: seiri_core::NARRATIVE_TOPOLOGY_REVISION.to_string(),
        first_value_path,
        disconnected_dimensions,
        translation_divergences,
    }
}

fn dimensions_for_language(
    grammar: &ReadmeGrammarIR,
    language: DocumentLanguage,
) -> BTreeSet<ValueDimension> {
    grammar
        .nodes
        .iter()
        .filter(|node| node.language == language)
        .map(|node| node.predicate.dimension())
        .collect()
}

fn nodes_for(grammar: &ReadmeGrammarIR, dimension: ValueDimension) -> Vec<GrammarNodeId> {
    grammar
        .nodes
        .iter()
        .filter(|node| node.predicate.dimension() == dimension)
        .map(|node| node.id)
        .collect()
}

fn opportunity(
    kind: UnderclaimOpportunityKind,
    dimension: ValueDimension,
    gate: GateKind,
    grammar_nodes: Vec<GrammarNodeId>,
) -> UnderclaimOpportunity {
    UnderclaimOpportunity {
        kind,
        dimension,
        gate,
        grammar_nodes,
        capability_nodes: Vec::new(),
    }
}

fn opportunity_key(
    opportunity: &UnderclaimOpportunity,
) -> (UnderclaimOpportunityKind, ValueDimension, u8) {
    (
        opportunity.kind,
        opportunity.dimension,
        gate_rank(opportunity.gate),
    )
}

const fn gate_rank(gate: GateKind) -> u8 {
    match gate {
        GateKind::Safe => 0,
        GateKind::Guarded => 1,
        GateKind::Manual => 2,
    }
}

fn is_present(
    dimensions: &BTreeMap<ValueDimension, AnswerState>,
    dimension: ValueDimension,
) -> bool {
    matches!(
        dimensions.get(&dimension),
        Some(AnswerState::Explicit | AnswerState::Inferred)
    )
}

fn is_missing(
    dimensions: &BTreeMap<ValueDimension, AnswerState>,
    dimension: ValueDimension,
) -> bool {
    dimensions.get(&dimension) == Some(&AnswerState::Missing)
}

#[cfg(test)]
mod tests {
    use super::*;
    use seiri_core::{
        CapabilityNode, CapabilityNodeId, CapabilityProvenance, CapabilityProvenanceKind,
        CapabilitySupport, ClaimModality, GrammarNode, GrammarNodeId, GrammarPredicate, SourceSpan,
    };
    use std::num::NonZeroU32;

    fn grammar(
        coverage: CoverageStatus,
        predicates: &[(GrammarPredicate, DocumentLanguage, usize)],
    ) -> ReadmeGrammarIR {
        let nodes = predicates
            .iter()
            .enumerate()
            .map(|(index, (predicate, language, line))| GrammarNode {
                id: GrammarNodeId::new(NonZeroU32::new((index + 1) as u32).expect("id")),
                predicate: *predicate,
                state: AnswerState::Explicit,
                language: *language,
                modality: ClaimModality::Asserted,
                negated: false,
                span: Some(SourceSpan::new(*line, 1, index * 10, index * 10 + 5)),
            })
            .collect();
        ReadmeGrammarIR::try_new("README.md", coverage, nodes, Vec::new(), Vec::new())
            .expect("grammar")
    }

    fn capabilities(coverage: CoverageStatus, kinds: &[CapabilityKind]) -> RepositoryCapabilityIR {
        let nodes = kinds
            .iter()
            .enumerate()
            .map(|(index, kind)| CapabilityNode {
                id: CapabilityNodeId::new(NonZeroU32::new((index + 1) as u32).expect("id")),
                kind: *kind,
                support: CapabilitySupport::Observed,
                symbol: format!("symbol-{index}"),
                provenance: vec![CapabilityProvenance {
                    path: "src/lib.rs".to_string(),
                    kind: CapabilityProvenanceKind::SourceSyntax,
                    span: Some(SourceSpan::new(index + 1, 1, index * 10, index * 10 + 5)),
                }],
            })
            .collect();
        RepositoryCapabilityIR::try_new(coverage, nodes, Vec::new(), Vec::new())
            .expect("capabilities")
    }

    #[test]
    fn document_shadow_detects_capability_outcome_disconnect() {
        let report = analyze_document_value_coverage(&grammar(
            CoverageStatus::Complete,
            &[(
                GrammarPredicate::PerformsOperation,
                DocumentLanguage::English,
                3,
            )],
        ));
        assert_eq!(
            report.dimensions[&ValueDimension::Outcome],
            AnswerState::Missing
        );
        assert!(report.opportunities.iter().any(|opportunity| {
            opportunity.kind == UnderclaimOpportunityKind::CapabilityOutcomeDisconnect
        }));
    }

    #[test]
    fn incomplete_coverage_preserves_unknown_instead_of_missing() {
        let report = analyze_document_value_coverage(&grammar(
            CoverageStatus::Partial(CoverageIncompleteReason::LimitExceeded),
            &[],
        ));
        assert_eq!(
            report.dimensions[&ValueDimension::Audience],
            AnswerState::Unknown(UnknownReason::LimitExceeded)
        );
        assert!(report.opportunities.is_empty());
    }

    #[test]
    fn bilingual_dimension_difference_is_not_silently_merged() {
        let report = analyze_document_value_coverage(&grammar(
            CoverageStatus::Complete,
            &[
                (
                    GrammarPredicate::DefinesIdentity,
                    DocumentLanguage::Japanese,
                    2,
                ),
                (
                    GrammarPredicate::PerformsOperation,
                    DocumentLanguage::Japanese,
                    3,
                ),
                (
                    GrammarPredicate::DefinesIdentity,
                    DocumentLanguage::English,
                    20,
                ),
            ],
        ));
        assert_eq!(report.narrative.translation_divergences, 1);
        assert!(report.opportunities.iter().any(|opportunity| {
            opportunity.kind == UnderclaimOpportunityKind::TranslationDivergence
        }));
    }

    #[test]
    fn membrane_finds_supported_value_missing_from_readme() {
        let membrane = evaluate_claim_capability_membrane(
            &grammar(CoverageStatus::Complete, &[]),
            &capabilities(CoverageStatus::Complete, &[CapabilityKind::Operation]),
            ProfileKind::Common,
        );
        assert!(membrane.opportunities.iter().any(|opportunity| {
            opportunity.kind == UnderclaimOpportunityKind::MissingSupportedValue
                && opportunity.dimension == ValueDimension::Capability
        }));
        assert_eq!(
            membrane.ceiling.limits[&ValueDimension::Capability],
            ClaimMode::Direct
        );
        assert_eq!(membrane.losses.missing_supported_values, 1);
    }

    #[test]
    fn membrane_keeps_incomplete_program_analysis_unknown() {
        let membrane = evaluate_claim_capability_membrane(
            &grammar(CoverageStatus::Complete, &[]),
            &capabilities(
                CoverageStatus::Partial(CoverageIncompleteReason::LimitExceeded),
                &[],
            ),
            ProfileKind::Common,
        );
        assert!(membrane.relations.iter().all(|relation| matches!(
            relation.state,
            SupportState::Unknown(UnknownReason::LimitExceeded)
        )));
        assert!(!membrane.opportunities.iter().any(|opportunity| {
            opportunity.kind == UnderclaimOpportunityKind::MissingSupportedValue
        }));
    }

    #[test]
    fn membrane_marks_outcome_above_evidence_ceiling() {
        let membrane = evaluate_claim_capability_membrane(
            &grammar(
                CoverageStatus::Complete,
                &[(
                    GrammarPredicate::ProducesOutcome,
                    DocumentLanguage::English,
                    3,
                )],
            ),
            &capabilities(CoverageStatus::Complete, &[CapabilityKind::Operation]),
            ProfileKind::Common,
        );
        assert!(membrane.risks.iter().any(|risk| {
            risk.kind == OverclaimRiskKind::UnsupportedOutcome
                && risk.dimension == ValueDimension::Outcome
        }));
    }

    #[test]
    fn tests_raise_only_the_evidence_ceiling_not_quality_or_outcome() {
        let membrane = evaluate_claim_capability_membrane(
            &grammar(CoverageStatus::Complete, &[]),
            &capabilities(CoverageStatus::Complete, &[CapabilityKind::Test]),
            ProfileKind::Common,
        );
        assert_eq!(
            membrane.ceiling.limits[&ValueDimension::Evidence],
            ClaimMode::Direct
        );
        assert_eq!(
            membrane.ceiling.limits[&ValueDimension::Outcome],
            ClaimMode::Omitted
        );
        assert_eq!(
            membrane.ceiling.limits[&ValueDimension::Differentiation],
            ClaimMode::Omitted
        );
    }
}

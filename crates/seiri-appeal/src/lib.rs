#![forbid(unsafe_code)]

mod geometry;
mod incremental;
mod input_projection;
mod performance_receipt;

pub use geometry::{
    rank_appeal_presentation, GeometryShadowOptions, GeometryShadowRanking, GeometryShadowSignal,
    GEOMETRY_SHADOW_REVISION,
};

pub use incremental::{
    membrane_semantic_digest, update_incremental_membrane, verify_incremental_against_scalar,
    AppealDependencyIndex, IncrementalEquivalence, IncrementalMembraneState,
    IncrementalMembraneUpdate, IncrementalUpdateMode,
};
pub use input_projection::CapabilityEvaluationIndex;
pub use performance_receipt::{
    DeterministicPerformanceReceipt, PerformanceReceiptContext, PerformanceReceiptError,
    PerformanceUpdateMode, PERFORMANCE_RECEIPT_REVISION,
};

use seiri_core::{
    AnswerState, AppealLossVector, CapabilityNodeId, CapabilitySemanticSignature,
    CapabilitySupport, ClaimAtom, ClaimCapabilityAlignment, ClaimCapabilityMembrane, ClaimCeiling,
    ClaimFloor, ClaimModality, ClaimMode, ClaimPolarity, ClaimRealization,
    CoverageIncompleteReason, CoverageStatus, DocumentLanguage, EvidenceCeilingContribution,
    GateKind, GrammarNode, GrammarNodeId, NarrativeTopologyReport, OverclaimRisk,
    OverclaimRiskKind, ProfileKind, ReadmeGrammarIR, ReadmeValueCoverageReport,
    RepositoryCapabilityIR, SupportRelation, SupportState, TranslationAlignmentState,
    UnderclaimOpportunity, UnderclaimOpportunityKind, UnknownReason, ValueDimension,
    CLAIM_CAPABILITY_MEMBRANE_REVISION, VALUE_COVERAGE_REVISION,
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
pub fn evaluate_claim_capability_membrane(
    grammar: &ReadmeGrammarIR,
    capabilities: &RepositoryCapabilityIR,
    profile: ProfileKind,
) -> Result<ClaimCapabilityMembrane, seiri_core::AppealIrError> {
    grammar.validate()?;
    capabilities.validate()?;
    let capability_inputs = CapabilityEvaluationIndex::try_new(capabilities)?;

    let document = analyze_document_value_coverage(grammar);
    let floor = profile_floor(profile);
    let ceiling = capability_ceiling(&capability_inputs)?;
    let alignments = evaluate_claim_alignments(grammar, &capability_inputs)?;
    let realizations = claim_realizations(grammar)?;
    let mut relations = Vec::new();
    let mut opportunities = document.opportunities.clone();
    let mut risks = Vec::new();

    let dimension_inputs = DimensionEvaluationInputs {
        grammar,
        capabilities,
        capability_inputs: &capability_inputs,
        floor: &floor,
        ceiling: &ceiling,
        alignments: &alignments,
        realizations: &realizations,
    };
    for dimension in ValueDimension::ALL {
        let evaluation = evaluate_dimension(&dimension_inputs, dimension)?;
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
        above_ceiling_dimensions: risks
            .iter()
            .map(|risk| risk.dimension)
            .collect::<BTreeSet<_>>()
            .len(),
        disconnected_value_paths: document.narrative.disconnected_dimensions.len(),
        translation_divergences: document.narrative.translation_divergences,
    };
    let membrane = ClaimCapabilityMembrane {
        semantic_revision: CLAIM_CAPABILITY_MEMBRANE_REVISION.to_string(),
        floor,
        ceiling,
        relations,
        alignments,
        opportunities,
        risks,
        losses,
    };
    membrane.validate()?;
    Ok(membrane)
}

struct DimensionEvaluation {
    relation: SupportRelation,
    opportunities: Vec<UnderclaimOpportunity>,
    risk: Option<OverclaimRisk>,
}

struct DimensionEvaluationInputs<'a> {
    grammar: &'a ReadmeGrammarIR,
    capabilities: &'a RepositoryCapabilityIR,
    capability_inputs: &'a CapabilityEvaluationIndex<'a>,
    floor: &'a ClaimFloor,
    ceiling: &'a ClaimCeiling,
    alignments: &'a [ClaimCapabilityAlignment],
    realizations: &'a [ClaimRealization],
}

fn evaluate_dimension(
    inputs: &DimensionEvaluationInputs<'_>,
    dimension: ValueDimension,
) -> Result<DimensionEvaluation, seiri_core::AppealIrError> {
    let grammar = inputs.grammar;
    let capabilities = inputs.capabilities;
    let capability_inputs = inputs.capability_inputs;
    let floor = inputs.floor;
    let ceiling = inputs.ceiling;
    let alignments = inputs.alignments;
    let realizations = inputs.realizations;
    let document_nodes = grammar
        .nodes
        .iter()
        .filter(|node| node.predicate.dimension() == dimension)
        .collect::<Vec<_>>();
    let document_state = answer_state(grammar.coverage, dimension, &document_nodes);
    let grammar_nodes = nodes_for(grammar, dimension);
    let atom_ids = grammar
        .claim_atoms
        .atoms
        .iter()
        .filter(|atom| atom.dimension == dimension)
        .map(|atom| atom.id)
        .collect::<BTreeSet<_>>();
    let matching_alignments = alignments
        .iter()
        .filter(|alignment| atom_ids.contains(&alignment.claim_atom))
        .collect::<Vec<_>>();
    let mut capability_nodes = if grammar_nodes.is_empty() {
        signature_nodes_for_dimension(capability_inputs, dimension)
    } else {
        matching_alignments
            .iter()
            .flat_map(|alignment| alignment.capability_nodes.iter().copied())
            .collect::<Vec<_>>()
    };
    capability_nodes.sort_unstable();
    capability_nodes.dedup();
    let evidence_count = capability_nodes
        .iter()
        .filter_map(|id| capability_inputs.node(*id))
        .map(|node| node.provenance.len())
        .sum();
    let state = if grammar_nodes.is_empty() {
        signature_support_state(capability_inputs, dimension)
    } else {
        aggregate_alignment_state(capabilities, &matching_alignments, atom_ids.len())
    };
    let relation = SupportRelation::try_new(
        dimension,
        grammar_nodes.clone(),
        capability_nodes.clone(),
        evidence_count,
        state,
    )?;
    let realized = realized_mode(grammar, realizations, dimension);
    let required = floor
        .requirements
        .get(&dimension)
        .copied()
        .unwrap_or(ClaimMode::Omitted);
    let allowed = if grammar_nodes.is_empty() {
        ceiling
            .limits
            .get(&dimension)
            .copied()
            .unwrap_or(ClaimMode::Omitted)
    } else {
        claim_specific_ceiling(&matching_alignments, atom_ids.len())
    };
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
    let risk = if realized != ClaimMode::Omitted
        && (state != SupportState::Supported || realized > allowed)
    {
        Some(OverclaimRisk {
            kind: overclaim_kind(grammar, dimension),
            dimension,
            grammar_nodes,
        })
    } else {
        None
    };
    Ok(DimensionEvaluation {
        relation,
        opportunities,
        risk,
    })
}

fn evaluate_claim_alignments(
    grammar: &ReadmeGrammarIR,
    capability_inputs: &CapabilityEvaluationIndex<'_>,
) -> Result<Vec<ClaimCapabilityAlignment>, seiri_core::AppealIrError> {
    grammar
        .claim_atoms
        .atoms
        .iter()
        .map(|atom| evaluate_claim_alignment(atom, capability_inputs))
        .collect()
}

fn evaluate_claim_alignment(
    atom: &ClaimAtom,
    capability_inputs: &CapabilityEvaluationIndex<'_>,
) -> Result<ClaimCapabilityAlignment, seiri_core::AppealIrError> {
    let capabilities = capability_inputs.capabilities();
    let matching = capability_inputs
        .signatures(atom.dimension)
        .iter()
        .copied()
        .filter(|signature| semantic_signature_matches(atom, signature, capability_inputs))
        .collect::<Vec<_>>();
    let supported = matching
        .iter()
        .copied()
        .filter(|signature| {
            signature.polarity == atom.polarity
                && signature_has_evidence(signature, capability_inputs)
        })
        .collect::<Vec<_>>();
    let contradicted = matching
        .iter()
        .copied()
        .filter(|signature| {
            signature.polarity != atom.polarity
                && signature_has_evidence(signature, capability_inputs)
        })
        .collect::<Vec<_>>();
    let matching_unknown = matching
        .iter()
        .find_map(|signature| signature_unknown_reason(signature, capability_inputs));

    let coverage_unknown = match capabilities.coverage {
        CoverageStatus::Complete => None,
        CoverageStatus::NotRequested => Some(UnknownReason::NotRequested),
        CoverageStatus::Partial(reason) => Some(unknown_reason(reason)),
    };
    let (selected, state) =
        if let (ClaimPolarity::Negative, Some(reason)) = (atom.polarity, coverage_unknown) {
            (matching.clone(), SupportState::Unknown(reason))
        } else if !supported.is_empty() && !contradicted.is_empty() {
            let mut selected = supported.clone();
            selected.extend(contradicted.iter().copied());
            (selected, SupportState::Contradicted)
        } else if !supported.is_empty() {
            (supported, SupportState::Supported)
        } else if !contradicted.is_empty() {
            (contradicted, SupportState::Contradicted)
        } else if let Some(reason) = matching_unknown {
            (matching, SupportState::Unknown(reason))
        } else {
            let state = match capabilities.coverage {
                CoverageStatus::Complete => SupportState::InsufficientEvidence,
                CoverageStatus::NotRequested => SupportState::Unknown(UnknownReason::NotRequested),
                CoverageStatus::Partial(reason) => SupportState::Unknown(unknown_reason(reason)),
            };
            (Vec::new(), state)
        };
    let mut capability_nodes = selected
        .iter()
        .map(|signature| signature.capability_node)
        .collect::<Vec<_>>();
    capability_nodes.sort_unstable();
    capability_nodes.dedup();
    let evidence_count = capability_nodes
        .iter()
        .filter_map(|id| capability_inputs.node(*id))
        .map(|node| node.provenance.len())
        .sum();
    let contributions = if state == SupportState::Supported {
        selected
            .iter()
            .map(|signature| {
                let owner = capability_inputs.node(signature.capability_node).ok_or(
                    seiri_core::AppealIrError::DanglingCapabilitySemanticSignatureOwner(
                        signature.capability_node,
                    ),
                )?;
                EvidenceCeilingContribution::try_new(
                    owner,
                    atom.dimension,
                    effective_semantic_support(owner.support, signature.semantic_support),
                )
                .map_err(|_| {
                    seiri_core::AppealIrError::EvidenceCeilingDimensionMismatch(
                        signature.capability_node,
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?
    } else {
        Vec::new()
    };
    ClaimCapabilityAlignment::try_new_with_evidence_ceiling(
        atom.id,
        capability_nodes,
        evidence_count,
        state,
        &contributions,
    )
}

const fn effective_semantic_support(
    owner: CapabilitySupport,
    semantic: CapabilitySupport,
) -> CapabilitySupport {
    match (owner, semantic) {
        (CapabilitySupport::Unknown(reason), _) | (_, CapabilitySupport::Unknown(reason)) => {
            CapabilitySupport::Unknown(reason)
        }
        (CapabilitySupport::Inferred, _) | (_, CapabilitySupport::Inferred) => {
            CapabilitySupport::Inferred
        }
        (CapabilitySupport::Observed, CapabilitySupport::Observed) => CapabilitySupport::Observed,
    }
}

fn semantic_signature_matches(
    atom: &ClaimAtom,
    signature: &CapabilitySemanticSignature,
    capability_inputs: &CapabilityEvaluationIndex<'_>,
) -> bool {
    if atom.dimension != signature.dimension {
        return false;
    }
    let action_matches = atom
        .action
        .as_deref()
        .zip(signature.action.as_deref())
        .is_some_and(|(claim, capability)| semantic_tokens(claim) == semantic_tokens(capability));
    if !action_matches {
        return false;
    }
    if let Some(object) = atom.object.as_deref() {
        let Some(capability_object) = signature.object.as_deref() else {
            return false;
        };
        if !semantic_subset(object, capability_object) {
            return false;
        }
    }
    if let (Some(subject), Some(capability_subject)) =
        (atom.subject.as_deref(), signature.subject.as_deref())
    {
        if !semantic_subset(subject, capability_subject) {
            return false;
        }
    }
    if atom.qualifiers.iter().any(|qualifier| {
        !signature
            .qualifiers
            .iter()
            .any(|candidate| semantic_tokens(qualifier) == semantic_tokens(candidate))
    }) {
        return false;
    }
    atom.condition.as_deref().is_none_or(|condition| {
        let observed = signature
            .condition_nodes
            .iter()
            .filter_map(|id| capability_inputs.node(*id))
            .map(|node| node.symbol.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        semantic_subset(condition, &observed)
    })
}

fn semantic_subset(claim: &str, capability: &str) -> bool {
    let claim = semantic_tokens(claim);
    let capability = semantic_tokens(capability)
        .into_iter()
        .collect::<BTreeSet<_>>();
    !claim.is_empty() && claim.iter().all(|token| capability.contains(token))
}

fn semantic_tokens(value: &str) -> Vec<String> {
    value
        .split(|character: char| !character.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(|token| canonical_semantic_token(&token.to_ascii_lowercase()))
        .filter(|token| !matches!(token.as_str(), "a" | "an" | "the"))
        .collect()
}

fn canonical_semantic_token(token: &str) -> String {
    match token {
        "analyzes" | "analyzing" | "analysis" => "analyze".to_string(),
        "audits" | "auditing" => "audit".to_string(),
        "deletes" | "deleting" => "delete".to_string(),
        "detects" | "detecting" => "detect".to_string(),
        "generates" | "generating" => "generate".to_string(),
        "inspects" | "inspecting" => "inspect".to_string(),
        "organizes" | "organizing" => "organize".to_string(),
        "repositories" | "repo" | "repos" => "repository".to_string(),
        "reviews" | "reviewing" => "review".to_string(),
        "scans" | "scanning" => "scan".to_string(),
        _ if token.len() > 3 && token.ends_with("ies") => {
            format!("{}y", &token[..token.len() - 3])
        }
        _ if token.len() > 3 && token.ends_with('s') => token[..token.len() - 1].to_string(),
        _ => token.to_string(),
    }
}

fn signature_has_evidence(
    signature: &CapabilitySemanticSignature,
    capability_inputs: &CapabilityEvaluationIndex<'_>,
) -> bool {
    matches!(
        signature.semantic_support,
        CapabilitySupport::Observed | CapabilitySupport::Inferred
    ) && capability_inputs
        .node(signature.capability_node)
        .is_some_and(|node| {
            matches!(
                node.support,
                CapabilitySupport::Observed | CapabilitySupport::Inferred
            ) && !node.provenance.is_empty()
        })
}

fn signature_unknown_reason(
    signature: &CapabilitySemanticSignature,
    capability_inputs: &CapabilityEvaluationIndex<'_>,
) -> Option<UnknownReason> {
    match signature.semantic_support {
        CapabilitySupport::Unknown(reason) => Some(reason),
        CapabilitySupport::Observed | CapabilitySupport::Inferred => capability_inputs
            .node(signature.capability_node)
            .and_then(|node| match node.support {
                CapabilitySupport::Unknown(reason) => Some(reason),
                CapabilitySupport::Observed | CapabilitySupport::Inferred => None,
            }),
    }
}

fn aggregate_alignment_state(
    capabilities: &RepositoryCapabilityIR,
    alignments: &[&ClaimCapabilityAlignment],
    expected_atoms: usize,
) -> SupportState {
    if expected_atoms == 0 || alignments.len() != expected_atoms {
        return match capabilities.coverage {
            CoverageStatus::Complete => SupportState::InsufficientEvidence,
            CoverageStatus::NotRequested => SupportState::Unknown(UnknownReason::NotRequested),
            CoverageStatus::Partial(reason) => SupportState::Unknown(unknown_reason(reason)),
        };
    }
    if alignments
        .iter()
        .any(|alignment| alignment.state == SupportState::Contradicted)
    {
        SupportState::Contradicted
    } else if alignments
        .iter()
        .all(|alignment| alignment.state == SupportState::Supported)
    {
        SupportState::Supported
    } else if let Some(reason) = alignments
        .iter()
        .find_map(|alignment| match alignment.state {
            SupportState::Unknown(reason) => Some(reason),
            SupportState::Supported
            | SupportState::InsufficientEvidence
            | SupportState::Contradicted => None,
        })
    {
        SupportState::Unknown(reason)
    } else {
        SupportState::InsufficientEvidence
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

fn capability_ceiling(
    capability_inputs: &CapabilityEvaluationIndex<'_>,
) -> Result<ClaimCeiling, seiri_core::AppealIrError> {
    let mut limits = ValueDimension::ALL
        .into_iter()
        .map(|dimension| (dimension, ClaimMode::Omitted))
        .collect::<BTreeMap<_, _>>();
    for dimension in ValueDimension::ALL {
        for node in capability_inputs
            .direct_nodes(dimension)
            .into_iter()
            .filter(|node| !node.provenance.is_empty())
        {
            let contribution = EvidenceCeilingContribution::try_new(node, dimension, node.support)?;
            let current = limits
                .get(&dimension)
                .copied()
                .unwrap_or(ClaimMode::Omitted);
            limits.insert(dimension, current.max(contribution.maximum()));
        }
    }
    for dimension in ValueDimension::ALL {
        let signatures = capability_inputs.signatures(dimension);
        if signatures.is_empty() {
            continue;
        }
        let semantic_limit = signatures
            .iter()
            .filter_map(|signature| {
                let owner = capability_inputs.node(signature.capability_node)?;
                if owner.provenance.is_empty() {
                    return None;
                }
                EvidenceCeilingContribution::try_new(
                    owner,
                    dimension,
                    effective_semantic_support(owner.support, signature.semantic_support),
                )
                .ok()
                .map(|contribution| contribution.maximum())
            })
            .max()
            .unwrap_or(ClaimMode::Omitted);
        limits.insert(dimension, semantic_limit);
    }
    Ok(ClaimCeiling { limits })
}

fn signature_nodes_for_dimension(
    capability_inputs: &CapabilityEvaluationIndex<'_>,
    dimension: ValueDimension,
) -> Vec<CapabilityNodeId> {
    capability_inputs
        .signatures(dimension)
        .iter()
        .map(|signature| signature.capability_node)
        .collect()
}

fn signature_support_state(
    capability_inputs: &CapabilityEvaluationIndex<'_>,
    dimension: ValueDimension,
) -> SupportState {
    let capabilities = capability_inputs.capabilities();
    let signatures = capability_inputs.signatures(dimension);
    if signatures
        .iter()
        .any(|signature| signature_has_evidence(signature, capability_inputs))
    {
        SupportState::Supported
    } else if let Some(reason) = signatures
        .iter()
        .find_map(|signature| signature_unknown_reason(signature, capability_inputs))
    {
        SupportState::Unknown(reason)
    } else {
        match capabilities.coverage {
            CoverageStatus::Complete => SupportState::InsufficientEvidence,
            CoverageStatus::NotRequested => SupportState::Unknown(UnknownReason::NotRequested),
            CoverageStatus::Partial(reason) => SupportState::Unknown(unknown_reason(reason)),
        }
    }
}

fn claim_specific_ceiling(
    alignments: &[&ClaimCapabilityAlignment],
    expected_atoms: usize,
) -> ClaimMode {
    if expected_atoms == 0 || alignments.len() != expected_atoms {
        return ClaimMode::Omitted;
    }
    alignments
        .iter()
        .map(|alignment| alignment.claim_ceiling)
        .min()
        .unwrap_or(ClaimMode::Omitted)
}

fn claim_realizations(
    grammar: &ReadmeGrammarIR,
) -> Result<Vec<ClaimRealization>, seiri_core::AppealIrError> {
    grammar
        .claim_atoms
        .atoms
        .iter()
        .map(|atom| {
            let node = grammar
                .nodes
                .iter()
                .find(|node| node.id == atom.grammar_node)
                .ok_or(seiri_core::AppealIrError::DanglingClaimAtomGrammarNode(
                    atom.id,
                ))?;
            ClaimRealization::try_new(atom, node)
        })
        .collect()
}

fn realized_mode(
    grammar: &ReadmeGrammarIR,
    realizations: &[ClaimRealization],
    dimension: ValueDimension,
) -> ClaimMode {
    let atom_mode = realizations
        .iter()
        .filter(|realization| realization.dimension == dimension)
        .map(|realization| realization.mode)
        .max();
    atom_mode.unwrap_or_else(|| {
        grammar
            .nodes
            .iter()
            .filter(|node| node.predicate.dimension() == dimension)
            .map(node_realized_mode)
            .max()
            .unwrap_or(ClaimMode::Omitted)
    })
}

const fn node_realized_mode(node: &GrammarNode) -> ClaimMode {
    match (node.state, node.modality) {
        (AnswerState::Explicit, ClaimModality::Asserted) => ClaimMode::Direct,
        (AnswerState::Explicit, _) => ClaimMode::Qualified,
        (AnswerState::Inferred, _) => ClaimMode::Generic,
        (AnswerState::Missing | AnswerState::Contested | AnswerState::Unknown(_), _) => {
            ClaimMode::Omitted
        }
    }
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

fn overclaim_kind(grammar: &ReadmeGrammarIR, dimension: ValueDimension) -> OverclaimRiskKind {
    if dimension == ValueDimension::Differentiation
        && grammar
            .claim_atoms
            .atoms
            .iter()
            .filter(|atom| atom.dimension == dimension)
            .flat_map(claim_atom_text)
            .any(is_performance_term)
    {
        OverclaimRiskKind::PerformanceNotMeasured
    } else {
        OverclaimRiskKind::for_dimension(dimension)
    }
}

fn claim_atom_text(atom: &ClaimAtom) -> impl Iterator<Item = &str> {
    atom.subject
        .as_deref()
        .into_iter()
        .chain(atom.action.as_deref())
        .chain(atom.object.as_deref())
        .chain(atom.qualifiers.iter().map(String::as_str))
        .chain(atom.condition.as_deref())
}

fn is_performance_term(value: &str) -> bool {
    let value = value.to_ascii_lowercase();
    [
        "fast",
        "faster",
        "performance",
        "latency",
        "throughput",
        "benchmark",
        "speed",
        "高速",
        "性能",
        "レイテンシ",
        "スループット",
    ]
    .iter()
    .any(|term| value.contains(term))
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
    if !grammar.translation_alignment.is_empty() {
        let atoms = grammar
            .claim_atoms
            .atoms
            .iter()
            .map(|atom| (atom.id, atom))
            .collect::<BTreeMap<_, _>>();
        for claim in grammar
            .translation_alignment
            .alignments
            .iter()
            .flat_map(|alignment| &alignment.claims)
            .filter(|claim| claim.state == TranslationAlignmentState::Divergent)
        {
            let Some(source) = atoms.get(&claim.source_claim).copied() else {
                continue;
            };
            let mut grammar_nodes = vec![source.grammar_node];
            if let Some(counterpart) = claim
                .counterpart_claim
                .and_then(|claim_id| atoms.get(&claim_id).copied())
            {
                grammar_nodes.push(counterpart.grammar_node);
            }
            grammar_nodes.sort_unstable();
            grammar_nodes.dedup();
            opportunities.push(opportunity(
                UnderclaimOpportunityKind::TranslationDivergence,
                source.dimension,
                GateKind::Guarded,
                grammar_nodes,
            ));
        }
        return;
    }

    // Backward-compatible fallback for grammar payloads produced before typed,
    // source-bound translation alignment was available.
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
    let translation_divergences = if grammar.translation_alignment.is_empty() {
        let japanese = dimensions_for_language(grammar, DocumentLanguage::Japanese);
        let english = dimensions_for_language(grammar, DocumentLanguage::English);
        if japanese.is_empty() || english.is_empty() {
            0
        } else {
            japanese.symmetric_difference(&english).count()
        }
    } else {
        grammar
            .translation_alignment
            .alignments
            .iter()
            .flat_map(|alignment| &alignment.claims)
            .filter(|claim| claim.state == TranslationAlignmentState::Divergent)
            .count()
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
        CapabilityKind, CapabilityNode, CapabilityNodeId, CapabilityProvenance,
        CapabilityProvenanceKind, CapabilitySupport, ClaimModality, GrammarNode, GrammarNodeId,
        GrammarPredicate, SourceSpan,
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
    fn membrane_requires_semantic_signature_for_missing_value() {
        let membrane = evaluate_claim_capability_membrane(
            &grammar(CoverageStatus::Complete, &[]),
            &capabilities(CoverageStatus::Complete, &[CapabilityKind::Operation]),
            ProfileKind::Common,
        )
        .expect("valid membrane");
        assert!(!membrane.opportunities.iter().any(|opportunity| {
            opportunity.kind == UnderclaimOpportunityKind::MissingSupportedValue
                && opportunity.dimension == ValueDimension::Capability
        }));
        assert_eq!(
            membrane.ceiling.limits[&ValueDimension::Capability],
            ClaimMode::Direct
        );
        assert_eq!(membrane.losses.missing_supported_values, 0);
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
        )
        .expect("valid membrane");
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
        )
        .expect("valid membrane");
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
        )
        .expect("valid membrane");
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

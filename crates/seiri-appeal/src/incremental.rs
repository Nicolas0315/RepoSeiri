use super::{
    capability_ceiling, claim_realizations, evaluate_claim_alignments,
    evaluate_claim_capability_membrane, evaluate_dimension, gate_rank, opportunity_key,
    profile_floor, CapabilityEvaluationIndex, DimensionEvaluationInputs,
};
use crate::input_projection::relation_rank;
use seiri_core::{
    AnswerState, AppealIrError, AppealLossVector, CapabilityKind, CapabilityProvenanceKind,
    CapabilitySemanticSignature, CapabilitySupport, ClaimAtomIR, ClaimCapabilityMembrane,
    ClaimMode, ClaimPolarity, CoverageIncompleteReason, CoverageStatus, DocumentLanguage,
    GrammarDiagnosticKind, GrammarPredicate, NarrativeRelation, OverclaimRiskKind, ProfileKind,
    ReadmeGrammarIR, ReadmeSectionLanguage, ReadmeTranslationAlignmentIR, RepositoryCapabilityIR,
    SourceSpan, SupportState, TranslationAlignmentState, TranslationDivergenceKind,
    UnderclaimOpportunityKind, UnknownReason, ValueDimension,
};
use seiri_digest::{Digest32, StableHasher};
use std::collections::{BTreeMap, BTreeSet};

const MAX_CHANGED_PATHS: usize = 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppealDependencyIndex {
    by_path: BTreeMap<String, BTreeSet<ValueDimension>>,
}

impl AppealDependencyIndex {
    #[must_use]
    pub fn build(grammar: &ReadmeGrammarIR, capabilities: &RepositoryCapabilityIR) -> Self {
        let mut by_path = BTreeMap::<String, BTreeSet<ValueDimension>>::new();
        if !grammar.path.is_empty() {
            by_path
                .entry(grammar.path.clone())
                .or_default()
                .extend(ValueDimension::ALL);
        }
        for node in &capabilities.nodes {
            let dimensions = capability_dimensions(node.kind);
            for provenance in &node.provenance {
                by_path
                    .entry(provenance.path.clone())
                    .or_default()
                    .extend(dimensions.iter().copied());
            }
        }
        Self { by_path }
    }

    #[must_use]
    pub fn dimensions_for_path(&self, path: &str) -> Vec<ValueDimension> {
        self.by_path
            .get(path)
            .map(|dimensions| dimensions.iter().copied().collect())
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncrementalUpdateMode {
    Reused,
    Sparse,
    ScalarRebuild,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncrementalMembraneState {
    profile: ProfileKind,
    membrane: ClaimCapabilityMembrane,
    semantic_digest: Digest32,
    dependency_index: AppealDependencyIndex,
    grammar_input_digest: Digest32,
    dimension_input_digests: BTreeMap<ValueDimension, Digest32>,
}

impl IncrementalMembraneState {
    pub fn from_scalar(
        grammar: &ReadmeGrammarIR,
        capabilities: &RepositoryCapabilityIR,
        profile: ProfileKind,
    ) -> Result<Self, AppealIrError> {
        let membrane = evaluate_claim_capability_membrane(grammar, capabilities, profile)?;
        Self::from_membrane(grammar, capabilities, profile, membrane)
    }

    #[must_use]
    pub const fn profile(&self) -> ProfileKind {
        self.profile
    }

    #[must_use]
    pub const fn membrane(&self) -> &ClaimCapabilityMembrane {
        &self.membrane
    }

    #[must_use]
    pub const fn semantic_digest(&self) -> Digest32 {
        self.semantic_digest
    }

    #[must_use]
    pub const fn dependency_index(&self) -> &AppealDependencyIndex {
        &self.dependency_index
    }

    fn from_membrane(
        grammar: &ReadmeGrammarIR,
        capabilities: &RepositoryCapabilityIR,
        profile: ProfileKind,
        membrane: ClaimCapabilityMembrane,
    ) -> Result<Self, AppealIrError> {
        let semantic_digest = membrane_semantic_digest(&membrane);
        Ok(Self {
            profile,
            membrane,
            semantic_digest,
            dependency_index: AppealDependencyIndex::build(grammar, capabilities),
            grammar_input_digest: grammar_input_digest(grammar),
            dimension_input_digests: dimension_input_digests(capabilities)?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncrementalMembraneUpdate {
    pub state: IncrementalMembraneState,
    pub frontier: Vec<ValueDimension>,
    pub mode: IncrementalUpdateMode,
    pub reused_dimensions: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IncrementalEquivalence {
    pub incremental_digest: Digest32,
    pub scalar_digest: Digest32,
    pub equivalent: bool,
}

/// Recomputes only membrane dimensions whose typed input or declared source dependency changed.
/// A README/profile/global-coverage change conservatively falls back to the scalar oracle.
pub fn update_incremental_membrane(
    previous: &IncrementalMembraneState,
    grammar: &ReadmeGrammarIR,
    capabilities: &RepositoryCapabilityIR,
    profile: ProfileKind,
    changed_paths: &[String],
) -> Result<IncrementalMembraneUpdate, AppealIrError> {
    grammar.validate()?;
    capabilities.validate()?;
    let grammar_digest = grammar_input_digest(grammar);
    let input_digests = dimension_input_digests(capabilities)?;
    let dependency_index = AppealDependencyIndex::build(grammar, capabilities);
    let mut frontier = BTreeSet::new();
    let scalar_required = previous.profile != profile
        || previous.grammar_input_digest != grammar_digest
        || changed_paths.len() > MAX_CHANGED_PATHS;

    if scalar_required {
        frontier.extend(ValueDimension::ALL);
    } else {
        for dimension in ValueDimension::ALL {
            if previous.dimension_input_digests.get(&dimension) != input_digests.get(&dimension) {
                frontier.insert(dimension);
            }
        }
        for path in changed_paths.iter().take(MAX_CHANGED_PATHS) {
            let mut matched = false;
            for dimension in previous
                .dependency_index
                .dimensions_for_path(path)
                .into_iter()
                .chain(dependency_index.dimensions_for_path(path))
            {
                matched = true;
                frontier.insert(dimension);
            }
            if !matched && is_program_path(path) {
                frontier.extend(ValueDimension::ALL);
            }
        }
    }

    let frontier = frontier.into_iter().collect::<Vec<_>>();
    let (membrane, mode) = if frontier.is_empty() {
        (previous.membrane.clone(), IncrementalUpdateMode::Reused)
    } else if scalar_required || frontier.len() == ValueDimension::ALL.len() {
        (
            evaluate_claim_capability_membrane(grammar, capabilities, profile)?,
            IncrementalUpdateMode::ScalarRebuild,
        )
    } else {
        (
            sparse_membrane_update(previous, grammar, capabilities, profile, &frontier)?,
            IncrementalUpdateMode::Sparse,
        )
    };
    let state = IncrementalMembraneState::from_membrane(grammar, capabilities, profile, membrane)?;
    Ok(IncrementalMembraneUpdate {
        state,
        reused_dimensions: ValueDimension::ALL.len().saturating_sub(frontier.len()),
        frontier,
        mode,
    })
}

/// Validation-only scalar comparison. Calling it performs a full scalar evaluation.
pub fn verify_incremental_against_scalar(
    update: &IncrementalMembraneUpdate,
    grammar: &ReadmeGrammarIR,
    capabilities: &RepositoryCapabilityIR,
    profile: ProfileKind,
) -> Result<IncrementalEquivalence, AppealIrError> {
    let scalar = evaluate_claim_capability_membrane(grammar, capabilities, profile)?;
    let scalar_digest = membrane_semantic_digest(&scalar);
    let incremental_digest = update.state.semantic_digest;
    Ok(IncrementalEquivalence {
        incremental_digest,
        scalar_digest,
        equivalent: incremental_digest == scalar_digest,
    })
}

fn sparse_membrane_update(
    previous: &IncrementalMembraneState,
    grammar: &ReadmeGrammarIR,
    capabilities: &RepositoryCapabilityIR,
    profile: ProfileKind,
    frontier: &[ValueDimension],
) -> Result<ClaimCapabilityMembrane, AppealIrError> {
    let frontier_set = frontier.iter().copied().collect::<BTreeSet<_>>();
    let capability_inputs = CapabilityEvaluationIndex::try_new(capabilities)?;
    let floor = profile_floor(profile);
    let ceiling = capability_ceiling(&capability_inputs)?;
    let current_alignments = evaluate_claim_alignments(grammar, &capability_inputs)?;
    let realizations = claim_realizations(grammar)?;
    let atom_dimensions = grammar
        .claim_atoms
        .atoms
        .iter()
        .map(|atom| (atom.id, atom.dimension))
        .collect::<BTreeMap<_, _>>();
    let mut membrane = previous.membrane.clone();
    membrane.floor = floor.clone();
    membrane.ceiling = ceiling.clone();
    membrane
        .relations
        .retain(|relation| !frontier_set.contains(&relation.dimension));
    membrane.alignments.retain(|alignment| {
        atom_dimensions
            .get(&alignment.claim_atom)
            .is_none_or(|dimension| !frontier_set.contains(dimension))
    });
    membrane
        .alignments
        .extend(current_alignments.into_iter().filter(|alignment| {
            atom_dimensions
                .get(&alignment.claim_atom)
                .is_some_and(|dimension| frontier_set.contains(dimension))
        }));
    membrane
        .alignments
        .sort_by_key(|alignment| alignment.claim_atom);
    membrane.opportunities.retain(|opportunity| {
        !frontier_set.contains(&opportunity.dimension)
            || !matches!(
                opportunity.kind,
                UnderclaimOpportunityKind::MissingSupportedValue
                    | UnderclaimOpportunityKind::ModeBelowFloor
            )
    });
    membrane
        .risks
        .retain(|risk| !frontier_set.contains(&risk.dimension));

    let dimension_inputs = DimensionEvaluationInputs {
        grammar,
        capabilities,
        capability_inputs: &capability_inputs,
        floor: &floor,
        ceiling: &ceiling,
        alignments: &membrane.alignments,
        realizations: &realizations,
    };
    for dimension in frontier.iter().copied() {
        let evaluation = evaluate_dimension(&dimension_inputs, dimension)?;
        membrane.relations.push(evaluation.relation);
        membrane.opportunities.extend(evaluation.opportunities);
        membrane.risks.extend(evaluation.risk);
    }

    membrane
        .relations
        .sort_by_key(|relation| relation.dimension);
    membrane.opportunities.sort_by_key(opportunity_key);
    membrane
        .opportunities
        .dedup_by(|left, right| opportunity_key(left) == opportunity_key(right));
    membrane
        .risks
        .sort_by_key(|risk| (risk.kind, risk.dimension));
    membrane
        .risks
        .dedup_by_key(|risk| (risk.kind, risk.dimension));
    membrane.losses = AppealLossVector {
        missing_supported_values: membrane
            .opportunities
            .iter()
            .filter(|opportunity| {
                opportunity.kind == UnderclaimOpportunityKind::MissingSupportedValue
            })
            .count(),
        below_floor_dimensions: membrane
            .opportunities
            .iter()
            .filter(|opportunity| opportunity.kind == UnderclaimOpportunityKind::ModeBelowFloor)
            .count(),
        above_ceiling_dimensions: membrane
            .risks
            .iter()
            .map(|risk| risk.dimension)
            .collect::<BTreeSet<_>>()
            .len(),
        disconnected_value_paths: previous.membrane.losses.disconnected_value_paths,
        translation_divergences: previous.membrane.losses.translation_divergences,
    };
    membrane.validate()?;
    Ok(membrane)
}

#[must_use]
pub fn membrane_semantic_digest(membrane: &ClaimCapabilityMembrane) -> Digest32 {
    let mut hash = StableHasher::new(b"seiri.claim-capability-membrane.normalized.v3", 8);
    hash.str(1, &membrane.semantic_revision);
    hash.digest(2, modes_digest(&membrane.floor.requirements, b"floor"));
    hash.digest(3, modes_digest(&membrane.ceiling.limits, b"ceiling"));
    hash.digest(4, relations_digest(membrane));
    hash.digest(5, opportunities_digest(membrane));
    hash.digest(6, risks_digest(membrane));
    let mut losses = StableHasher::new(b"seiri.appeal-loss-vector.v1", 5);
    losses
        .usize(1, membrane.losses.missing_supported_values)
        .usize(2, membrane.losses.below_floor_dimensions)
        .usize(3, membrane.losses.above_ceiling_dimensions)
        .usize(4, membrane.losses.disconnected_value_paths)
        .usize(5, membrane.losses.translation_divergences);
    hash.digest(7, losses.finish());
    hash.digest(8, alignments_digest(membrane));
    hash.finish()
}

fn modes_digest(modes: &BTreeMap<ValueDimension, ClaimMode>, suffix: &[u8]) -> Digest32 {
    let mut domain = b"seiri.claim-modes.v1/".to_vec();
    domain.extend_from_slice(suffix);
    let mut hash = StableHasher::new(&domain, 1);
    for (dimension, mode) in modes {
        let mut item = StableHasher::new(b"seiri.claim-mode.v1", 2);
        item.u8(1, dimension_rank(*dimension))
            .u8(2, claim_mode_rank(*mode));
        hash.digest(1, item.finish());
    }
    hash.finish()
}

fn relations_digest(membrane: &ClaimCapabilityMembrane) -> Digest32 {
    let mut relations = membrane.relations.iter().collect::<Vec<_>>();
    relations.sort_by_key(|relation| relation.dimension);
    let mut hash = StableHasher::new(b"seiri.support-relations.v1", 1);
    for relation in relations {
        let mut item = StableHasher::new(b"seiri.support-relation.v1", 5);
        item.u8(1, dimension_rank(relation.dimension));
        for id in &relation.grammar_nodes {
            item.u32(2, id.get());
        }
        for id in &relation.capability_nodes {
            item.u32(3, id.get());
        }
        item.usize(4, relation.evidence_count);
        hash_support_state(&mut item, 5, relation.state);
        hash.digest(1, item.finish());
    }
    hash.finish()
}

fn alignments_digest(membrane: &ClaimCapabilityMembrane) -> Digest32 {
    let mut alignments = membrane.alignments.iter().collect::<Vec<_>>();
    alignments.sort_by_key(|alignment| alignment.claim_atom);
    let mut hash = StableHasher::new(b"seiri.claim-capability-alignments.v2", 1);
    for alignment in alignments {
        let mut item = StableHasher::new(b"seiri.claim-capability-alignment.v2", 5);
        item.u32(1, alignment.claim_atom.get());
        for id in &alignment.capability_nodes {
            item.u32(2, id.get());
        }
        item.usize(3, alignment.evidence_count);
        hash_support_state(&mut item, 4, alignment.state);
        item.u8(5, claim_mode_rank(alignment.claim_ceiling));
        hash.digest(1, item.finish());
    }
    hash.finish()
}

fn opportunities_digest(membrane: &ClaimCapabilityMembrane) -> Digest32 {
    let mut opportunities = membrane.opportunities.iter().collect::<Vec<_>>();
    opportunities.sort_by_key(|opportunity| {
        (
            opportunity_kind_rank(opportunity.kind),
            dimension_rank(opportunity.dimension),
            gate_rank(opportunity.gate),
            opportunity.grammar_nodes.clone(),
            opportunity.capability_nodes.clone(),
        )
    });
    let mut hash = StableHasher::new(b"seiri.underclaim-opportunities.v1", 1);
    for opportunity in opportunities {
        let mut item = StableHasher::new(b"seiri.underclaim-opportunity.v1", 5);
        item.u8(1, opportunity_kind_rank(opportunity.kind))
            .u8(2, dimension_rank(opportunity.dimension))
            .u8(3, gate_rank(opportunity.gate));
        for id in &opportunity.grammar_nodes {
            item.u32(4, id.get());
        }
        for id in &opportunity.capability_nodes {
            item.u32(5, id.get());
        }
        hash.digest(1, item.finish());
    }
    hash.finish()
}

fn risks_digest(membrane: &ClaimCapabilityMembrane) -> Digest32 {
    let mut risks = membrane.risks.iter().collect::<Vec<_>>();
    risks.sort_by_key(|risk| {
        (
            risk_kind_rank(risk.kind),
            dimension_rank(risk.dimension),
            risk.grammar_nodes.clone(),
        )
    });
    let mut hash = StableHasher::new(b"seiri.overclaim-risks.v1", 1);
    for risk in risks {
        let mut item = StableHasher::new(b"seiri.overclaim-risk.v1", 3);
        item.u8(1, risk_kind_rank(risk.kind))
            .u8(2, dimension_rank(risk.dimension));
        for id in &risk.grammar_nodes {
            item.u32(3, id.get());
        }
        hash.digest(1, item.finish());
    }
    hash.finish()
}

fn grammar_input_digest(grammar: &ReadmeGrammarIR) -> Digest32 {
    let mut hash = StableHasher::new(b"seiri.readme-grammar.incremental-input.v3", 8);
    hash.str(1, &grammar.semantic_revision)
        .str(2, &grammar.path);
    hash_coverage(&mut hash, 3, grammar.coverage);
    for node in &grammar.nodes {
        let mut item = StableHasher::new(b"seiri.grammar-node.incremental.v1", 8);
        item.u32(1, node.id.get())
            .u8(2, predicate_rank(node.predicate));
        hash_answer_state(&mut item, 3, node.state);
        item.u8(4, language_rank(node.language))
            .u8(5, modality_rank(node.modality))
            .bool(6, node.negated);
        hash_span(&mut item, 7, node.span);
        item.u8(8, dimension_rank(node.predicate.dimension()));
        hash.digest(4, item.finish());
    }
    for edge in &grammar.edges {
        let mut item = StableHasher::new(b"seiri.grammar-edge.incremental.v1", 3);
        item.u32(1, edge.from.get())
            .u32(2, edge.to.get())
            .u8(3, narrative_rank(edge.relation));
        hash.digest(5, item.finish());
    }
    for diagnostic in &grammar.diagnostics {
        let mut item = StableHasher::new(b"seiri.grammar-diagnostic.incremental.v1", 2);
        item.u8(1, diagnostic_rank(diagnostic.kind));
        hash_span(&mut item, 2, diagnostic.span);
        hash.digest(6, item.finish());
    }
    hash.digest(7, claim_atom_ir_digest(&grammar.claim_atoms));
    hash.digest(
        8,
        translation_alignment_digest(&grammar.translation_alignment),
    );
    hash.finish()
}

fn claim_atom_ir_digest(claim_atoms: &ClaimAtomIR) -> Digest32 {
    let mut hash = StableHasher::new(b"seiri.readme-claim-atoms.incremental.v1", 2);
    hash.str(1, &claim_atoms.semantic_revision);
    for atom in &claim_atoms.atoms {
        let mut item = StableHasher::new(b"seiri.readme-claim-atom.incremental.v1", 12);
        item.u32(1, atom.id.get())
            .u32(2, atom.grammar_node.get())
            .u8(3, dimension_rank(atom.dimension));
        hash_optional_str(&mut item, 4, atom.subject.as_deref());
        hash_optional_str(&mut item, 5, atom.action.as_deref());
        hash_optional_str(&mut item, 6, atom.object.as_deref());
        for qualifier in &atom.qualifiers {
            item.str(7, qualifier);
        }
        hash_optional_str(&mut item, 8, atom.condition.as_deref());
        item.u8(9, polarity_rank(atom.polarity))
            .u8(10, modality_rank(atom.modality))
            .u8(11, language_rank(atom.language));
        hash_span(&mut item, 12, atom.span);
        hash.digest(2, item.finish());
    }
    hash.finish()
}

fn translation_alignment_digest(translation: &ReadmeTranslationAlignmentIR) -> Digest32 {
    let mut hash = StableHasher::new(b"seiri.readme-translation-alignment.incremental.v1", 7);
    hash.str(1, &translation.semantic_revision)
        .str(2, &translation.path)
        .digest(3, translation.source_digest.digest())
        .usize(4, translation.source_byte_len);
    for section in &translation.sections {
        let mut item = StableHasher::new(b"seiri.readme-translation-section.incremental.v1", 4);
        item.u32(1, section.id.get());
        hash_section_language(&mut item, 2, section.language);
        hash_span(&mut item, 3, Some(section.span));
        for claim in &section.claim_atoms {
            item.u32(4, claim.get());
        }
        hash.digest(5, item.finish());
    }
    for alignment in &translation.alignments {
        let mut item = StableHasher::new(b"seiri.readme-section-alignment.incremental.v1", 4);
        item.u32(1, alignment.source_section.get())
            .u32(2, alignment.counterpart_section.get());
        hash_translation_state(&mut item, 3, alignment.state);
        for claim in &alignment.claims {
            let mut claim_hash =
                StableHasher::new(b"seiri.readme-claim-alignment.incremental.v1", 4);
            claim_hash.u32(1, claim.source_claim.get());
            hash_optional_claim_id(&mut claim_hash, 2, claim.counterpart_claim);
            hash_translation_state(&mut claim_hash, 3, claim.state);
            for divergence in &claim.divergences {
                claim_hash.u8(4, translation_divergence_rank(*divergence));
            }
            item.digest(4, claim_hash.finish());
        }
        hash.digest(6, item.finish());
    }
    for reason in &translation.unknown_reasons {
        hash.u8(7, unknown_rank(*reason));
    }
    hash.finish()
}

fn hash_section_language(hash: &mut StableHasher, tag: u8, language: ReadmeSectionLanguage) {
    let mut value = StableHasher::new(b"seiri.readme-section-language.incremental.v1", 2);
    match language {
        ReadmeSectionLanguage::Japanese => {
            value.u8(1, 0);
        }
        ReadmeSectionLanguage::English => {
            value.u8(1, 1);
        }
        ReadmeSectionLanguage::Mixed => {
            value.u8(1, 2);
        }
        ReadmeSectionLanguage::Ambiguous(reason) => {
            value.u8(1, 3).u8(2, unknown_rank(reason));
        }
    }
    hash.digest(tag, value.finish());
}

fn hash_translation_state(hash: &mut StableHasher, tag: u8, state: TranslationAlignmentState) {
    let mut value = StableHasher::new(b"seiri.translation-alignment-state.incremental.v1", 2);
    match state {
        TranslationAlignmentState::Aligned => {
            value.u8(1, 0);
        }
        TranslationAlignmentState::Divergent => {
            value.u8(1, 1);
        }
        TranslationAlignmentState::Unknown(reason) => {
            value.u8(1, 2).u8(2, unknown_rank(reason));
        }
    }
    hash.digest(tag, value.finish());
}

fn hash_optional_claim_id(
    hash: &mut StableHasher,
    tag: u8,
    value: Option<seiri_core::ClaimAtomId>,
) {
    let mut optional = StableHasher::new(b"seiri.optional-claim-atom-id.incremental.v1", 2);
    optional.bool(1, value.is_some());
    if let Some(value) = value {
        optional.u32(2, value.get());
    }
    hash.digest(tag, optional.finish());
}

fn hash_optional_str(hash: &mut StableHasher, tag: u8, value: Option<&str>) {
    let mut optional = StableHasher::new(b"seiri.optional-string.incremental.v1", 2);
    optional.bool(1, value.is_some());
    if let Some(value) = value {
        optional.str(2, value);
    }
    hash.digest(tag, optional.finish());
}

fn dimension_input_digests(
    capabilities: &RepositoryCapabilityIR,
) -> Result<BTreeMap<ValueDimension, Digest32>, AppealIrError> {
    let capability_inputs = CapabilityEvaluationIndex::try_new(capabilities)?;
    Ok(ValueDimension::ALL
        .into_iter()
        .map(|dimension| {
            let mut hash =
                StableHasher::new(b"seiri.repository-capability.incremental-dimension.v3", 7);
            hash.str(1, &capabilities.semantic_revision)
                .u8(2, dimension_rank(dimension));
            hash_coverage(&mut hash, 3, capabilities.coverage);
            for reason in &capabilities.unknown_reasons {
                hash.u8(4, unknown_rank(*reason));
            }
            for node in capability_inputs.relevant_nodes(dimension) {
                hash.digest(5, capability_node_input_digest(node));
            }
            for signature in capability_inputs.signatures(dimension) {
                hash.digest(6, capability_signature_digest(signature));
            }
            for edge in capability_inputs.edges(dimension) {
                let mut item = StableHasher::new(b"seiri.capability-edge.membrane.v1", 3);
                item.u32(1, edge.from.get())
                    .u32(2, edge.to.get())
                    .u8(3, relation_rank(edge.relation));
                hash.digest(7, item.finish());
            }
            (dimension, hash.finish())
        })
        .collect())
}

fn capability_node_input_digest(node: &seiri_core::CapabilityNode) -> Digest32 {
    let mut hash = StableHasher::new(b"seiri.capability-node.membrane.v2", 5);
    hash.u32(1, node.id.get())
        .u8(2, capability_kind_rank(node.kind));
    hash_capability_support(&mut hash, 3, node.support);
    hash.str(4, &node.symbol);
    for provenance in &node.provenance {
        let mut item = StableHasher::new(b"seiri.capability-provenance.membrane.v1", 3);
        item.str(1, &provenance.path)
            .u8(2, capability_provenance_rank(provenance.kind));
        hash_span(&mut item, 3, provenance.span);
        hash.digest(5, item.finish());
    }
    hash.finish()
}

fn capability_signature_digest(signature: &CapabilitySemanticSignature) -> Digest32 {
    let mut hash = StableHasher::new(b"seiri.capability-semantic-signature.incremental.v1", 11);
    hash.u32(1, signature.capability_node.get())
        .u8(2, dimension_rank(signature.dimension));
    hash_optional_str(&mut hash, 3, signature.subject.as_deref());
    hash_optional_str(&mut hash, 4, signature.action.as_deref());
    hash_optional_str(&mut hash, 5, signature.object.as_deref());
    for qualifier in &signature.qualifiers {
        hash.str(6, qualifier);
    }
    hash.u8(7, polarity_rank(signature.polarity));
    for id in &signature.input_nodes {
        hash.u32(8, id.get());
    }
    for id in &signature.output_nodes {
        hash.u32(9, id.get());
    }
    for id in &signature.condition_nodes {
        hash.u32(10, id.get());
    }
    hash_capability_support(&mut hash, 11, signature.semantic_support);
    hash.finish()
}

fn capability_dimensions(kind: CapabilityKind) -> Vec<ValueDimension> {
    vec![kind.dimension()]
}

fn is_program_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower == "cargo.toml" || lower.ends_with("/cargo.toml") || lower.ends_with(".rs")
}

fn hash_span(hash: &mut StableHasher, tag: u8, span: Option<SourceSpan>) {
    let mut value = StableHasher::new(b"seiri.optional-source-span.v1", 5);
    value.bool(1, span.is_some());
    if let Some(span) = span {
        value
            .usize(2, span.line)
            .usize(3, span.column)
            .usize(4, span.byte_start)
            .usize(5, span.byte_end);
    }
    hash.digest(tag, value.finish());
}

fn hash_coverage(hash: &mut StableHasher, tag: u8, coverage: CoverageStatus) {
    let (state, reason) = match coverage {
        CoverageStatus::Complete => (0, None),
        CoverageStatus::Partial(reason) => (1, Some(coverage_reason_rank(reason))),
        CoverageStatus::NotRequested => (2, None),
    };
    let mut value = StableHasher::new(b"seiri.coverage-status.incremental.v1", 2);
    value.u8(1, state);
    if let Some(reason) = reason {
        value.u8(2, reason);
    }
    hash.digest(tag, value.finish());
}

fn hash_answer_state(hash: &mut StableHasher, tag: u8, state: AnswerState) {
    let (state, reason) = match state {
        AnswerState::Explicit => (0, None),
        AnswerState::Inferred => (1, None),
        AnswerState::Missing => (2, None),
        AnswerState::Contested => (3, None),
        AnswerState::Unknown(reason) => (4, Some(unknown_rank(reason))),
    };
    let mut value = StableHasher::new(b"seiri.answer-state.incremental.v1", 2);
    value.u8(1, state);
    if let Some(reason) = reason {
        value.u8(2, reason);
    }
    hash.digest(tag, value.finish());
}

fn hash_support_state(hash: &mut StableHasher, tag: u8, state: SupportState) {
    let (state, reason) = match state {
        SupportState::Supported => (0, None),
        SupportState::InsufficientEvidence => (1, None),
        SupportState::Contradicted => (2, None),
        SupportState::Unknown(reason) => (3, Some(unknown_rank(reason))),
    };
    let mut value = StableHasher::new(b"seiri.support-state.v1", 2);
    value.u8(1, state);
    if let Some(reason) = reason {
        value.u8(2, reason);
    }
    hash.digest(tag, value.finish());
}

fn hash_capability_support(hash: &mut StableHasher, tag: u8, support: CapabilitySupport) {
    let (state, reason) = match support {
        CapabilitySupport::Observed => (0, None),
        CapabilitySupport::Inferred => (1, None),
        CapabilitySupport::Unknown(reason) => (2, Some(unknown_rank(reason))),
    };
    let mut value = StableHasher::new(b"seiri.capability-support.incremental.v1", 2);
    value.u8(1, state);
    if let Some(reason) = reason {
        value.u8(2, reason);
    }
    hash.digest(tag, value.finish());
}

const fn dimension_rank(value: ValueDimension) -> u8 {
    match value {
        ValueDimension::Identity => 0,
        ValueDimension::Audience => 1,
        ValueDimension::Problem => 2,
        ValueDimension::Capability => 3,
        ValueDimension::Outcome => 4,
        ValueDimension::FirstAction => 5,
        ValueDimension::FirstResult => 6,
        ValueDimension::Evidence => 7,
        ValueDimension::Constraint => 8,
        ValueDimension::Differentiation => 9,
    }
}

const fn claim_mode_rank(value: ClaimMode) -> u8 {
    match value {
        ClaimMode::Omitted => 0,
        ClaimMode::Generic => 1,
        ClaimMode::Qualified => 2,
        ClaimMode::Direct => 3,
    }
}

const fn polarity_rank(value: ClaimPolarity) -> u8 {
    match value {
        ClaimPolarity::Positive => 0,
        ClaimPolarity::Negative => 1,
    }
}

const fn opportunity_kind_rank(value: UnderclaimOpportunityKind) -> u8 {
    match value {
        UnderclaimOpportunityKind::MissingSupportedValue => 0,
        UnderclaimOpportunityKind::ModeBelowFloor => 1,
        UnderclaimOpportunityKind::GenericVerbCollapse => 2,
        UnderclaimOpportunityKind::JargonOcclusion => 3,
        UnderclaimOpportunityKind::CapabilityOutcomeDisconnect => 4,
        UnderclaimOpportunityKind::FirstValueDisconnect => 5,
        UnderclaimOpportunityKind::EvidenceDisconnect => 6,
        UnderclaimOpportunityKind::QualifierDominance => 7,
        UnderclaimOpportunityKind::BuriedPrimaryValue => 8,
        UnderclaimOpportunityKind::FragmentedValue => 9,
        UnderclaimOpportunityKind::TranslationDivergence => 10,
        UnderclaimOpportunityKind::UnexpressedConstraintBackedAdvantage => 11,
    }
}

const fn risk_kind_rank(value: OverclaimRiskKind) -> u8 {
    match value {
        OverclaimRiskKind::UnsupportedCapability => 0,
        OverclaimRiskKind::UnsupportedOutcome => 1,
        OverclaimRiskKind::RuntimeSuccessNotObserved => 2,
        OverclaimRiskKind::PerformanceNotMeasured => 3,
        OverclaimRiskKind::TrustNotEstablished => 4,
        OverclaimRiskKind::SecurityNotEstablished => 5,
        OverclaimRiskKind::AudienceNotObserved => 6,
        OverclaimRiskKind::IdentityNotObserved => 7,
        OverclaimRiskKind::DifferentiationNotEstablished => 8,
        OverclaimRiskKind::ProblemNotObserved => 9,
        OverclaimRiskKind::FirstActionNotObserved => 10,
        OverclaimRiskKind::EvidenceNotEstablished => 11,
        OverclaimRiskKind::ConstraintNotEstablished => 12,
    }
}

const fn unknown_rank(value: UnknownReason) -> u8 {
    match value {
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

const fn translation_divergence_rank(value: TranslationDivergenceKind) -> u8 {
    match value {
        TranslationDivergenceKind::Polarity => 0,
        TranslationDivergenceKind::Qualifier => 1,
        TranslationDivergenceKind::Condition => 2,
        TranslationDivergenceKind::MissingCounterpart => 3,
    }
}

const fn coverage_reason_rank(value: CoverageIncompleteReason) -> u8 {
    match value {
        CoverageIncompleteReason::LimitExceeded => 0,
        CoverageIncompleteReason::InvalidUtf8 => 1,
        CoverageIncompleteReason::ParseFailed => 2,
        CoverageIncompleteReason::UnsupportedSyntax => 3,
        CoverageIncompleteReason::PermissionDenied => 4,
        CoverageIncompleteReason::RateLimited => 5,
        CoverageIncompleteReason::Unavailable => 6,
    }
}

const fn predicate_rank(value: GrammarPredicate) -> u8 {
    match value {
        GrammarPredicate::DefinesIdentity => 0,
        GrammarPredicate::TargetsAudience => 1,
        GrammarPredicate::StatesProblem => 2,
        GrammarPredicate::PerformsOperation => 3,
        GrammarPredicate::ProducesOutcome => 4,
        GrammarPredicate::DescribesFirstAction => 5,
        GrammarPredicate::DescribesFirstResult => 6,
        GrammarPredicate::ProvidesEvidence => 7,
        GrammarPredicate::StatesConstraint => 8,
        GrammarPredicate::StatesDifferentiation => 9,
        GrammarPredicate::RequestsAction => 10,
    }
}

const fn capability_kind_rank(value: CapabilityKind) -> u8 {
    match value {
        CapabilityKind::Manifest => 0,
        CapabilityKind::ProgramLanguage => 1,
        CapabilityKind::Entrypoint => 2,
        CapabilityKind::PublicApi => 3,
        CapabilityKind::Operation => 4,
        CapabilityKind::Input => 5,
        CapabilityKind::Output => 6,
        CapabilityKind::FirstResult => 7,
        CapabilityKind::Example => 8,
        CapabilityKind::Test => 9,
        CapabilityKind::Feature => 10,
        CapabilityKind::Constraint => 11,
        CapabilityKind::ComparisonReceipt => 12,
    }
}

const fn capability_provenance_rank(value: CapabilityProvenanceKind) -> u8 {
    match value {
        CapabilityProvenanceKind::Manifest => 0,
        CapabilityProvenanceKind::SourceSyntax => 1,
        CapabilityProvenanceKind::ExamplePath => 2,
        CapabilityProvenanceKind::TestPath => 3,
        CapabilityProvenanceKind::Documentation => 4,
    }
}

const fn language_rank(value: DocumentLanguage) -> u8 {
    match value {
        DocumentLanguage::Japanese => 0,
        DocumentLanguage::English => 1,
    }
}

const fn modality_rank(value: seiri_core::ClaimModality) -> u8 {
    match value {
        seiri_core::ClaimModality::Asserted => 0,
        seiri_core::ClaimModality::Qualified => 1,
        seiri_core::ClaimModality::Possible => 2,
        seiri_core::ClaimModality::Required => 3,
        seiri_core::ClaimModality::Prohibited => 4,
    }
}

const fn narrative_rank(value: NarrativeRelation) -> u8 {
    match value {
        NarrativeRelation::Precedes => 0,
        NarrativeRelation::Supports => 1,
        NarrativeRelation::Explains => 2,
        NarrativeRelation::Qualifies => 3,
        NarrativeRelation::Translates => 4,
    }
}

const fn diagnostic_rank(value: GrammarDiagnosticKind) -> u8 {
    match value {
        GrammarDiagnosticKind::SourceLimitExceeded => 0,
        GrammarDiagnosticKind::NodeLimitExceeded => 1,
        GrammarDiagnosticKind::AmbiguousLanguage => 2,
        GrammarDiagnosticKind::UnsupportedConstruct => 3,
        GrammarDiagnosticKind::ConflictingClause => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use seiri_core::{
        CapabilityEdge, CapabilityNode, CapabilityNodeId, CapabilityProvenance,
        CapabilityProvenanceKind, CapabilityRelation, CapabilitySemanticSignature, ClaimAtom,
        ClaimAtomId, ClaimModality, GrammarNode, GrammarNodeId, SourceSpan,
        README_CLAIM_ATOM_REVISION,
    };
    use std::num::NonZeroU32;

    fn empty_grammar() -> ReadmeGrammarIR {
        ReadmeGrammarIR::try_new(
            "README.md",
            CoverageStatus::Complete,
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .expect("grammar")
    }

    fn capability(kind: CapabilityKind, path: &str) -> RepositoryCapabilityIR {
        RepositoryCapabilityIR::try_new(
            CoverageStatus::Complete,
            vec![CapabilityNode {
                id: CapabilityNodeId::new(NonZeroU32::MIN),
                kind,
                support: CapabilitySupport::Observed,
                symbol: "symbol".to_string(),
                provenance: vec![CapabilityProvenance {
                    path: path.to_string(),
                    kind: CapabilityProvenanceKind::SourceSyntax,
                    span: Some(SourceSpan::new(1, 1, 0, 6)),
                }],
            }],
            Vec::new(),
            Vec::new(),
        )
        .expect("capability")
    }

    fn grammar_with_atom(action: &str) -> ReadmeGrammarIR {
        let node_id = GrammarNodeId::new(NonZeroU32::MIN);
        let span = SourceSpan::new(1, 1, 0, 20);
        ReadmeGrammarIR::try_new_with_claim_atoms(
            "README.md",
            CoverageStatus::Complete,
            vec![GrammarNode {
                id: node_id,
                predicate: GrammarPredicate::PerformsOperation,
                state: AnswerState::Explicit,
                language: DocumentLanguage::English,
                modality: ClaimModality::Asserted,
                negated: false,
                span: Some(span),
            }],
            Vec::new(),
            Vec::new(),
            ClaimAtomIR {
                semantic_revision: README_CLAIM_ATOM_REVISION.to_string(),
                atoms: vec![ClaimAtom {
                    id: ClaimAtomId::new(NonZeroU32::MIN),
                    grammar_node: node_id,
                    dimension: ValueDimension::Capability,
                    subject: Some("reposeiri".to_string()),
                    action: Some(action.to_string()),
                    object: Some("repositories".to_string()),
                    qualifiers: Vec::new(),
                    condition: None,
                    polarity: ClaimPolarity::Positive,
                    modality: ClaimModality::Asserted,
                    language: DocumentLanguage::English,
                    span: Some(span),
                }],
            },
        )
        .expect("grammar with atom")
    }

    fn capability_with_signature(action: &str) -> RepositoryCapabilityIR {
        let owner = CapabilityNodeId::new(NonZeroU32::MIN);
        RepositoryCapabilityIR::try_new_with_semantic_signatures_and_diagnostics(
            CoverageStatus::Complete,
            vec![CapabilityNode {
                id: owner,
                kind: CapabilityKind::Operation,
                support: CapabilitySupport::Observed,
                symbol: "operation".to_string(),
                provenance: vec![CapabilityProvenance {
                    path: "src/lib.rs".to_string(),
                    kind: CapabilityProvenanceKind::SourceSyntax,
                    span: Some(SourceSpan::new(1, 1, 0, 9)),
                }],
            }],
            Vec::new(),
            vec![CapabilitySemanticSignature {
                capability_node: owner,
                dimension: ValueDimension::Capability,
                subject: None,
                action: Some(action.to_string()),
                object: Some("repository".to_string()),
                qualifiers: Vec::new(),
                polarity: ClaimPolarity::Positive,
                input_nodes: Vec::new(),
                output_nodes: Vec::new(),
                condition_nodes: Vec::new(),
                semantic_support: CapabilitySupport::Inferred,
            }],
            Vec::new(),
            Vec::new(),
        )
        .expect("capability with signature")
    }

    #[test]
    fn unchanged_inputs_reuse_the_entire_membrane() {
        let grammar = empty_grammar();
        let capabilities = RepositoryCapabilityIR::try_new(
            CoverageStatus::Complete,
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .expect("capabilities");
        let state =
            IncrementalMembraneState::from_scalar(&grammar, &capabilities, ProfileKind::Common)
                .expect("valid scalar state");
        let update =
            update_incremental_membrane(&state, &grammar, &capabilities, ProfileKind::Common, &[])
                .expect("valid incremental update");
        assert_eq!(update.mode, IncrementalUpdateMode::Reused);
        assert!(update.frontier.is_empty());
        assert_eq!(update.reused_dimensions, ValueDimension::ALL.len());
        assert_eq!(update.state.semantic_digest(), state.semantic_digest());
    }

    #[test]
    fn claim_atom_semantic_change_forces_scalar_rebuild() {
        let before = grammar_with_atom("audit");
        let after = grammar_with_atom("inspect");
        assert_ne!(grammar_input_digest(&before), grammar_input_digest(&after));
        let capabilities = RepositoryCapabilityIR::try_new(
            CoverageStatus::Complete,
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .expect("capabilities");
        let state =
            IncrementalMembraneState::from_scalar(&before, &capabilities, ProfileKind::Common)
                .expect("valid scalar state");
        let update =
            update_incremental_membrane(&state, &after, &capabilities, ProfileKind::Common, &[])
                .expect("valid incremental update");

        assert_eq!(update.mode, IncrementalUpdateMode::ScalarRebuild);
        assert_eq!(update.frontier, ValueDimension::ALL);
        assert!(
            verify_incremental_against_scalar(&update, &after, &capabilities, ProfileKind::Common,)
                .expect("valid scalar verification")
                .equivalent
        );
    }

    #[test]
    fn signature_only_change_updates_exact_dimension_and_matches_scalar() {
        let grammar = grammar_with_atom("audit");
        let before = capability_with_signature("audit");
        let after = capability_with_signature("inspect");
        let before_digests = dimension_input_digests(&before).expect("valid before inputs");
        let after_digests = dimension_input_digests(&after).expect("valid after inputs");
        assert_ne!(
            before_digests[&ValueDimension::Capability],
            after_digests[&ValueDimension::Capability]
        );
        let state = IncrementalMembraneState::from_scalar(&grammar, &before, ProfileKind::Common)
            .expect("valid scalar state");
        let update =
            update_incremental_membrane(&state, &grammar, &after, ProfileKind::Common, &[])
                .expect("valid incremental update");

        assert_eq!(update.mode, IncrementalUpdateMode::Sparse);
        assert_eq!(update.frontier, [ValueDimension::Capability]);
        assert!(
            verify_incremental_against_scalar(&update, &grammar, &after, ProfileKind::Common,)
                .expect("valid scalar verification")
                .equivalent
        );
        assert_eq!(
            update
                .state
                .membrane()
                .relations
                .iter()
                .find(|relation| relation.dimension == ValueDimension::Capability)
                .expect("capability relation")
                .state,
            SupportState::InsufficientEvidence
        );
    }

    #[test]
    fn one_program_dimension_uses_sparse_update_and_matches_scalar() {
        let grammar = empty_grammar();
        let before = RepositoryCapabilityIR::try_new(
            CoverageStatus::Complete,
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .expect("before");
        let after = capability(CapabilityKind::Test, "tests/evidence.rs");
        let state = IncrementalMembraneState::from_scalar(&grammar, &before, ProfileKind::Common)
            .expect("valid scalar state");
        let update = update_incremental_membrane(
            &state,
            &grammar,
            &after,
            ProfileKind::Common,
            &["tests/evidence.rs".to_string()],
        )
        .expect("valid incremental update");
        let equivalence =
            verify_incremental_against_scalar(&update, &grammar, &after, ProfileKind::Common)
                .expect("valid scalar verification");

        assert_eq!(update.mode, IncrementalUpdateMode::Sparse);
        assert_eq!(update.frontier, vec![ValueDimension::Evidence]);
        assert_eq!(update.reused_dimensions, 9);
        assert!(equivalence.equivalent);
        assert_eq!(equivalence.incremental_digest, equivalence.scalar_digest);
    }

    #[test]
    fn undeclared_semantic_change_is_detected_by_typed_dimension_digest() {
        let grammar = empty_grammar();
        let before = RepositoryCapabilityIR::try_new(
            CoverageStatus::Complete,
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .expect("before");
        let after = capability(CapabilityKind::Constraint, "src/config.rs");
        let state = IncrementalMembraneState::from_scalar(&grammar, &before, ProfileKind::Common)
            .expect("valid scalar state");
        let update =
            update_incremental_membrane(&state, &grammar, &after, ProfileKind::Common, &[])
                .expect("valid incremental update");

        assert_eq!(update.frontier, vec![ValueDimension::Constraint]);
        assert!(
            verify_incremental_against_scalar(&update, &grammar, &after, ProfileKind::Common)
                .expect("valid scalar verification")
                .equivalent
        );
    }

    #[test]
    fn readme_change_conservatively_uses_scalar_oracle() {
        let before = empty_grammar();
        let after = ReadmeGrammarIR::try_new(
            "README.md",
            CoverageStatus::Complete,
            vec![GrammarNode {
                id: GrammarNodeId::new(NonZeroU32::MIN),
                predicate: GrammarPredicate::DefinesIdentity,
                state: AnswerState::Explicit,
                language: DocumentLanguage::English,
                modality: seiri_core::ClaimModality::Asserted,
                negated: false,
                span: Some(SourceSpan::new(1, 1, 0, 4)),
            }],
            Vec::new(),
            Vec::new(),
        )
        .expect("after grammar");
        let capabilities = capability(CapabilityKind::Manifest, "Cargo.toml");
        let state =
            IncrementalMembraneState::from_scalar(&before, &capabilities, ProfileKind::Common)
                .expect("valid scalar state");
        let update = update_incremental_membrane(
            &state,
            &after,
            &capabilities,
            ProfileKind::Common,
            &["README.md".to_string()],
        )
        .expect("valid incremental update");

        assert_eq!(update.mode, IncrementalUpdateMode::ScalarRebuild);
        assert_eq!(update.frontier, ValueDimension::ALL);
        assert!(
            verify_incremental_against_scalar(&update, &after, &capabilities, ProfileKind::Common)
                .expect("valid scalar verification")
                .equivalent
        );
    }

    #[test]
    fn every_capability_kind_matches_the_scalar_oracle() {
        let kinds = [
            CapabilityKind::Manifest,
            CapabilityKind::ProgramLanguage,
            CapabilityKind::Entrypoint,
            CapabilityKind::PublicApi,
            CapabilityKind::Operation,
            CapabilityKind::Input,
            CapabilityKind::Output,
            CapabilityKind::FirstResult,
            CapabilityKind::Example,
            CapabilityKind::Test,
            CapabilityKind::Feature,
            CapabilityKind::Constraint,
            CapabilityKind::ComparisonReceipt,
        ];
        let grammar = empty_grammar();
        let before = RepositoryCapabilityIR::try_new(
            CoverageStatus::Complete,
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .expect("before");
        for kind in kinds {
            let path = format!("src/{kind:?}.rs").to_ascii_lowercase();
            let after = capability(kind, &path);
            let state =
                IncrementalMembraneState::from_scalar(&grammar, &before, ProfileKind::Common)
                    .expect("valid scalar state");
            let update =
                update_incremental_membrane(&state, &grammar, &after, ProfileKind::Common, &[path])
                    .expect("valid incremental update");
            assert!(
                verify_incremental_against_scalar(&update, &grammar, &after, ProfileKind::Common)
                    .expect("valid scalar verification")
                    .equivalent,
                "{kind:?}"
            );
        }
    }

    #[test]
    fn global_unknown_coverage_rebuilds_and_preserves_unknown_relations() {
        let grammar = empty_grammar();
        let before = RepositoryCapabilityIR::try_new(
            CoverageStatus::Complete,
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .expect("before");
        let after = RepositoryCapabilityIR::try_new(
            CoverageStatus::Partial(CoverageIncompleteReason::UnsupportedSyntax),
            Vec::new(),
            Vec::new(),
            vec![UnknownReason::UnsupportedSyntax],
        )
        .expect("after");
        let state = IncrementalMembraneState::from_scalar(&grammar, &before, ProfileKind::Common)
            .expect("valid scalar state");
        let update = update_incremental_membrane(
            &state,
            &grammar,
            &after,
            ProfileKind::Common,
            &["src/generated.rs".to_string()],
        )
        .expect("valid incremental update");

        assert_eq!(update.mode, IncrementalUpdateMode::ScalarRebuild);
        assert!(update
            .state
            .membrane()
            .relations
            .iter()
            .all(|relation| matches!(
                relation.state,
                SupportState::Unknown(UnknownReason::UnsupportedSyntax)
            )));
        assert!(
            verify_incremental_against_scalar(&update, &grammar, &after, ProfileKind::Common)
                .expect("valid scalar verification")
                .equivalent
        );
    }

    #[test]
    fn normalized_digest_ignores_collection_order_only() {
        let grammar = empty_grammar();
        let capabilities = capability(CapabilityKind::Example, "examples/demo.rs");
        let membrane =
            evaluate_claim_capability_membrane(&grammar, &capabilities, ProfileKind::Common)
                .expect("valid membrane");
        let mut reordered = membrane.clone();
        reordered.relations.reverse();
        reordered.opportunities.reverse();
        reordered.risks.reverse();

        assert_eq!(
            membrane_semantic_digest(&membrane),
            membrane_semantic_digest(&reordered)
        );
        reordered.losses.missing_supported_values =
            reordered.losses.missing_supported_values.saturating_add(1);
        assert_ne!(
            membrane_semantic_digest(&membrane),
            membrane_semantic_digest(&reordered)
        );
    }

    #[test]
    fn demonstrated_by_edge_change_invalidates_evidence_without_path_hint() {
        let grammar = empty_grammar();
        let example = CapabilityNodeId::new(NonZeroU32::MIN);
        let operation = CapabilityNodeId::new(NonZeroU32::new(2).expect("non-zero"));
        let nodes = vec![
            CapabilityNode {
                id: example,
                kind: CapabilityKind::Example,
                support: CapabilitySupport::Observed,
                symbol: "demo".to_string(),
                provenance: vec![CapabilityProvenance {
                    path: "examples/demo.rs".to_string(),
                    kind: CapabilityProvenanceKind::ExamplePath,
                    span: None,
                }],
            },
            CapabilityNode {
                id: operation,
                kind: CapabilityKind::Operation,
                support: CapabilitySupport::Observed,
                symbol: "audit".to_string(),
                provenance: vec![CapabilityProvenance {
                    path: "src/lib.rs".to_string(),
                    kind: CapabilityProvenanceKind::SourceSyntax,
                    span: None,
                }],
            },
        ];
        let before = RepositoryCapabilityIR::try_new(
            CoverageStatus::Complete,
            nodes.clone(),
            Vec::new(),
            Vec::new(),
        )
        .expect("before capabilities");
        let after = RepositoryCapabilityIR::try_new(
            CoverageStatus::Complete,
            nodes,
            vec![CapabilityEdge {
                from: operation,
                to: example,
                relation: CapabilityRelation::DemonstratedBy,
            }],
            Vec::new(),
        )
        .expect("after capabilities");
        let state = IncrementalMembraneState::from_scalar(&grammar, &before, ProfileKind::Common)
            .expect("valid scalar state");
        let update =
            update_incremental_membrane(&state, &grammar, &after, ProfileKind::Common, &[])
                .expect("valid incremental update");

        assert_ne!(update.mode, IncrementalUpdateMode::Reused);
        assert!(update.frontier.contains(&ValueDimension::Evidence));
        assert!(
            verify_incremental_against_scalar(&update, &grammar, &after, ProfileKind::Common)
                .expect("valid scalar verification")
                .equivalent
        );
    }

    #[test]
    fn referenced_condition_symbol_change_invalidates_claim_dimension() {
        let mut grammar = grammar_with_atom("audit");
        grammar.claim_atoms.atoms[0].condition = Some("offline".to_string());
        grammar.validate().expect("conditioned grammar");
        let owner = CapabilityNodeId::new(NonZeroU32::MIN);
        let condition = CapabilityNodeId::new(NonZeroU32::new(2).expect("non-zero"));
        let capabilities = |symbol: &str| {
            RepositoryCapabilityIR::try_new_with_semantic_signatures_and_diagnostics(
                CoverageStatus::Complete,
                vec![
                    CapabilityNode {
                        id: owner,
                        kind: CapabilityKind::Operation,
                        support: CapabilitySupport::Observed,
                        symbol: "audit_repository".to_string(),
                        provenance: vec![CapabilityProvenance {
                            path: "src/lib.rs".to_string(),
                            kind: CapabilityProvenanceKind::SourceSyntax,
                            span: None,
                        }],
                    },
                    CapabilityNode {
                        id: condition,
                        kind: CapabilityKind::Feature,
                        support: CapabilitySupport::Observed,
                        symbol: symbol.to_string(),
                        provenance: vec![CapabilityProvenance {
                            path: "Cargo.toml".to_string(),
                            kind: CapabilityProvenanceKind::Manifest,
                            span: None,
                        }],
                    },
                ],
                vec![CapabilityEdge {
                    from: owner,
                    to: condition,
                    relation: CapabilityRelation::ConditionedBy,
                }],
                vec![CapabilitySemanticSignature {
                    capability_node: owner,
                    dimension: ValueDimension::Capability,
                    subject: Some("reposeiri".to_string()),
                    action: Some("audit".to_string()),
                    object: Some("repository".to_string()),
                    qualifiers: Vec::new(),
                    polarity: ClaimPolarity::Positive,
                    input_nodes: Vec::new(),
                    output_nodes: Vec::new(),
                    condition_nodes: vec![condition],
                    semantic_support: CapabilitySupport::Inferred,
                }],
                Vec::new(),
                Vec::new(),
            )
            .expect("conditioned capabilities")
        };
        let before = capabilities("online");
        let after = capabilities("offline");
        let state = IncrementalMembraneState::from_scalar(&grammar, &before, ProfileKind::Common)
            .expect("valid scalar state");
        let update =
            update_incremental_membrane(&state, &grammar, &after, ProfileKind::Common, &[])
                .expect("valid incremental update");

        assert_eq!(update.frontier, vec![ValueDimension::Capability]);
        assert!(
            verify_incremental_against_scalar(&update, &grammar, &after, ProfileKind::Common)
                .expect("valid scalar verification")
                .equivalent
        );
    }
}

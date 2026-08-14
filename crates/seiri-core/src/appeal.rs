use crate::{
    CoverageStatus, DocumentLanguage, GateKind, PatchBaseDigest, SourceSpan, UnknownReason,
};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{Display, Formatter};
use std::num::NonZeroU32;

pub const README_GRAMMAR_REVISION: &str = "seiri.readme-grammar.v2";
pub const README_CLAIM_ATOM_REVISION: &str = "seiri.readme-claim-atom.v2";
pub const README_TRANSLATION_ALIGNMENT_REVISION: &str = "seiri.readme-translation-alignment.v2";
pub const CLAIM_DRAFT_REVISION: &str = "seiri.claim-draft.v1";
pub const CLAIM_DRAFT_REAUDIT_REVISION: &str = "seiri.claim-draft-reaudit.v1";
pub const REPOSITORY_CAPABILITY_REVISION: &str = "seiri.repository-capability.v2";
pub const CLAIM_CAPABILITY_MEMBRANE_REVISION: &str = "seiri.claim-capability-membrane.v3";
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
pub struct ClaimAtomId(NonZeroU32);

impl ClaimAtomId {
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
pub struct ClaimDraftId(NonZeroU32);

impl ClaimDraftId {
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
pub struct ReadmeSectionId(NonZeroU32);

impl ReadmeSectionId {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimPolarity {
    Positive,
    Negative,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimAtom {
    pub id: ClaimAtomId,
    pub grammar_node: GrammarNodeId,
    pub dimension: ValueDimension,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub qualifiers: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
    pub polarity: ClaimPolarity,
    pub modality: ClaimModality,
    pub language: DocumentLanguage,
    pub span: Option<SourceSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimAtomIR {
    pub semantic_revision: String,
    pub atoms: Vec<ClaimAtom>,
}

impl Default for ClaimAtomIR {
    fn default() -> Self {
        Self {
            semantic_revision: README_CLAIM_ATOM_REVISION.to_string(),
            atoms: Vec::new(),
        }
    }
}

/// Section-local language state. This intentionally does not widen
/// [`DocumentLanguage`], which remains the language of a confidently classified
/// grammar node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "reason", rename_all = "snake_case")]
pub enum ReadmeSectionLanguage {
    Japanese,
    English,
    Mixed,
    Ambiguous(UnknownReason),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadmeSection {
    pub id: ReadmeSectionId,
    pub language: ReadmeSectionLanguage,
    pub span: SourceSpan,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub claim_atoms: Vec<ClaimAtomId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TranslationDivergenceKind {
    Polarity,
    Qualifier,
    Condition,
    MissingCounterpart,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "reason", rename_all = "snake_case")]
pub enum TranslationAlignmentState {
    Aligned,
    Divergent,
    Unknown(UnknownReason),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranslationClaimAlignment {
    pub source_claim: ClaimAtomId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub counterpart_claim: Option<ClaimAtomId>,
    pub state: TranslationAlignmentState,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub divergences: Vec<TranslationDivergenceKind>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadmeSectionAlignment {
    pub source_section: ReadmeSectionId,
    pub counterpart_section: ReadmeSectionId,
    pub state: TranslationAlignmentState,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub claims: Vec<TranslationClaimAlignment>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadmeTranslationAlignmentIR {
    pub semantic_revision: String,
    pub path: String,
    pub source_digest: PatchBaseDigest,
    pub source_byte_len: usize,
    pub sections: Vec<ReadmeSection>,
    pub alignments: Vec<ReadmeSectionAlignment>,
    pub unknown_reasons: Vec<UnknownReason>,
}

impl Default for ReadmeTranslationAlignmentIR {
    fn default() -> Self {
        Self {
            semantic_revision: README_TRANSLATION_ALIGNMENT_REVISION.to_string(),
            path: String::new(),
            source_digest: PatchBaseDigest::from_bytes(&[]),
            source_byte_len: 0,
            sections: Vec::new(),
            alignments: Vec::new(),
            unknown_reasons: Vec::new(),
        }
    }
}

impl ReadmeTranslationAlignmentIR {
    pub fn try_new(
        path: impl Into<String>,
        source_digest: PatchBaseDigest,
        source_byte_len: usize,
        sections: Vec<ReadmeSection>,
        alignments: Vec<ReadmeSectionAlignment>,
        mut unknown_reasons: Vec<UnknownReason>,
    ) -> Result<Self, AppealIrError> {
        let path = path.into();
        if path.is_empty() {
            return Err(AppealIrError::EmptyPath);
        }
        validate_readme_sections(&sections, source_byte_len, &mut unknown_reasons)?;
        validate_translation_alignments(&sections, &alignments, &mut unknown_reasons)?;
        unknown_reasons.sort_unstable_by_key(|reason| unknown_reason_rank(*reason));
        unknown_reasons.dedup();
        Ok(Self {
            semantic_revision: README_TRANSLATION_ALIGNMENT_REVISION.to_string(),
            path,
            source_digest,
            source_byte_len,
            sections,
            alignments,
            unknown_reasons,
        })
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.path.is_empty()
            && self.source_byte_len == 0
            && self.sections.is_empty()
            && self.alignments.is_empty()
            && self.unknown_reasons.is_empty()
    }

    /// Revalidates the public wire shape after deserialization or direct field mutation.
    pub fn validate(&self) -> Result<(), AppealIrError> {
        if self.semantic_revision != README_TRANSLATION_ALIGNMENT_REVISION {
            return Err(AppealIrError::ReadmeTranslationSemanticRevisionMismatch);
        }
        if self.path.is_empty() {
            return if self == &Self::default() {
                Ok(())
            } else {
                Err(AppealIrError::EmptyPath)
            };
        }
        let mut canonical_unknown_reasons = self.unknown_reasons.clone();
        validate_readme_sections(
            &self.sections,
            self.source_byte_len,
            &mut canonical_unknown_reasons,
        )?;
        validate_translation_alignments(
            &self.sections,
            &self.alignments,
            &mut canonical_unknown_reasons,
        )?;
        canonical_unknown_reasons.sort_unstable_by_key(|reason| unknown_reason_rank(*reason));
        canonical_unknown_reasons.dedup();
        if canonical_unknown_reasons != self.unknown_reasons {
            return Err(AppealIrError::NonCanonicalTranslationAlignments);
        }
        Ok(())
    }
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
    #[serde(default)]
    pub claim_atoms: ClaimAtomIR,
    #[serde(
        default,
        skip_serializing_if = "ReadmeTranslationAlignmentIR::is_empty"
    )]
    pub translation_alignment: ReadmeTranslationAlignmentIR,
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
            claim_atoms: ClaimAtomIR::default(),
            translation_alignment: ReadmeTranslationAlignmentIR::default(),
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
        Self::try_new_with_claim_atoms(
            path,
            coverage,
            nodes,
            edges,
            diagnostics,
            ClaimAtomIR::default(),
        )
    }

    pub fn try_new_with_claim_atoms(
        path: impl Into<String>,
        coverage: CoverageStatus,
        nodes: Vec<GrammarNode>,
        edges: Vec<GrammarEdge>,
        diagnostics: Vec<GrammarDiagnostic>,
        claim_atoms: ClaimAtomIR,
    ) -> Result<Self, AppealIrError> {
        Self::try_new_with_claim_atoms_and_translation_alignment(
            path,
            coverage,
            nodes,
            edges,
            diagnostics,
            claim_atoms,
            ReadmeTranslationAlignmentIR::default(),
        )
    }

    pub fn try_new_with_claim_atoms_and_translation_alignment(
        path: impl Into<String>,
        coverage: CoverageStatus,
        nodes: Vec<GrammarNode>,
        edges: Vec<GrammarEdge>,
        diagnostics: Vec<GrammarDiagnostic>,
        claim_atoms: ClaimAtomIR,
        translation_alignment: ReadmeTranslationAlignmentIR,
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
        validate_claim_atoms(&claim_atoms, &nodes, &ids)?;
        if !translation_alignment.is_empty() && translation_alignment.path != path {
            return Err(AppealIrError::TranslationAlignmentPathMismatch);
        }
        validate_translation_claim_membership(&claim_atoms, &translation_alignment)?;
        Ok(Self {
            semantic_revision: README_GRAMMAR_REVISION.to_string(),
            path,
            coverage,
            nodes,
            edges,
            diagnostics,
            claim_atoms,
            translation_alignment,
        })
    }

    /// Revalidates grammar, claim-atom, and translation membership invariants
    /// without trusting public fields or serde-derived construction.
    pub fn validate(&self) -> Result<(), AppealIrError> {
        if self.semantic_revision != README_GRAMMAR_REVISION {
            return Err(AppealIrError::ReadmeGrammarSemanticRevisionMismatch);
        }
        if self.claim_atoms.semantic_revision != README_CLAIM_ATOM_REVISION {
            return Err(AppealIrError::ClaimAtomSemanticRevisionMismatch);
        }
        if self.path.is_empty() {
            return if self == &Self::default() {
                Ok(())
            } else {
                Err(AppealIrError::EmptyPath)
            };
        }
        let mut ids = BTreeSet::new();
        let mut previous = None;
        for node in &self.nodes {
            if !ids.insert(node.id) || previous.is_some_and(|id| id >= node.id) {
                return Err(AppealIrError::NonCanonicalGrammarNodes);
            }
            if node.state == AnswerState::Explicit && node.span.is_none() {
                return Err(AppealIrError::ExplicitGrammarWithoutSpan(node.id));
            }
            previous = Some(node.id);
        }
        if self
            .edges
            .iter()
            .any(|edge| !ids.contains(&edge.from) || !ids.contains(&edge.to))
        {
            return Err(AppealIrError::DanglingGrammarEdge);
        }
        validate_claim_atoms(&self.claim_atoms, &self.nodes, &ids)?;
        self.translation_alignment.validate()?;
        if !self.translation_alignment.is_empty() && self.translation_alignment.path != self.path {
            return Err(AppealIrError::TranslationAlignmentPathMismatch);
        }
        validate_translation_claim_membership(&self.claim_atoms, &self.translation_alignment)?;
        Ok(())
    }
}

fn validate_claim_atoms(
    claim_atoms: &ClaimAtomIR,
    nodes: &[GrammarNode],
    grammar_ids: &BTreeSet<GrammarNodeId>,
) -> Result<(), AppealIrError> {
    let mut atom_ids = BTreeSet::new();
    let mut atom_grammar_nodes = BTreeSet::new();
    let mut previous = None;
    for atom in &claim_atoms.atoms {
        if !atom_ids.insert(atom.id)
            || !atom_grammar_nodes.insert(atom.grammar_node)
            || previous.is_some_and(|id| id >= atom.id)
        {
            return Err(AppealIrError::NonCanonicalClaimAtoms);
        }
        previous = Some(atom.id);
        if !grammar_ids.contains(&atom.grammar_node) {
            return Err(AppealIrError::DanglingClaimAtomGrammarNode(atom.id));
        }
        let node = nodes
            .iter()
            .find(|node| node.id == atom.grammar_node)
            .ok_or(AppealIrError::DanglingClaimAtomGrammarNode(atom.id))?;
        if atom.dimension != node.predicate.dimension() {
            return Err(AppealIrError::ClaimAtomDimensionMismatch(atom.id));
        }
        if atom.modality != node.modality {
            return Err(AppealIrError::ClaimAtomModalityMismatch(atom.id));
        }
        if atom.language != node.language {
            return Err(AppealIrError::ClaimAtomLanguageMismatch(atom.id));
        }
        if atom.span != node.span {
            return Err(AppealIrError::ClaimAtomSpanMismatch(atom.id));
        }
        let expected_polarity = if node.negated {
            ClaimPolarity::Negative
        } else {
            ClaimPolarity::Positive
        };
        if atom.polarity != expected_polarity {
            return Err(AppealIrError::ClaimAtomPolarityMismatch(atom.id));
        }
        if !is_canonical_qualifier_set(&atom.qualifiers) {
            return Err(AppealIrError::NonCanonicalClaimAtomQualifiers(atom.id));
        }
    }
    Ok(())
}

fn validate_readme_sections(
    sections: &[ReadmeSection],
    source_byte_len: usize,
    unknown_reasons: &mut Vec<UnknownReason>,
) -> Result<(), AppealIrError> {
    if !sections.windows(2).all(|pair| pair[0].id < pair[1].id) {
        return Err(AppealIrError::NonCanonicalReadmeSections);
    }
    let mut assigned_claims = BTreeSet::new();
    for section in sections {
        if !section.span.is_valid() || section.span.byte_end > source_byte_len {
            return Err(AppealIrError::ReadmeSectionSpanOutOfBounds(section.id));
        }
        if !section.claim_atoms.windows(2).all(|pair| pair[0] < pair[1]) {
            return Err(AppealIrError::NonCanonicalReadmeSectionClaims(section.id));
        }
        for claim in &section.claim_atoms {
            if !assigned_claims.insert(*claim) {
                return Err(AppealIrError::ClaimAtomInMultipleReadmeSections(*claim));
            }
        }
        if let ReadmeSectionLanguage::Ambiguous(reason) = section.language {
            unknown_reasons.push(reason);
        }
    }
    Ok(())
}

fn validate_translation_alignments(
    sections: &[ReadmeSection],
    alignments: &[ReadmeSectionAlignment],
    unknown_reasons: &mut Vec<UnknownReason>,
) -> Result<(), AppealIrError> {
    if !alignments
        .windows(2)
        .all(|pair| translation_alignment_key(&pair[0]) < translation_alignment_key(&pair[1]))
    {
        return Err(AppealIrError::NonCanonicalTranslationAlignments);
    }
    let sections_by_id = sections
        .iter()
        .map(|section| (section.id, section))
        .collect::<BTreeMap<_, _>>();
    let mut undirected_pairs = BTreeSet::new();
    let mut aligned_claims = BTreeSet::new();
    for alignment in alignments {
        if alignment.source_section == alignment.counterpart_section {
            return Err(AppealIrError::InvalidTranslationSectionPair);
        }
        let pair = if alignment.source_section < alignment.counterpart_section {
            (alignment.source_section, alignment.counterpart_section)
        } else {
            (alignment.counterpart_section, alignment.source_section)
        };
        if !undirected_pairs.insert(pair) {
            return Err(AppealIrError::NonCanonicalTranslationAlignments);
        }
        let source = sections_by_id
            .get(&alignment.source_section)
            .copied()
            .ok_or(AppealIrError::DanglingTranslationSection)?;
        let counterpart = sections_by_id
            .get(&alignment.counterpart_section)
            .copied()
            .ok_or(AppealIrError::DanglingTranslationSection)?;
        let uncertain_language = matches!(
            source.language,
            ReadmeSectionLanguage::Mixed | ReadmeSectionLanguage::Ambiguous(_)
        ) || matches!(
            counterpart.language,
            ReadmeSectionLanguage::Mixed | ReadmeSectionLanguage::Ambiguous(_)
        );
        let bilingual_pair = matches!(
            (source.language, counterpart.language),
            (
                ReadmeSectionLanguage::Japanese,
                ReadmeSectionLanguage::English
            ) | (
                ReadmeSectionLanguage::English,
                ReadmeSectionLanguage::Japanese
            )
        );
        if !uncertain_language && !bilingual_pair {
            return Err(AppealIrError::TranslationLanguagePairMismatch);
        }
        if uncertain_language && !matches!(alignment.state, TranslationAlignmentState::Unknown(_)) {
            return Err(AppealIrError::TranslationLanguagePairMismatch);
        }
        if !alignment
            .claims
            .windows(2)
            .all(|pair| translation_claim_key(&pair[0]) < translation_claim_key(&pair[1]))
        {
            return Err(AppealIrError::NonCanonicalTranslationClaimAlignments);
        }
        for claim in &alignment.claims {
            if !aligned_claims.insert(claim.source_claim) {
                return Err(AppealIrError::DuplicateTranslationClaim(claim.source_claim));
            }
            if let Some(counterpart_claim) = claim.counterpart_claim {
                if !aligned_claims.insert(counterpart_claim) {
                    return Err(AppealIrError::DuplicateTranslationClaim(counterpart_claim));
                }
            }
            validate_translation_claim_alignment(claim, unknown_reasons)?;
            let source_in_source = source.claim_atoms.contains(&claim.source_claim);
            let source_in_counterpart = counterpart.claim_atoms.contains(&claim.source_claim);
            if source_in_source == source_in_counterpart {
                return Err(AppealIrError::TranslationClaimOutsideSection(
                    claim.source_claim,
                ));
            }
            if let Some(counterpart_claim) = claim.counterpart_claim {
                let counterpart_in_source = source.claim_atoms.contains(&counterpart_claim);
                let counterpart_in_counterpart =
                    counterpart.claim_atoms.contains(&counterpart_claim);
                if !counterpart_in_source && !counterpart_in_counterpart {
                    return Err(AppealIrError::TranslationClaimOutsideSection(
                        counterpart_claim,
                    ));
                }
                if (source_in_source && counterpart_in_source)
                    || (source_in_counterpart && counterpart_in_counterpart)
                {
                    return Err(AppealIrError::TranslationCounterpartOnSameSide(
                        counterpart_claim,
                    ));
                }
            }
        }
        match alignment.state {
            TranslationAlignmentState::Aligned
                if alignment.claims.is_empty()
                    || !alignment
                        .claims
                        .iter()
                        .all(|claim| claim.state == TranslationAlignmentState::Aligned) =>
            {
                return Err(AppealIrError::InvalidTranslationAlignmentState);
            }
            TranslationAlignmentState::Divergent
                if !alignment
                    .claims
                    .iter()
                    .any(|claim| claim.state == TranslationAlignmentState::Divergent)
                    || alignment.claims.iter().any(|claim| {
                        matches!(claim.state, TranslationAlignmentState::Unknown(_))
                    }) =>
            {
                return Err(AppealIrError::InvalidTranslationAlignmentState);
            }
            TranslationAlignmentState::Unknown(reason) => unknown_reasons.push(reason),
            TranslationAlignmentState::Aligned | TranslationAlignmentState::Divergent => {}
        }
    }
    Ok(())
}

fn validate_translation_claim_alignment(
    alignment: &TranslationClaimAlignment,
    unknown_reasons: &mut Vec<UnknownReason>,
) -> Result<(), AppealIrError> {
    if !alignment
        .divergences
        .windows(2)
        .all(|pair| pair[0] < pair[1])
    {
        return Err(AppealIrError::NonCanonicalTranslationDivergences);
    }
    let missing = alignment
        .divergences
        .contains(&TranslationDivergenceKind::MissingCounterpart);
    match alignment.state {
        TranslationAlignmentState::Aligned
            if alignment.counterpart_claim.is_none() || !alignment.divergences.is_empty() =>
        {
            Err(AppealIrError::InvalidTranslationAlignmentState)
        }
        TranslationAlignmentState::Divergent
            if alignment.divergences.is_empty()
                || missing != alignment.counterpart_claim.is_none() =>
        {
            Err(AppealIrError::InvalidTranslationAlignmentState)
        }
        TranslationAlignmentState::Unknown(reason) if !alignment.divergences.is_empty() => {
            unknown_reasons.push(reason);
            Err(AppealIrError::InvalidTranslationAlignmentState)
        }
        TranslationAlignmentState::Unknown(reason) => {
            unknown_reasons.push(reason);
            Ok(())
        }
        TranslationAlignmentState::Aligned | TranslationAlignmentState::Divergent => Ok(()),
    }
}

fn validate_translation_claim_membership(
    claim_atoms: &ClaimAtomIR,
    translation: &ReadmeTranslationAlignmentIR,
) -> Result<(), AppealIrError> {
    if translation.is_empty() {
        return Ok(());
    }
    let atoms = claim_atoms
        .atoms
        .iter()
        .map(|atom| (atom.id, atom))
        .collect::<BTreeMap<_, _>>();
    for section in &translation.sections {
        for claim_id in &section.claim_atoms {
            let atom = atoms
                .get(claim_id)
                .copied()
                .ok_or(AppealIrError::DanglingTranslationClaimAtom(*claim_id))?;
            let expected = match section.language {
                ReadmeSectionLanguage::Japanese => Some(DocumentLanguage::Japanese),
                ReadmeSectionLanguage::English => Some(DocumentLanguage::English),
                ReadmeSectionLanguage::Mixed | ReadmeSectionLanguage::Ambiguous(_) => None,
            };
            if expected.is_some_and(|language| atom.language != language) {
                return Err(AppealIrError::TranslationClaimLanguageMismatch(*claim_id));
            }
            let Some(span) = atom.span else {
                return Err(AppealIrError::TranslationClaimSpanOutsideSection(*claim_id));
            };
            if span.byte_start < section.span.byte_start || span.byte_end > section.span.byte_end {
                return Err(AppealIrError::TranslationClaimSpanOutsideSection(*claim_id));
            }
        }
    }
    for section_alignment in &translation.alignments {
        for claim_alignment in &section_alignment.claims {
            let source = atoms.get(&claim_alignment.source_claim).copied().ok_or(
                AppealIrError::DanglingTranslationClaimAtom(claim_alignment.source_claim),
            )?;
            if let Some(counterpart_id) = claim_alignment.counterpart_claim {
                let counterpart = atoms
                    .get(&counterpart_id)
                    .copied()
                    .ok_or(AppealIrError::DanglingTranslationClaimAtom(counterpart_id))?;
                if source.dimension != counterpart.dimension {
                    return Err(AppealIrError::TranslationClaimDimensionMismatch(
                        claim_alignment.source_claim,
                    ));
                }
                if source.subject != counterpart.subject
                    || source.action != counterpart.action
                    || source.object != counterpart.object
                {
                    return Err(AppealIrError::TranslationSemanticTermMismatch(
                        claim_alignment.source_claim,
                    ));
                }
                let polarity_diverges = source.polarity != counterpart.polarity;
                let polarity_marked = claim_alignment
                    .divergences
                    .contains(&TranslationDivergenceKind::Polarity);
                if polarity_diverges != polarity_marked {
                    return Err(AppealIrError::TranslationPolarityAssessmentMismatch(
                        claim_alignment.source_claim,
                    ));
                }
                let qualifier_diverges = source.qualifiers != counterpart.qualifiers;
                let qualifier_marked = claim_alignment
                    .divergences
                    .contains(&TranslationDivergenceKind::Qualifier);
                if qualifier_diverges != qualifier_marked {
                    return Err(AppealIrError::TranslationQualifierAssessmentMismatch(
                        claim_alignment.source_claim,
                    ));
                }
                let condition_diverges = source.condition != counterpart.condition;
                let condition_marked = claim_alignment
                    .divergences
                    .contains(&TranslationDivergenceKind::Condition);
                if condition_diverges != condition_marked {
                    return Err(AppealIrError::TranslationConditionAssessmentMismatch(
                        claim_alignment.source_claim,
                    ));
                }
            }
        }
    }
    Ok(())
}

fn translation_alignment_key(
    alignment: &ReadmeSectionAlignment,
) -> (ReadmeSectionId, ReadmeSectionId) {
    (alignment.source_section, alignment.counterpart_section)
}

fn translation_claim_key(
    alignment: &TranslationClaimAlignment,
) -> (ClaimAtomId, Option<ClaimAtomId>) {
    (alignment.source_claim, alignment.counterpart_claim)
}

fn is_canonical_qualifier_set(qualifiers: &[String]) -> bool {
    qualifiers.iter().all(|value| !value.trim().is_empty())
        && qualifiers.windows(2).all(|pair| pair[0] < pair[1])
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
    /// A deliberately imported, source-bound comparison receipt. Ordinary
    /// tests and benchmark paths never become this kind automatically.
    ComparisonReceipt,
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
            Self::FirstResult => ValueDimension::FirstResult,
            Self::Example => ValueDimension::FirstAction,
            Self::Test => ValueDimension::Evidence,
            Self::Constraint => ValueDimension::Constraint,
            Self::ComparisonReceipt => ValueDimension::Differentiation,
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

/// A bounded semantic projection for one capability node.
///
/// `semantic_support` describes support for the natural-language projection,
/// rather than support for the owning capability node itself. In particular,
/// source syntax can observe that an API exists, but interpreting its name and
/// type shape as prose is capped at [`CapabilitySupport::Inferred`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilitySemanticSignature {
    pub capability_node: CapabilityNodeId,
    pub dimension: ValueDimension,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub qualifiers: Vec<String>,
    pub polarity: ClaimPolarity,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub input_nodes: Vec<CapabilityNodeId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub output_nodes: Vec<CapabilityNodeId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub condition_nodes: Vec<CapabilityNodeId>,
    pub semantic_support: CapabilitySupport,
}

/// A source-bound region where the capability frontend could not make a safe observation.
/// Diagnostics never become capability evidence by themselves.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityDiagnostic {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<SourceSpan>,
    pub reason: UnknownReason,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepositoryCapabilityIR {
    pub semantic_revision: String,
    pub coverage: CoverageStatus,
    pub nodes: Vec<CapabilityNode>,
    pub edges: Vec<CapabilityEdge>,
    #[serde(default)]
    pub semantic_signatures: Vec<CapabilitySemanticSignature>,
    #[serde(default)]
    pub diagnostics: Vec<CapabilityDiagnostic>,
    pub unknown_reasons: Vec<UnknownReason>,
}

impl Default for RepositoryCapabilityIR {
    fn default() -> Self {
        Self {
            semantic_revision: REPOSITORY_CAPABILITY_REVISION.to_string(),
            coverage: CoverageStatus::NotRequested,
            nodes: Vec::new(),
            edges: Vec::new(),
            semantic_signatures: Vec::new(),
            diagnostics: Vec::new(),
            unknown_reasons: Vec::new(),
        }
    }
}

impl RepositoryCapabilityIR {
    pub fn try_new(
        coverage: CoverageStatus,
        nodes: Vec<CapabilityNode>,
        edges: Vec<CapabilityEdge>,
        unknown_reasons: Vec<UnknownReason>,
    ) -> Result<Self, AppealIrError> {
        Self::try_new_with_diagnostics(coverage, nodes, edges, Vec::new(), unknown_reasons)
    }

    pub fn try_new_with_diagnostics(
        coverage: CoverageStatus,
        nodes: Vec<CapabilityNode>,
        edges: Vec<CapabilityEdge>,
        diagnostics: Vec<CapabilityDiagnostic>,
        unknown_reasons: Vec<UnknownReason>,
    ) -> Result<Self, AppealIrError> {
        Self::try_new_with_semantic_signatures_and_diagnostics(
            coverage,
            nodes,
            edges,
            Vec::new(),
            diagnostics,
            unknown_reasons,
        )
    }

    pub fn try_new_with_semantic_signatures_and_diagnostics(
        coverage: CoverageStatus,
        nodes: Vec<CapabilityNode>,
        edges: Vec<CapabilityEdge>,
        semantic_signatures: Vec<CapabilitySemanticSignature>,
        diagnostics: Vec<CapabilityDiagnostic>,
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
        validate_capability_semantic_signatures(&nodes, &edges, &semantic_signatures)?;
        if diagnostics
            .iter()
            .any(|diagnostic| diagnostic.path.is_empty())
        {
            return Err(AppealIrError::EmptyPath);
        }
        if !diagnostics
            .windows(2)
            .all(|pair| capability_diagnostic_key(&pair[0]) < capability_diagnostic_key(&pair[1]))
        {
            return Err(AppealIrError::NonCanonicalCapabilityDiagnostics);
        }
        unknown_reasons.extend(nodes.iter().filter_map(|node| match node.support {
            CapabilitySupport::Unknown(reason) => Some(reason),
            CapabilitySupport::Observed | CapabilitySupport::Inferred => None,
        }));
        unknown_reasons.extend(semantic_signatures.iter().filter_map(|signature| {
            match signature.semantic_support {
                CapabilitySupport::Unknown(reason) => Some(reason),
                CapabilitySupport::Observed | CapabilitySupport::Inferred => None,
            }
        }));
        unknown_reasons.extend(diagnostics.iter().map(|diagnostic| diagnostic.reason));
        unknown_reasons.sort_unstable_by_key(|reason| unknown_reason_rank(*reason));
        unknown_reasons.dedup();
        Ok(Self {
            semantic_revision: REPOSITORY_CAPABILITY_REVISION.to_string(),
            coverage,
            nodes,
            edges,
            semantic_signatures,
            diagnostics,
            unknown_reasons,
        })
    }

    /// Revalidates capability ownership, provenance, semantic references, and
    /// canonical Unknown retention after public-field or wire construction.
    pub fn validate(&self) -> Result<(), AppealIrError> {
        if self.semantic_revision != REPOSITORY_CAPABILITY_REVISION {
            return Err(AppealIrError::RepositoryCapabilitySemanticRevisionMismatch);
        }
        let mut ids = BTreeSet::new();
        let mut previous = None;
        for node in &self.nodes {
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
        if self
            .edges
            .iter()
            .any(|edge| !ids.contains(&edge.from) || !ids.contains(&edge.to))
        {
            return Err(AppealIrError::DanglingCapabilityEdge);
        }
        validate_capability_semantic_signatures(
            &self.nodes,
            &self.edges,
            &self.semantic_signatures,
        )?;
        if self
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.path.is_empty())
        {
            return Err(AppealIrError::EmptyPath);
        }
        if !self
            .diagnostics
            .windows(2)
            .all(|pair| capability_diagnostic_key(&pair[0]) < capability_diagnostic_key(&pair[1]))
        {
            return Err(AppealIrError::NonCanonicalCapabilityDiagnostics);
        }
        let mut canonical_unknown_reasons = self.unknown_reasons.clone();
        canonical_unknown_reasons.extend(self.nodes.iter().filter_map(|node| match node.support {
            CapabilitySupport::Unknown(reason) => Some(reason),
            CapabilitySupport::Observed | CapabilitySupport::Inferred => None,
        }));
        canonical_unknown_reasons.extend(self.semantic_signatures.iter().filter_map(|signature| {
            match signature.semantic_support {
                CapabilitySupport::Unknown(reason) => Some(reason),
                CapabilitySupport::Observed | CapabilitySupport::Inferred => None,
            }
        }));
        canonical_unknown_reasons
            .extend(self.diagnostics.iter().map(|diagnostic| diagnostic.reason));
        canonical_unknown_reasons.sort_unstable_by_key(|reason| unknown_reason_rank(*reason));
        canonical_unknown_reasons.dedup();
        if canonical_unknown_reasons != self.unknown_reasons {
            return Err(AppealIrError::NonCanonicalCapabilityUnknownReasons);
        }
        Ok(())
    }
}

fn validate_capability_semantic_signatures(
    nodes: &[CapabilityNode],
    edges: &[CapabilityEdge],
    signatures: &[CapabilitySemanticSignature],
) -> Result<(), AppealIrError> {
    if !signatures
        .windows(2)
        .all(|pair| pair[0].capability_node < pair[1].capability_node)
    {
        return Err(AppealIrError::NonCanonicalCapabilitySemanticSignatures);
    }

    let nodes_by_id = nodes
        .iter()
        .map(|node| (node.id, node))
        .collect::<BTreeMap<_, _>>();
    for signature in signatures {
        let Some(owner) = nodes_by_id.get(&signature.capability_node).copied() else {
            return Err(AppealIrError::DanglingCapabilitySemanticSignatureOwner(
                signature.capability_node,
            ));
        };
        if signature.dimension != owner.kind.dimension() {
            return Err(AppealIrError::CapabilitySemanticSignatureDimensionMismatch(
                signature.capability_node,
            ));
        }
        if [&signature.subject, &signature.action, &signature.object]
            .into_iter()
            .flatten()
            .any(|term| !is_canonical_semantic_term(term))
        {
            return Err(AppealIrError::NonCanonicalCapabilitySemanticTerms(
                signature.capability_node,
            ));
        }
        if !is_canonical_semantic_qualifier_set(&signature.qualifiers) {
            return Err(AppealIrError::NonCanonicalCapabilitySemanticQualifiers(
                signature.capability_node,
            ));
        }
        if owner
            .provenance
            .iter()
            .any(|provenance| provenance.kind == CapabilityProvenanceKind::SourceSyntax)
            && signature.semantic_support == CapabilitySupport::Observed
        {
            return Err(AppealIrError::SourceSyntaxSemanticSupportMustBeInferred(
                signature.capability_node,
            ));
        }
        if !semantic_support_is_bounded_by(owner.support, signature.semantic_support) {
            return Err(AppealIrError::CapabilitySemanticSupportExceedsOwner(
                signature.capability_node,
            ));
        }

        validate_capability_semantic_references(
            signature,
            &nodes_by_id,
            edges,
            &signature.input_nodes,
            CapabilityKind::Input,
            CapabilityRelation::Accepts,
        )?;
        validate_capability_semantic_references(
            signature,
            &nodes_by_id,
            edges,
            &signature.output_nodes,
            CapabilityKind::Output,
            CapabilityRelation::Produces,
        )?;
        validate_capability_semantic_conditions(signature, &nodes_by_id, edges)?;
    }
    Ok(())
}

fn validate_capability_semantic_references(
    signature: &CapabilitySemanticSignature,
    nodes_by_id: &BTreeMap<CapabilityNodeId, &CapabilityNode>,
    edges: &[CapabilityEdge],
    references: &[CapabilityNodeId],
    expected_kind: CapabilityKind,
    expected_relation: CapabilityRelation,
) -> Result<(), AppealIrError> {
    validate_canonical_capability_references(signature.capability_node, references)?;
    for reference in references {
        let Some(target) = nodes_by_id.get(reference) else {
            return Err(AppealIrError::DanglingCapabilitySemanticReference(
                signature.capability_node,
            ));
        };
        if target.kind != expected_kind {
            return Err(AppealIrError::CapabilitySemanticReferenceKindMismatch(
                signature.capability_node,
            ));
        }
        if !edges.iter().any(|edge| {
            edge.from == signature.capability_node
                && edge.to == *reference
                && edge.relation == expected_relation
        }) {
            return Err(AppealIrError::MissingCapabilitySemanticRelation(
                signature.capability_node,
            ));
        }
    }
    Ok(())
}

fn validate_capability_semantic_conditions(
    signature: &CapabilitySemanticSignature,
    nodes_by_id: &BTreeMap<CapabilityNodeId, &CapabilityNode>,
    edges: &[CapabilityEdge],
) -> Result<(), AppealIrError> {
    validate_canonical_capability_references(
        signature.capability_node,
        &signature.condition_nodes,
    )?;
    for reference in &signature.condition_nodes {
        let Some(target) = nodes_by_id.get(reference) else {
            return Err(AppealIrError::DanglingCapabilitySemanticReference(
                signature.capability_node,
            ));
        };
        let expected_relation = match target.kind {
            CapabilityKind::Feature => CapabilityRelation::ConditionedBy,
            CapabilityKind::Constraint => CapabilityRelation::ConstrainedBy,
            _ => {
                return Err(AppealIrError::CapabilitySemanticReferenceKindMismatch(
                    signature.capability_node,
                ));
            }
        };
        if !edges.iter().any(|edge| {
            edge.from == signature.capability_node
                && edge.to == *reference
                && edge.relation == expected_relation
        }) {
            return Err(AppealIrError::MissingCapabilitySemanticRelation(
                signature.capability_node,
            ));
        }
    }
    Ok(())
}

fn validate_canonical_capability_references(
    owner: CapabilityNodeId,
    references: &[CapabilityNodeId],
) -> Result<(), AppealIrError> {
    if !references.windows(2).all(|pair| pair[0] < pair[1]) {
        return Err(AppealIrError::NonCanonicalCapabilitySemanticReferences(
            owner,
        ));
    }
    Ok(())
}

fn is_canonical_semantic_term(term: &str) -> bool {
    !term.is_empty() && term == term.trim()
}

fn is_canonical_semantic_qualifier_set(qualifiers: &[String]) -> bool {
    qualifiers
        .iter()
        .all(|qualifier| is_canonical_semantic_term(qualifier))
        && qualifiers.windows(2).all(|pair| pair[0] < pair[1])
}

const fn semantic_support_is_bounded_by(
    owner: CapabilitySupport,
    semantic: CapabilitySupport,
) -> bool {
    match owner {
        CapabilitySupport::Observed => true,
        CapabilitySupport::Inferred => !matches!(semantic, CapabilitySupport::Observed),
        CapabilitySupport::Unknown(_) => matches!(semantic, CapabilitySupport::Unknown(_)),
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
        unknown_reason_rank(diagnostic.reason),
    )
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimMode {
    #[default]
    Omitted,
    Generic,
    Qualified,
    Direct,
}

pub const CLAIM_DRAFT_MAX_PATH_BYTES: usize = 1_024;
pub const CLAIM_DRAFT_MAX_TOKEN_BYTES: usize = 64;
pub const CLAIM_DRAFT_MAX_PHRASE_BYTES: usize = 256;
pub const CLAIM_DRAFT_MAX_PHRASE_TOKENS: usize = 16;
pub const CLAIM_DRAFT_MAX_QUALIFIERS: usize = 16;
pub const CLAIM_DRAFT_MAX_REFERENCES: usize = 64;
pub const CLAIM_DRAFT_MAX_ITEMS: usize = 256;
pub const CLAIM_DRAFT_MAX_UNKNOWN_COUNT: usize = 1_000_000;

/// Structured claim meaning used by the draft round-trip. Fields are semantic
/// tokens or bounded token sequences, never a rendered README sentence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ClaimDraftSemantics {
    pub dimension: ValueDimension,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub qualifiers: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
    pub polarity: ClaimPolarity,
    pub modality: ClaimModality,
    pub language: DocumentLanguage,
}

impl ClaimDraftSemantics {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        dimension: ValueDimension,
        subject: Option<String>,
        action: Option<String>,
        object: Option<String>,
        qualifiers: Vec<String>,
        condition: Option<String>,
        polarity: ClaimPolarity,
        modality: ClaimModality,
        language: DocumentLanguage,
    ) -> Result<Self, AppealIrError> {
        let semantics = Self {
            dimension,
            subject,
            action,
            object,
            qualifiers,
            condition,
            polarity,
            modality,
            language,
        };
        semantics.validate()?;
        Ok(semantics)
    }

    pub fn validate(&self) -> Result<(), AppealIrError> {
        let canonical = (self.subject.is_some() || self.action.is_some() || self.object.is_some())
            && matches!(
                (self.polarity, self.modality),
                (ClaimPolarity::Negative, ClaimModality::Prohibited)
                    | (
                        ClaimPolarity::Positive,
                        ClaimModality::Asserted
                            | ClaimModality::Qualified
                            | ClaimModality::Possible
                            | ClaimModality::Required
                    )
            )
            && self
                .subject
                .as_deref()
                .is_none_or(is_canonical_claim_draft_token)
            && self
                .action
                .as_deref()
                .is_none_or(is_canonical_claim_draft_token)
            && self
                .object
                .as_deref()
                .is_none_or(is_canonical_claim_draft_phrase)
            && self.qualifiers.len() <= CLAIM_DRAFT_MAX_QUALIFIERS
            && self
                .qualifiers
                .iter()
                .all(|qualifier| is_canonical_claim_draft_token(qualifier))
            && self.qualifiers.windows(2).all(|pair| pair[0] < pair[1])
            && self
                .condition
                .as_deref()
                .is_none_or(is_canonical_claim_draft_phrase);
        if !canonical {
            return Err(AppealIrError::NonCanonicalClaimDraftSemantics);
        }
        Ok(())
    }

    /// Returns the only realization mode represented by this semantic
    /// modality. Keeping this mapping inside the typed payload prevents a
    /// draft from labelling asserted or prohibited wording as merely
    /// qualified during ceiling checks.
    #[must_use]
    pub const fn realization_mode(&self) -> ClaimMode {
        match self.modality {
            ClaimModality::Asserted | ClaimModality::Required | ClaimModality::Prohibited => {
                ClaimMode::Direct
            }
            ClaimModality::Qualified | ClaimModality::Possible => ClaimMode::Qualified,
        }
    }
}

impl<'de> Deserialize<'de> for ClaimDraftSemantics {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct WireSemantics {
            dimension: ValueDimension,
            #[serde(default)]
            subject: Option<String>,
            #[serde(default)]
            action: Option<String>,
            #[serde(default)]
            object: Option<String>,
            #[serde(default)]
            qualifiers: Vec<String>,
            #[serde(default)]
            condition: Option<String>,
            polarity: ClaimPolarity,
            modality: ClaimModality,
            language: DocumentLanguage,
        }

        let wire = WireSemantics::deserialize(deserializer)?;
        Self::try_new(
            wire.dimension,
            wire.subject,
            wire.action,
            wire.object,
            wire.qualifiers,
            wire.condition,
            wire.polarity,
            wire.modality,
            wire.language,
        )
        .map_err(D::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ClaimDraft {
    pub id: ClaimDraftId,
    pub target_span: SourceSpan,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_claims: Vec<ClaimAtomId>,
    pub capability_nodes: Vec<CapabilityNodeId>,
    pub semantics: ClaimDraftSemantics,
    pub realization_mode: ClaimMode,
    pub claim_ceiling: ClaimMode,
}

impl ClaimDraft {
    pub fn try_new(
        id: ClaimDraftId,
        target_span: SourceSpan,
        source_claims: Vec<ClaimAtomId>,
        capability_nodes: Vec<CapabilityNodeId>,
        semantics: ClaimDraftSemantics,
        realization_mode: ClaimMode,
        claim_ceiling: ClaimMode,
    ) -> Result<Self, AppealIrError> {
        let draft = Self {
            id,
            target_span,
            source_claims,
            capability_nodes,
            semantics,
            realization_mode,
            claim_ceiling,
        };
        draft.validate()?;
        Ok(draft)
    }

    pub fn validate(&self) -> Result<(), AppealIrError> {
        if !self.target_span.is_valid() {
            return Err(AppealIrError::ClaimDraftSpanOutOfBounds(self.id));
        }
        self.semantics.validate()?;
        if self.source_claims.len() > CLAIM_DRAFT_MAX_REFERENCES
            || self.capability_nodes.is_empty()
            || self.capability_nodes.len() > CLAIM_DRAFT_MAX_REFERENCES
            || !self.source_claims.windows(2).all(|pair| pair[0] < pair[1])
            || !self
                .capability_nodes
                .windows(2)
                .all(|pair| pair[0] < pair[1])
        {
            return Err(AppealIrError::NonCanonicalClaimDraftReferences(self.id));
        }
        if self.claim_ceiling == ClaimMode::Omitted {
            return Err(AppealIrError::ClaimDraftWithoutCeiling(self.id));
        }
        if self.realization_mode == ClaimMode::Omitted {
            return Err(AppealIrError::ClaimDraftWithoutRealization(self.id));
        }
        if self.realization_mode != self.semantics.realization_mode() {
            return Err(AppealIrError::ClaimDraftRealizationMismatch(self.id));
        }
        if self.realization_mode > self.claim_ceiling {
            return Err(AppealIrError::ClaimDraftCeilingExceeded(self.id));
        }
        Ok(())
    }
}

impl<'de> Deserialize<'de> for ClaimDraft {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct WireDraft {
            id: ClaimDraftId,
            target_span: SourceSpan,
            #[serde(default)]
            source_claims: Vec<ClaimAtomId>,
            capability_nodes: Vec<CapabilityNodeId>,
            semantics: ClaimDraftSemantics,
            realization_mode: ClaimMode,
            claim_ceiling: ClaimMode,
        }

        let wire = WireDraft::deserialize(deserializer)?;
        Self::try_new(
            wire.id,
            wire.target_span,
            wire.source_claims,
            wire.capability_nodes,
            wire.semantics,
            wire.realization_mode,
            wire.claim_ceiling,
        )
        .map_err(D::Error::custom)
    }
}

/// Source-bound, prose-free claim candidates. The IR contains no rendered
/// candidate document, output path, edit operation, or write authorization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ClaimDraftIR {
    pub semantic_revision: String,
    pub path: String,
    pub source_digest: PatchBaseDigest,
    pub source_byte_len: usize,
    pub baseline_unknown_count: usize,
    pub drafts: Vec<ClaimDraft>,
}

impl ClaimDraftIR {
    pub fn try_new(
        path: impl Into<String>,
        source_digest: PatchBaseDigest,
        source_byte_len: usize,
        baseline_unknown_count: usize,
        drafts: Vec<ClaimDraft>,
    ) -> Result<Self, AppealIrError> {
        let path = path.into();
        let ir = Self {
            semantic_revision: CLAIM_DRAFT_REVISION.to_string(),
            path,
            source_digest,
            source_byte_len,
            baseline_unknown_count,
            drafts,
        };
        ir.validate()?;
        Ok(ir)
    }

    pub fn validate(&self) -> Result<(), AppealIrError> {
        if self.semantic_revision != CLAIM_DRAFT_REVISION {
            return Err(AppealIrError::ClaimDraftSemanticRevisionMismatch);
        }
        if self.is_empty() {
            if self.source_digest != PatchBaseDigest::from_bytes(&[]) {
                return Err(AppealIrError::ClaimDraftSourceDigestMismatch);
            }
            return Ok(());
        }
        if !is_canonical_claim_draft_path(&self.path) {
            return Err(AppealIrError::InvalidClaimDraftPath);
        }
        if self.baseline_unknown_count > CLAIM_DRAFT_MAX_UNKNOWN_COUNT
            || self.drafts.len() > CLAIM_DRAFT_MAX_ITEMS
            || !self.drafts.windows(2).all(|pair| pair[0].id < pair[1].id)
        {
            return Err(AppealIrError::NonCanonicalClaimDrafts);
        }
        for draft in &self.drafts {
            draft.validate()?;
            if draft.target_span.byte_end > self.source_byte_len {
                return Err(AppealIrError::ClaimDraftSpanOutOfBounds(draft.id));
            }
        }
        Ok(())
    }

    pub fn validate_against_source(
        &self,
        path: &str,
        source: impl AsRef<[u8]>,
    ) -> Result<(), AppealIrError> {
        self.validate()?;
        if path != self.path {
            return Err(AppealIrError::ClaimDraftSourcePathMismatch);
        }
        let source = source.as_ref();
        if source.len() != self.source_byte_len {
            return Err(AppealIrError::ClaimDraftSourceLengthMismatch);
        }
        if PatchBaseDigest::from_bytes(source) != self.source_digest {
            return Err(AppealIrError::ClaimDraftSourceDigestMismatch);
        }
        let source =
            std::str::from_utf8(source).map_err(|_| AppealIrError::ClaimDraftSourceNotUtf8)?;
        for draft in &self.drafts {
            if !source.is_char_boundary(draft.target_span.byte_start)
                || !source.is_char_boundary(draft.target_span.byte_end)
            {
                return Err(AppealIrError::ClaimDraftSpanNotCharBoundary(draft.id));
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn draft(&self, id: ClaimDraftId) -> Option<&ClaimDraft> {
        self.drafts
            .binary_search_by_key(&id, |draft| draft.id)
            .ok()
            .map(|index| &self.drafts[index])
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.path.is_empty()
            && self.source_byte_len == 0
            && self.baseline_unknown_count == 0
            && self.drafts.is_empty()
    }
}

impl<'de> Deserialize<'de> for ClaimDraftIR {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct WireDraftIr {
            semantic_revision: String,
            path: String,
            source_digest: PatchBaseDigest,
            source_byte_len: usize,
            baseline_unknown_count: usize,
            drafts: Vec<ClaimDraft>,
        }

        let wire = WireDraftIr::deserialize(deserializer)?;
        let ir = Self {
            semantic_revision: wire.semantic_revision,
            path: wire.path,
            source_digest: wire.source_digest,
            source_byte_len: wire.source_byte_len,
            baseline_unknown_count: wire.baseline_unknown_count,
            drafts: wire.drafts,
        };
        ir.validate().map_err(D::Error::custom)?;
        Ok(ir)
    }
}

impl Default for ClaimDraftIR {
    fn default() -> Self {
        Self {
            semantic_revision: CLAIM_DRAFT_REVISION.to_string(),
            path: String::new(),
            source_digest: PatchBaseDigest::from_bytes(&[]),
            source_byte_len: 0,
            baseline_unknown_count: 0,
            drafts: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimDraftReauditObservation {
    pub observed_source_digest: PatchBaseDigest,
    pub observed_source_byte_len: usize,
    pub candidate_path: String,
    pub candidate_source_digest: PatchBaseDigest,
    pub candidate_source_byte_len: usize,
    pub candidate_unknown_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_semantics: Option<ClaimDraftSemantics>,
    pub candidate_mode: ClaimMode,
    pub scope_preserved: bool,
}

impl ClaimDraftReauditObservation {
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        observed_source_digest: PatchBaseDigest,
        observed_source_byte_len: usize,
        candidate_path: impl Into<String>,
        candidate_source_digest: PatchBaseDigest,
        candidate_source_byte_len: usize,
        candidate_unknown_count: usize,
        candidate_semantics: Option<ClaimDraftSemantics>,
        candidate_mode: ClaimMode,
        scope_preserved: bool,
    ) -> Self {
        Self {
            observed_source_digest,
            observed_source_byte_len,
            candidate_path: candidate_path.into(),
            candidate_source_digest,
            candidate_source_byte_len,
            candidate_unknown_count,
            candidate_semantics,
            candidate_mode,
            scope_preserved,
        }
    }

    pub fn validate(&self) -> Result<(), AppealIrError> {
        if self.candidate_unknown_count > CLAIM_DRAFT_MAX_UNKNOWN_COUNT {
            return Err(AppealIrError::NonCanonicalClaimDraftReaudit);
        }
        if let Some(semantics) = &self.candidate_semantics {
            semantics.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimDraftReauditHoldReason {
    StaleSourceDigest,
    UnknownIncreased,
    CeilingExceeded,
    SemanticDrift,
    ScopeEscape,
}

pub type ClaimDraftReauditHold = ClaimDraftReauditHoldReason;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimDraftReauditDecision {
    Accepted,
    Held,
}

/// Result of a pure candidate re-audit. It carries digests and typed facts, but
/// never the candidate README body or a write operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ClaimDraftReauditResult {
    pub semantic_revision: String,
    pub draft: ClaimDraftId,
    pub path: String,
    pub source_digest: PatchBaseDigest,
    pub source_byte_len: usize,
    pub observed_source_digest: PatchBaseDigest,
    pub observed_source_byte_len: usize,
    pub candidate_path: String,
    pub candidate_source_digest: PatchBaseDigest,
    pub candidate_source_byte_len: usize,
    pub baseline_unknown_count: usize,
    pub candidate_unknown_count: usize,
    pub candidate_mode: ClaimMode,
    pub claim_ceiling: ClaimMode,
    pub candidate_semantics_match: bool,
    pub scope_preserved: bool,
    pub candidate_checks_performed: bool,
    pub decision: ClaimDraftReauditDecision,
    pub holds: Vec<ClaimDraftReauditHoldReason>,
}

impl ClaimDraftReauditResult {
    pub fn evaluate(
        drafts: &ClaimDraftIR,
        draft_id: ClaimDraftId,
        observation: ClaimDraftReauditObservation,
    ) -> Result<Self, AppealIrError> {
        drafts.validate()?;
        observation.validate()?;
        let draft = drafts
            .draft(draft_id)
            .ok_or(AppealIrError::DanglingClaimDraft(draft_id))?;
        let stale = observation.observed_source_digest != drafts.source_digest
            || observation.observed_source_byte_len != drafts.source_byte_len;
        let candidate_semantics_match =
            observation.candidate_semantics.as_ref() == Some(&draft.semantics);
        let mut holds = Vec::new();
        if stale {
            holds.push(ClaimDraftReauditHoldReason::StaleSourceDigest);
        } else {
            if observation.candidate_unknown_count > drafts.baseline_unknown_count {
                holds.push(ClaimDraftReauditHoldReason::UnknownIncreased);
            }
            if observation.candidate_mode > draft.claim_ceiling {
                holds.push(ClaimDraftReauditHoldReason::CeilingExceeded);
            }
            if observation.candidate_mode == ClaimMode::Omitted || !candidate_semantics_match {
                holds.push(ClaimDraftReauditHoldReason::SemanticDrift);
            }
            if observation.candidate_path != drafts.path
                || !is_canonical_claim_draft_path(&observation.candidate_path)
                || !observation.scope_preserved
            {
                holds.push(ClaimDraftReauditHoldReason::ScopeEscape);
            }
        }
        let decision = if holds.is_empty() {
            ClaimDraftReauditDecision::Accepted
        } else {
            ClaimDraftReauditDecision::Held
        };
        let result = Self {
            semantic_revision: CLAIM_DRAFT_REAUDIT_REVISION.to_string(),
            draft: draft_id,
            path: drafts.path.clone(),
            source_digest: drafts.source_digest,
            source_byte_len: drafts.source_byte_len,
            observed_source_digest: observation.observed_source_digest,
            observed_source_byte_len: observation.observed_source_byte_len,
            candidate_path: observation.candidate_path,
            candidate_source_digest: observation.candidate_source_digest,
            candidate_source_byte_len: observation.candidate_source_byte_len,
            baseline_unknown_count: drafts.baseline_unknown_count,
            candidate_unknown_count: observation.candidate_unknown_count,
            candidate_mode: observation.candidate_mode,
            claim_ceiling: draft.claim_ceiling,
            candidate_semantics_match,
            scope_preserved: observation.scope_preserved,
            candidate_checks_performed: !stale,
            decision,
            holds,
        };
        result.validate_against_drafts(drafts)?;
        Ok(result)
    }

    pub fn validate(&self) -> Result<(), AppealIrError> {
        if self.semantic_revision != CLAIM_DRAFT_REAUDIT_REVISION
            || !is_canonical_claim_draft_path(&self.path)
            || self.baseline_unknown_count > CLAIM_DRAFT_MAX_UNKNOWN_COUNT
            || self.candidate_unknown_count > CLAIM_DRAFT_MAX_UNKNOWN_COUNT
            || self.holds.len() > 5
            || !self.holds.windows(2).all(|pair| pair[0] < pair[1])
        {
            return Err(AppealIrError::NonCanonicalClaimDraftReaudit);
        }
        let stale = self.observed_source_digest != self.source_digest
            || self.observed_source_byte_len != self.source_byte_len;
        let mut expected = Vec::new();
        if stale {
            expected.push(ClaimDraftReauditHoldReason::StaleSourceDigest);
        } else {
            if self.candidate_unknown_count > self.baseline_unknown_count {
                expected.push(ClaimDraftReauditHoldReason::UnknownIncreased);
            }
            if self.candidate_mode > self.claim_ceiling {
                expected.push(ClaimDraftReauditHoldReason::CeilingExceeded);
            }
            if self.candidate_mode == ClaimMode::Omitted || !self.candidate_semantics_match {
                expected.push(ClaimDraftReauditHoldReason::SemanticDrift);
            }
            if self.candidate_path != self.path
                || !is_canonical_claim_draft_path(&self.candidate_path)
                || !self.scope_preserved
            {
                expected.push(ClaimDraftReauditHoldReason::ScopeEscape);
            }
        }
        let expected_decision = if expected.is_empty() {
            ClaimDraftReauditDecision::Accepted
        } else {
            ClaimDraftReauditDecision::Held
        };
        if self.holds != expected
            || self.decision != expected_decision
            || self.candidate_checks_performed == stale
        {
            return Err(AppealIrError::NonCanonicalClaimDraftReaudit);
        }
        Ok(())
    }

    pub fn validate_against_drafts(&self, drafts: &ClaimDraftIR) -> Result<(), AppealIrError> {
        self.validate()?;
        drafts.validate()?;
        let draft = drafts
            .draft(self.draft)
            .ok_or(AppealIrError::DanglingClaimDraft(self.draft))?;
        if self.path != drafts.path
            || self.source_digest != drafts.source_digest
            || self.source_byte_len != drafts.source_byte_len
            || self.baseline_unknown_count != drafts.baseline_unknown_count
            || self.claim_ceiling != draft.claim_ceiling
        {
            return Err(AppealIrError::ClaimDraftReauditBindingMismatch(self.draft));
        }
        Ok(())
    }

    #[must_use]
    pub const fn writes_files(&self) -> bool {
        false
    }
}

impl<'de> Deserialize<'de> for ClaimDraftReauditResult {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct WireResult {
            semantic_revision: String,
            draft: ClaimDraftId,
            path: String,
            source_digest: PatchBaseDigest,
            source_byte_len: usize,
            observed_source_digest: PatchBaseDigest,
            observed_source_byte_len: usize,
            candidate_path: String,
            candidate_source_digest: PatchBaseDigest,
            candidate_source_byte_len: usize,
            baseline_unknown_count: usize,
            candidate_unknown_count: usize,
            candidate_mode: ClaimMode,
            #[serde(default)]
            claim_ceiling: ClaimMode,
            #[serde(default)]
            candidate_semantics_match: bool,
            #[serde(default)]
            scope_preserved: bool,
            candidate_checks_performed: bool,
            decision: ClaimDraftReauditDecision,
            holds: Vec<ClaimDraftReauditHoldReason>,
        }

        let wire = WireResult::deserialize(deserializer)?;
        let result = Self {
            semantic_revision: wire.semantic_revision,
            draft: wire.draft,
            path: wire.path,
            source_digest: wire.source_digest,
            source_byte_len: wire.source_byte_len,
            observed_source_digest: wire.observed_source_digest,
            observed_source_byte_len: wire.observed_source_byte_len,
            candidate_path: wire.candidate_path,
            candidate_source_digest: wire.candidate_source_digest,
            candidate_source_byte_len: wire.candidate_source_byte_len,
            baseline_unknown_count: wire.baseline_unknown_count,
            candidate_unknown_count: wire.candidate_unknown_count,
            candidate_mode: wire.candidate_mode,
            claim_ceiling: wire.claim_ceiling,
            candidate_semantics_match: wire.candidate_semantics_match,
            scope_preserved: wire.scope_preserved,
            candidate_checks_performed: wire.candidate_checks_performed,
            decision: wire.decision,
            holds: wire.holds,
        };
        result.validate().map_err(D::Error::custom)?;
        Ok(result)
    }
}

fn is_canonical_claim_draft_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= CLAIM_DRAFT_MAX_PATH_BYTES
        && !path.starts_with('/')
        && !path.contains(['\\', '\0'])
        && path.split('/').all(|component| {
            !component.is_empty()
                && component != "."
                && component != ".."
                && !component.contains(':')
        })
}

fn is_canonical_claim_draft_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= CLAIM_DRAFT_MAX_TOKEN_BYTES
        && value == value.trim()
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-' | b'.')
        })
}

fn is_canonical_claim_draft_phrase(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= CLAIM_DRAFT_MAX_PHRASE_BYTES
        && value == value.trim()
        && !value.contains("  ")
        && !value
            .chars()
            .any(|character| character.is_whitespace() && character != ' ')
        && value.split(' ').count() <= CLAIM_DRAFT_MAX_PHRASE_TOKENS
        && value.split(' ').all(is_canonical_claim_draft_token)
}

/// The typed source of one evidence-ceiling contribution.
///
/// `DifferentiationReceipt` is intentionally separate from ordinary capability
/// presence: a comparative claim needs an explicitly bound receipt rather than
/// merely a source symbol or test path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceCeilingBasis {
    Capability(CapabilityKind),
    DifferentiationReceipt,
}

/// A single, support-bounded contribution to an aggregate [`ClaimCeiling`].
///
/// Construction derives `maximum` from the basis, dimension, and support. An
/// Unknown contribution is retained but contributes only `Omitted`; an
/// Inferred contribution can never contribute `Direct`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceCeilingContribution {
    capability_node: CapabilityNodeId,
    basis: EvidenceCeilingBasis,
    dimension: ValueDimension,
    support: CapabilitySupport,
    maximum: ClaimMode,
}

impl EvidenceCeilingContribution {
    pub fn try_new(
        owner: &CapabilityNode,
        dimension: ValueDimension,
        support: CapabilitySupport,
    ) -> Result<Self, AppealIrError> {
        if !semantic_support_is_bounded_by(owner.support, support) {
            return Err(AppealIrError::CapabilitySemanticSupportExceedsOwner(
                owner.id,
            ));
        }
        let basis = if owner.kind == CapabilityKind::ComparisonReceipt {
            EvidenceCeilingBasis::DifferentiationReceipt
        } else {
            EvidenceCeilingBasis::Capability(owner.kind)
        };
        let observed_maximum = observed_evidence_ceiling(basis, dimension)
            .ok_or(AppealIrError::EvidenceCeilingDimensionMismatch(owner.id))?;
        let maximum = match support {
            CapabilitySupport::Observed => observed_maximum,
            CapabilitySupport::Inferred => match observed_maximum {
                ClaimMode::Direct => ClaimMode::Qualified,
                other => other,
            },
            CapabilitySupport::Unknown(_) => ClaimMode::Omitted,
        };
        Ok(Self {
            capability_node: owner.id,
            basis,
            dimension,
            support,
            maximum,
        })
    }

    #[must_use]
    pub const fn capability_node(self) -> CapabilityNodeId {
        self.capability_node
    }

    #[must_use]
    pub const fn basis(self) -> EvidenceCeilingBasis {
        self.basis
    }

    #[must_use]
    pub const fn dimension(self) -> ValueDimension {
        self.dimension
    }

    #[must_use]
    pub const fn support(self) -> CapabilitySupport {
        self.support
    }

    #[must_use]
    pub const fn maximum(self) -> ClaimMode {
        self.maximum
    }
}

const fn observed_evidence_ceiling(
    basis: EvidenceCeilingBasis,
    dimension: ValueDimension,
) -> Option<ClaimMode> {
    match (basis, dimension) {
        (
            EvidenceCeilingBasis::Capability(
                CapabilityKind::Manifest | CapabilityKind::ProgramLanguage,
            ),
            ValueDimension::Identity,
        )
        | (
            EvidenceCeilingBasis::Capability(
                CapabilityKind::Entrypoint
                | CapabilityKind::PublicApi
                | CapabilityKind::Operation
                | CapabilityKind::Input
                | CapabilityKind::Feature,
            ),
            ValueDimension::Capability,
        )
        | (EvidenceCeilingBasis::Capability(CapabilityKind::Test), ValueDimension::Evidence) => {
            Some(ClaimMode::Direct)
        }
        (EvidenceCeilingBasis::Capability(CapabilityKind::Output), ValueDimension::Outcome)
        | (
            EvidenceCeilingBasis::Capability(CapabilityKind::FirstResult),
            ValueDimension::FirstResult,
        )
        | (
            EvidenceCeilingBasis::Capability(CapabilityKind::Example),
            ValueDimension::FirstAction,
        )
        | (
            EvidenceCeilingBasis::Capability(CapabilityKind::Constraint),
            ValueDimension::Constraint,
        )
        | (EvidenceCeilingBasis::DifferentiationReceipt, ValueDimension::Differentiation) => {
            Some(ClaimMode::Qualified)
        }
        (EvidenceCeilingBasis::Capability(CapabilityKind::Example), ValueDimension::Evidence) => {
            Some(ClaimMode::Generic)
        }
        _ => None,
    }
}

/// The source state and realized specificity of one validated claim atom.
/// Negative claims remain realizations; polarity is not silently treated as
/// absence. Unknown/contested/missing source states retain their state while
/// realizing only `Omitted`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimRealization {
    pub claim_atom: ClaimAtomId,
    pub grammar_node: GrammarNodeId,
    pub dimension: ValueDimension,
    pub polarity: ClaimPolarity,
    pub state: AnswerState,
    pub mode: ClaimMode,
}

impl ClaimRealization {
    pub fn try_new(atom: &ClaimAtom, node: &GrammarNode) -> Result<Self, AppealIrError> {
        if atom.grammar_node != node.id {
            return Err(AppealIrError::DanglingClaimAtomGrammarNode(atom.id));
        }
        if atom.dimension != node.predicate.dimension() {
            return Err(AppealIrError::ClaimAtomDimensionMismatch(atom.id));
        }
        if atom.modality != node.modality {
            return Err(AppealIrError::ClaimAtomModalityMismatch(atom.id));
        }
        if atom.language != node.language {
            return Err(AppealIrError::ClaimAtomLanguageMismatch(atom.id));
        }
        if atom.span != node.span {
            return Err(AppealIrError::ClaimAtomSpanMismatch(atom.id));
        }
        let polarity = if node.negated {
            ClaimPolarity::Negative
        } else {
            ClaimPolarity::Positive
        };
        if atom.polarity != polarity {
            return Err(AppealIrError::ClaimAtomPolarityMismatch(atom.id));
        }
        let mode = match (node.state, node.modality) {
            (AnswerState::Explicit, ClaimModality::Asserted) => ClaimMode::Direct,
            (AnswerState::Explicit, _) => ClaimMode::Qualified,
            (AnswerState::Inferred, _) => ClaimMode::Generic,
            (AnswerState::Missing | AnswerState::Contested | AnswerState::Unknown(_), _) => {
                ClaimMode::Omitted
            }
        };
        Ok(Self {
            claim_atom: atom.id,
            grammar_node: node.id,
            dimension: atom.dimension,
            polarity,
            state: node.state,
            mode,
        })
    }
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

/// Proposition-level evidence alignment between one README claim atom and repository capability
/// signatures. A shared value dimension alone never creates an alignment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimCapabilityAlignment {
    pub claim_atom: ClaimAtomId,
    pub capability_nodes: Vec<CapabilityNodeId>,
    pub evidence_count: usize,
    pub state: SupportState,
    /// Claim-specific evidence ceiling. Legacy payloads and constructors remain
    /// fail-closed at `Omitted` until semantic contributions are supplied.
    #[serde(default, skip_serializing_if = "claim_mode_is_omitted")]
    pub claim_ceiling: ClaimMode,
}

fn claim_mode_is_omitted(mode: &ClaimMode) -> bool {
    *mode == ClaimMode::Omitted
}

impl ClaimCapabilityAlignment {
    pub fn try_new(
        claim_atom: ClaimAtomId,
        capability_nodes: Vec<CapabilityNodeId>,
        evidence_count: usize,
        state: SupportState,
    ) -> Result<Self, AppealIrError> {
        Self::try_new_with_evidence_ceiling(
            claim_atom,
            capability_nodes,
            evidence_count,
            state,
            &[],
        )
    }

    pub fn try_new_with_evidence_ceiling(
        claim_atom: ClaimAtomId,
        mut capability_nodes: Vec<CapabilityNodeId>,
        evidence_count: usize,
        state: SupportState,
        contributions: &[EvidenceCeilingContribution],
    ) -> Result<Self, AppealIrError> {
        capability_nodes.sort_unstable();
        capability_nodes.dedup();
        if state == SupportState::Supported && (capability_nodes.is_empty() || evidence_count == 0)
        {
            return Err(AppealIrError::SupportedWithoutEvidence);
        }
        if contributions
            .iter()
            .any(|contribution| !capability_nodes.contains(&contribution.capability_node()))
        {
            return Err(AppealIrError::UnalignedEvidenceCeilingContribution(
                claim_atom,
            ));
        }
        let claim_ceiling = if state == SupportState::Supported {
            contributions
                .iter()
                .map(|contribution| contribution.maximum())
                .max()
                .unwrap_or(ClaimMode::Omitted)
        } else {
            ClaimMode::Omitted
        };
        Ok(Self {
            claim_atom,
            capability_nodes,
            evidence_count,
            state,
            claim_ceiling,
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
    IdentityNotObserved,
    UnsupportedCapability,
    UnsupportedOutcome,
    RuntimeSuccessNotObserved,
    DifferentiationNotEstablished,
    PerformanceNotMeasured,
    TrustNotEstablished,
    SecurityNotEstablished,
    AudienceNotObserved,
    ProblemNotObserved,
    FirstActionNotObserved,
    EvidenceNotEstablished,
    ConstraintNotEstablished,
}

impl OverclaimRiskKind {
    /// Exhaustive default risk category for each of the ten value dimensions.
    #[must_use]
    pub const fn for_dimension(dimension: ValueDimension) -> Self {
        match dimension {
            ValueDimension::Identity => Self::IdentityNotObserved,
            ValueDimension::Audience => Self::AudienceNotObserved,
            ValueDimension::Problem => Self::ProblemNotObserved,
            ValueDimension::Capability => Self::UnsupportedCapability,
            ValueDimension::Outcome => Self::UnsupportedOutcome,
            ValueDimension::FirstAction => Self::FirstActionNotObserved,
            ValueDimension::FirstResult => Self::RuntimeSuccessNotObserved,
            ValueDimension::Evidence => Self::EvidenceNotEstablished,
            ValueDimension::Constraint => Self::ConstraintNotEstablished,
            ValueDimension::Differentiation => Self::DifferentiationNotEstablished,
        }
    }
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
    #[serde(default)]
    pub alignments: Vec<ClaimCapabilityAlignment>,
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
            alignments: Vec::new(),
            opportunities: Vec::new(),
            risks: Vec::new(),
            losses: AppealLossVector::default(),
        }
    }
}

impl ClaimCapabilityMembrane {
    /// Revalidates the public membrane wire without recomputing semantic
    /// evaluation. Unknown and evidence ceilings remain fail-closed.
    pub fn validate(&self) -> Result<(), AppealIrError> {
        if self.semantic_revision != CLAIM_CAPABILITY_MEMBRANE_REVISION {
            return Err(AppealIrError::ClaimCapabilityMembraneSemanticRevisionMismatch);
        }
        if !self
            .relations
            .windows(2)
            .all(|pair| pair[0].dimension < pair[1].dimension)
            || !self
                .alignments
                .windows(2)
                .all(|pair| pair[0].claim_atom < pair[1].claim_atom)
        {
            return Err(AppealIrError::NonCanonicalClaimCapabilityMembrane);
        }
        for relation in &self.relations {
            if !relation
                .grammar_nodes
                .windows(2)
                .all(|pair| pair[0] < pair[1])
                || !relation
                    .capability_nodes
                    .windows(2)
                    .all(|pair| pair[0] < pair[1])
                || (relation.state == SupportState::Supported
                    && (relation.capability_nodes.is_empty() || relation.evidence_count == 0))
            {
                return Err(AppealIrError::NonCanonicalClaimCapabilityMembrane);
            }
        }
        for alignment in &self.alignments {
            if !alignment
                .capability_nodes
                .windows(2)
                .all(|pair| pair[0] < pair[1])
                || (alignment.state == SupportState::Supported
                    && (alignment.capability_nodes.is_empty() || alignment.evidence_count == 0))
                || (alignment.state != SupportState::Supported
                    && alignment.claim_ceiling != ClaimMode::Omitted)
            {
                return Err(AppealIrError::NonCanonicalClaimCapabilityMembrane);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppealIrError {
    EmptyPath,
    ReadmeGrammarSemanticRevisionMismatch,
    ClaimAtomSemanticRevisionMismatch,
    ReadmeTranslationSemanticRevisionMismatch,
    NonCanonicalGrammarNodes,
    ExplicitGrammarWithoutSpan(GrammarNodeId),
    DanglingGrammarEdge,
    NonCanonicalClaimAtoms,
    DanglingClaimAtomGrammarNode(ClaimAtomId),
    ClaimAtomDimensionMismatch(ClaimAtomId),
    ClaimAtomModalityMismatch(ClaimAtomId),
    ClaimAtomLanguageMismatch(ClaimAtomId),
    ClaimAtomSpanMismatch(ClaimAtomId),
    ClaimAtomPolarityMismatch(ClaimAtomId),
    NonCanonicalClaimAtomQualifiers(ClaimAtomId),
    NonCanonicalReadmeSections,
    ReadmeSectionSpanOutOfBounds(ReadmeSectionId),
    NonCanonicalReadmeSectionClaims(ReadmeSectionId),
    ClaimAtomInMultipleReadmeSections(ClaimAtomId),
    TranslationAlignmentPathMismatch,
    NonCanonicalTranslationAlignments,
    InvalidTranslationSectionPair,
    DanglingTranslationSection,
    TranslationLanguagePairMismatch,
    NonCanonicalTranslationClaimAlignments,
    DuplicateTranslationClaim(ClaimAtomId),
    NonCanonicalTranslationDivergences,
    InvalidTranslationAlignmentState,
    TranslationClaimOutsideSection(ClaimAtomId),
    TranslationCounterpartOnSameSide(ClaimAtomId),
    DanglingTranslationClaimAtom(ClaimAtomId),
    TranslationClaimLanguageMismatch(ClaimAtomId),
    TranslationClaimSpanOutsideSection(ClaimAtomId),
    TranslationClaimDimensionMismatch(ClaimAtomId),
    TranslationSemanticTermMismatch(ClaimAtomId),
    TranslationPolarityAssessmentMismatch(ClaimAtomId),
    TranslationQualifierAssessmentMismatch(ClaimAtomId),
    TranslationConditionAssessmentMismatch(ClaimAtomId),
    NonCanonicalCapabilityNodes,
    NonCanonicalCapabilityDiagnostics,
    RepositoryCapabilitySemanticRevisionMismatch,
    NonCanonicalCapabilityUnknownReasons,
    CapabilityWithoutProvenance(CapabilityNodeId),
    DanglingCapabilityEdge,
    NonCanonicalCapabilitySemanticSignatures,
    DanglingCapabilitySemanticSignatureOwner(CapabilityNodeId),
    CapabilitySemanticSignatureDimensionMismatch(CapabilityNodeId),
    NonCanonicalCapabilitySemanticTerms(CapabilityNodeId),
    NonCanonicalCapabilitySemanticQualifiers(CapabilityNodeId),
    NonCanonicalCapabilitySemanticReferences(CapabilityNodeId),
    DanglingCapabilitySemanticReference(CapabilityNodeId),
    CapabilitySemanticReferenceKindMismatch(CapabilityNodeId),
    MissingCapabilitySemanticRelation(CapabilityNodeId),
    CapabilitySemanticSupportExceedsOwner(CapabilityNodeId),
    SourceSyntaxSemanticSupportMustBeInferred(CapabilityNodeId),
    EvidenceCeilingDimensionMismatch(CapabilityNodeId),
    UnalignedEvidenceCeilingContribution(ClaimAtomId),
    ClaimDraftSemanticRevisionMismatch,
    InvalidClaimDraftPath,
    NonCanonicalClaimDrafts,
    ClaimDraftSpanOutOfBounds(ClaimDraftId),
    NonCanonicalClaimDraftSemantics,
    NonCanonicalClaimDraftReferences(ClaimDraftId),
    ClaimDraftWithoutCeiling(ClaimDraftId),
    ClaimDraftWithoutRealization(ClaimDraftId),
    ClaimDraftRealizationMismatch(ClaimDraftId),
    ClaimDraftCeilingExceeded(ClaimDraftId),
    DanglingClaimDraft(ClaimDraftId),
    ClaimDraftSourcePathMismatch,
    ClaimDraftSourceDigestMismatch,
    ClaimDraftSourceLengthMismatch,
    ClaimDraftSourceNotUtf8,
    ClaimDraftSpanNotCharBoundary(ClaimDraftId),
    NonCanonicalClaimDraftReaudit,
    ClaimDraftReauditBindingMismatch(ClaimDraftId),
    ClaimCapabilityMembraneSemanticRevisionMismatch,
    NonCanonicalClaimCapabilityMembrane,
    SupportedWithoutEvidence,
    SourceMismatch,
}

impl Display for AppealIrError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyPath => formatter.write_str("appeal IR path must not be empty"),
            Self::ReadmeGrammarSemanticRevisionMismatch => {
                formatter.write_str("README grammar semantic revision is unsupported")
            }
            Self::ClaimAtomSemanticRevisionMismatch => {
                formatter.write_str("README claim-atom semantic revision is unsupported")
            }
            Self::ReadmeTranslationSemanticRevisionMismatch => {
                formatter.write_str("README translation semantic revision is unsupported")
            }
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
            Self::NonCanonicalClaimAtoms => {
                formatter.write_str("claim atoms must be sorted and unique")
            }
            Self::DanglingClaimAtomGrammarNode(id) => write!(
                formatter,
                "claim atom {} references an unknown grammar node",
                id.get()
            ),
            Self::ClaimAtomDimensionMismatch(id) => write!(
                formatter,
                "claim atom {} dimension does not match its grammar node",
                id.get()
            ),
            Self::ClaimAtomModalityMismatch(id) => write!(
                formatter,
                "claim atom {} modality does not match its grammar node",
                id.get()
            ),
            Self::ClaimAtomLanguageMismatch(id) => write!(
                formatter,
                "claim atom {} language does not match its grammar node",
                id.get()
            ),
            Self::ClaimAtomSpanMismatch(id) => write!(
                formatter,
                "claim atom {} span does not match its grammar node",
                id.get()
            ),
            Self::ClaimAtomPolarityMismatch(id) => write!(
                formatter,
                "claim atom {} polarity does not match its grammar node",
                id.get()
            ),
            Self::NonCanonicalClaimAtomQualifiers(id) => write!(
                formatter,
                "claim atom {} qualifiers must be non-empty, sorted, and unique",
                id.get()
            ),
            Self::NonCanonicalReadmeSections => {
                formatter.write_str("README sections must be sorted and unique")
            }
            Self::ReadmeSectionSpanOutOfBounds(id) => write!(
                formatter,
                "README section {} span exceeds the bound source",
                id.get()
            ),
            Self::NonCanonicalReadmeSectionClaims(id) => write!(
                formatter,
                "README section {} claim atoms must be sorted and unique",
                id.get()
            ),
            Self::ClaimAtomInMultipleReadmeSections(id) => write!(
                formatter,
                "claim atom {} belongs to multiple README sections",
                id.get()
            ),
            Self::TranslationAlignmentPathMismatch => formatter
                .write_str("translation alignment path does not match its README grammar"),
            Self::NonCanonicalTranslationAlignments => formatter
                .write_str("translation alignments must be sorted, unique, and non-reversed"),
            Self::InvalidTranslationSectionPair => {
                formatter.write_str("translation alignment cannot reference one section twice")
            }
            Self::DanglingTranslationSection => {
                formatter.write_str("translation alignment references an unknown section")
            }
            Self::TranslationLanguagePairMismatch => formatter.write_str(
                "translation alignment requires a Japanese/English pair or an Unknown mixed/ambiguous assessment",
            ),
            Self::NonCanonicalTranslationClaimAlignments => formatter
                .write_str("translation claim alignments must be sorted and unique"),
            Self::DuplicateTranslationClaim(id) => write!(
                formatter,
                "translation claim {} participates in more than one alignment",
                id.get()
            ),
            Self::NonCanonicalTranslationDivergences => formatter
                .write_str("translation divergences must be sorted and unique"),
            Self::InvalidTranslationAlignmentState => formatter
                .write_str("translation state conflicts with its counterpart or divergences"),
            Self::TranslationClaimOutsideSection(id) => write!(
                formatter,
                "translation claim {} is outside its declared section",
                id.get()
            ),
            Self::TranslationCounterpartOnSameSide(id) => write!(
                formatter,
                "translation counterpart claim {} belongs to the same endpoint section",
                id.get()
            ),
            Self::DanglingTranslationClaimAtom(id) => write!(
                formatter,
                "translation alignment references unknown claim atom {}",
                id.get()
            ),
            Self::TranslationClaimLanguageMismatch(id) => write!(
                formatter,
                "translation claim {} language does not match its section",
                id.get()
            ),
            Self::TranslationClaimSpanOutsideSection(id) => write!(
                formatter,
                "translation claim {} span is missing or outside its section",
                id.get()
            ),
            Self::TranslationClaimDimensionMismatch(id) => write!(
                formatter,
                "translation claim {} dimension differs from its counterpart",
                id.get()
            ),
            Self::TranslationSemanticTermMismatch(id) => write!(
                formatter,
                "translation claim {} semantic terms differ from its counterpart",
                id.get()
            ),
            Self::TranslationPolarityAssessmentMismatch(id) => write!(
                formatter,
                "translation claim {} polarity divergence assessment is inconsistent",
                id.get()
            ),
            Self::TranslationQualifierAssessmentMismatch(id) => write!(
                formatter,
                "translation claim {} qualifier divergence assessment is inconsistent",
                id.get()
            ),
            Self::TranslationConditionAssessmentMismatch(id) => write!(
                formatter,
                "translation claim {} condition divergence assessment is inconsistent",
                id.get()
            ),
            Self::NonCanonicalCapabilityNodes => {
                formatter.write_str("capability nodes must be sorted and unique")
            }
            Self::NonCanonicalCapabilityDiagnostics => formatter
                .write_str("capability diagnostics must be source-bound, sorted, and unique"),
            Self::RepositoryCapabilitySemanticRevisionMismatch => formatter
                .write_str("repository capability semantic revision is unsupported"),
            Self::NonCanonicalCapabilityUnknownReasons => formatter
                .write_str("capability Unknown reasons must be complete, sorted, and unique"),
            Self::CapabilityWithoutProvenance(id) => write!(
                formatter,
                "capability node {} requires provenance",
                id.get()
            ),
            Self::DanglingCapabilityEdge => {
                formatter.write_str("capability edge references unknown node")
            }
            Self::NonCanonicalCapabilitySemanticSignatures => formatter
                .write_str("capability semantic signatures must be sorted and unique by owner"),
            Self::DanglingCapabilitySemanticSignatureOwner(id) => write!(
                formatter,
                "capability semantic signature references unknown owner {}",
                id.get()
            ),
            Self::CapabilitySemanticSignatureDimensionMismatch(id) => write!(
                formatter,
                "capability semantic signature {} dimension does not match its owner",
                id.get()
            ),
            Self::NonCanonicalCapabilitySemanticTerms(id) => write!(
                formatter,
                "capability semantic signature {} terms must be non-empty and trimmed",
                id.get()
            ),
            Self::NonCanonicalCapabilitySemanticQualifiers(id) => write!(
                formatter,
                "capability semantic signature {} qualifiers must be trimmed, sorted, and unique",
                id.get()
            ),
            Self::NonCanonicalCapabilitySemanticReferences(id) => write!(
                formatter,
                "capability semantic signature {} references must be sorted and unique",
                id.get()
            ),
            Self::DanglingCapabilitySemanticReference(id) => write!(
                formatter,
                "capability semantic signature {} contains an unknown node reference",
                id.get()
            ),
            Self::CapabilitySemanticReferenceKindMismatch(id) => write!(
                formatter,
                "capability semantic signature {} contains a reference of the wrong kind",
                id.get()
            ),
            Self::MissingCapabilitySemanticRelation(id) => write!(
                formatter,
                "capability semantic signature {} is not backed by the required capability edge",
                id.get()
            ),
            Self::CapabilitySemanticSupportExceedsOwner(id) => write!(
                formatter,
                "capability semantic signature {} support exceeds its owner",
                id.get()
            ),
            Self::SourceSyntaxSemanticSupportMustBeInferred(id) => write!(
                formatter,
                "source-syntax semantic signature {} cannot exceed inferred support",
                id.get()
            ),
            Self::EvidenceCeilingDimensionMismatch(id) => write!(
                formatter,
                "evidence ceiling contribution {} has an unsupported basis/dimension pair",
                id.get()
            ),
            Self::UnalignedEvidenceCeilingContribution(id) => write!(
                formatter,
                "claim atom {} evidence ceiling references an unaligned capability node",
                id.get()
            ),
            Self::ClaimDraftSemanticRevisionMismatch => {
                formatter.write_str("claim draft semantic revision is unsupported")
            }
            Self::InvalidClaimDraftPath => formatter.write_str(
                "claim draft path must be a bounded canonical repository-relative path",
            ),
            Self::NonCanonicalClaimDrafts => {
                formatter.write_str("claim drafts must be bounded, sorted, and unique")
            }
            Self::ClaimDraftSpanOutOfBounds(id) => write!(
                formatter,
                "claim draft {} target span exceeds the bound source",
                id.get()
            ),
            Self::NonCanonicalClaimDraftSemantics => formatter.write_str(
                "claim draft semantics must contain bounded canonical tokens without prose punctuation",
            ),
            Self::NonCanonicalClaimDraftReferences(id) => write!(
                formatter,
                "claim draft {} references must be bounded, sorted, unique, and capability-backed",
                id.get()
            ),
            Self::ClaimDraftWithoutCeiling(id) => write!(
                formatter,
                "claim draft {} requires a non-omitted evidence ceiling",
                id.get()
            ),
            Self::ClaimDraftWithoutRealization(id) => write!(
                formatter,
                "claim draft {} requires a non-omitted realization mode",
                id.get()
            ),
            Self::ClaimDraftRealizationMismatch(id) => write!(
                formatter,
                "claim draft {} realization mode does not match its semantic modality",
                id.get()
            ),
            Self::ClaimDraftCeilingExceeded(id) => write!(
                formatter,
                "claim draft {} realization mode exceeds its evidence ceiling",
                id.get()
            ),
            Self::DanglingClaimDraft(id) => write!(
                formatter,
                "claim draft re-audit references unknown draft {}",
                id.get()
            ),
            Self::ClaimDraftSourcePathMismatch => {
                formatter.write_str("claim draft path does not match the validated source")
            }
            Self::ClaimDraftSourceDigestMismatch => {
                formatter.write_str("claim draft digest does not match the validated source")
            }
            Self::ClaimDraftSourceLengthMismatch => {
                formatter.write_str("claim draft byte length does not match the validated source")
            }
            Self::ClaimDraftSourceNotUtf8 => {
                formatter.write_str("claim draft source must be valid UTF-8")
            }
            Self::ClaimDraftSpanNotCharBoundary(id) => write!(
                formatter,
                "claim draft {} target span splits a UTF-8 code point",
                id.get()
            ),
            Self::NonCanonicalClaimDraftReaudit => formatter
                .write_str("claim draft re-audit facts, holds, and decision are inconsistent"),
            Self::ClaimDraftReauditBindingMismatch(id) => write!(
                formatter,
                "claim draft re-audit {} does not match its source-bound draft IR",
                id.get()
            ),
            Self::ClaimCapabilityMembraneSemanticRevisionMismatch => formatter
                .write_str("claim-capability membrane semantic revision is unsupported"),
            Self::NonCanonicalClaimCapabilityMembrane => formatter.write_str(
                "claim-capability membrane relations and alignments must be canonical and evidence bounded",
            ),
            Self::SupportedWithoutEvidence => {
                formatter.write_str("supported relation requires capability and evidence")
            }
            Self::SourceMismatch => {
                formatter.write_str("grammar source text does not match the document scan")
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

    fn atom_id(value: u32) -> ClaimAtomId {
        ClaimAtomId::new(NonZeroU32::new(value).expect("non-zero"))
    }

    fn draft_id(value: u32) -> ClaimDraftId {
        ClaimDraftId::new(NonZeroU32::new(value).expect("non-zero"))
    }

    fn section_id(value: u32) -> ReadmeSectionId {
        ReadmeSectionId::new(NonZeroU32::new(value).expect("non-zero"))
    }

    fn claim_draft_semantics() -> ClaimDraftSemantics {
        ClaimDraftSemantics::try_new(
            ValueDimension::Capability,
            Some("reposeiri".to_string()),
            Some("audit".to_string()),
            Some("repository structure".to_string()),
            vec!["bounded".to_string(), "local".to_string()],
            None,
            ClaimPolarity::Positive,
            ClaimModality::Qualified,
            DocumentLanguage::English,
        )
        .expect("canonical draft semantics")
    }

    fn claim_draft_ir() -> ClaimDraftIR {
        let draft = ClaimDraft::try_new(
            draft_id(1),
            SourceSpan::new(1, 1, 0, 10),
            vec![atom_id(1)],
            vec![capability_id(1)],
            claim_draft_semantics(),
            ClaimMode::Qualified,
            ClaimMode::Qualified,
        )
        .expect("canonical claim draft");
        ClaimDraftIR::try_new(
            "README.md",
            PatchBaseDigest::from_bytes(b"01234567890123456789"),
            20,
            1,
            vec![draft],
        )
        .expect("source-bound claim draft IR")
    }

    fn claim_atom_fixture() -> (GrammarNode, ClaimAtom) {
        let span = SourceSpan::new(1, 1, 0, 20);
        (
            GrammarNode {
                id: grammar_id(1),
                predicate: GrammarPredicate::PerformsOperation,
                state: AnswerState::Explicit,
                language: DocumentLanguage::English,
                modality: ClaimModality::Asserted,
                negated: false,
                span: Some(span),
            },
            ClaimAtom {
                id: atom_id(1),
                grammar_node: grammar_id(1),
                dimension: ValueDimension::Capability,
                subject: Some("reposeiri".to_string()),
                action: Some("audit".to_string()),
                object: Some("repositories".to_string()),
                qualifiers: vec!["bounded".to_string(), "locally".to_string()],
                condition: None,
                polarity: ClaimPolarity::Positive,
                modality: ClaimModality::Asserted,
                language: DocumentLanguage::English,
                span: Some(span),
            },
        )
    }

    fn bilingual_claim_fixture(english_polarity: ClaimPolarity) -> (Vec<GrammarNode>, ClaimAtomIR) {
        let japanese_span = SourceSpan::new(1, 1, 0, 10);
        let english_span = SourceSpan::new(2, 1, 10, 20);
        let english_negated = english_polarity == ClaimPolarity::Negative;
        let english_modality = if english_negated {
            ClaimModality::Prohibited
        } else {
            ClaimModality::Asserted
        };
        (
            vec![
                GrammarNode {
                    id: grammar_id(1),
                    predicate: GrammarPredicate::PerformsOperation,
                    state: AnswerState::Explicit,
                    language: DocumentLanguage::Japanese,
                    modality: ClaimModality::Asserted,
                    negated: false,
                    span: Some(japanese_span),
                },
                GrammarNode {
                    id: grammar_id(2),
                    predicate: GrammarPredicate::PerformsOperation,
                    state: AnswerState::Explicit,
                    language: DocumentLanguage::English,
                    modality: english_modality,
                    negated: english_negated,
                    span: Some(english_span),
                },
            ],
            ClaimAtomIR {
                semantic_revision: README_CLAIM_ATOM_REVISION.to_string(),
                atoms: vec![
                    ClaimAtom {
                        id: atom_id(1),
                        grammar_node: grammar_id(1),
                        dimension: ValueDimension::Capability,
                        subject: Some("reposeiri".to_string()),
                        action: Some("audit".to_string()),
                        object: Some("repository".to_string()),
                        qualifiers: Vec::new(),
                        condition: None,
                        polarity: ClaimPolarity::Positive,
                        modality: ClaimModality::Asserted,
                        language: DocumentLanguage::Japanese,
                        span: Some(japanese_span),
                    },
                    ClaimAtom {
                        id: atom_id(2),
                        grammar_node: grammar_id(2),
                        dimension: ValueDimension::Capability,
                        subject: Some("reposeiri".to_string()),
                        action: Some("audit".to_string()),
                        object: Some("repository".to_string()),
                        qualifiers: Vec::new(),
                        condition: None,
                        polarity: english_polarity,
                        modality: english_modality,
                        language: DocumentLanguage::English,
                        span: Some(english_span),
                    },
                ],
            },
        )
    }

    fn bilingual_sections() -> Vec<ReadmeSection> {
        vec![
            ReadmeSection {
                id: section_id(1),
                language: ReadmeSectionLanguage::Japanese,
                span: SourceSpan::new(1, 1, 0, 10),
                claim_atoms: vec![atom_id(1)],
            },
            ReadmeSection {
                id: section_id(2),
                language: ReadmeSectionLanguage::English,
                span: SourceSpan::new(2, 1, 10, 20),
                claim_atoms: vec![atom_id(2)],
            },
        ]
    }

    fn bilingual_translation(
        state: TranslationAlignmentState,
        divergences: Vec<TranslationDivergenceKind>,
    ) -> ReadmeTranslationAlignmentIR {
        ReadmeTranslationAlignmentIR::try_new(
            "README.md",
            PatchBaseDigest::from_bytes(b"01234567890123456789"),
            20,
            bilingual_sections(),
            vec![ReadmeSectionAlignment {
                source_section: section_id(1),
                counterpart_section: section_id(2),
                state,
                claims: vec![TranslationClaimAlignment {
                    source_claim: atom_id(1),
                    counterpart_claim: Some(atom_id(2)),
                    state,
                    divergences,
                }],
            }],
            Vec::new(),
        )
        .expect("structurally valid bilingual translation")
    }

    fn source_capability(value: u32, kind: CapabilityKind, symbol: &str) -> CapabilityNode {
        CapabilityNode {
            id: capability_id(value),
            kind,
            support: CapabilitySupport::Observed,
            symbol: symbol.to_string(),
            provenance: vec![CapabilityProvenance {
                path: "src/lib.rs".to_string(),
                kind: CapabilityProvenanceKind::SourceSyntax,
                span: Some(SourceSpan::new(
                    value as usize,
                    1,
                    value as usize,
                    value as usize + 1,
                )),
            }],
        }
    }

    fn capability_semantic_fixture() -> (
        Vec<CapabilityNode>,
        Vec<CapabilityEdge>,
        CapabilitySemanticSignature,
    ) {
        (
            vec![
                source_capability(1, CapabilityKind::Operation, "audit_repository"),
                source_capability(2, CapabilityKind::Input, "repository"),
                source_capability(3, CapabilityKind::Output, "audit_report"),
                source_capability(4, CapabilityKind::Feature, "local"),
            ],
            vec![
                CapabilityEdge {
                    from: capability_id(1),
                    to: capability_id(2),
                    relation: CapabilityRelation::Accepts,
                },
                CapabilityEdge {
                    from: capability_id(1),
                    to: capability_id(3),
                    relation: CapabilityRelation::Produces,
                },
                CapabilityEdge {
                    from: capability_id(1),
                    to: capability_id(4),
                    relation: CapabilityRelation::ConditionedBy,
                },
            ],
            CapabilitySemanticSignature {
                capability_node: capability_id(1),
                dimension: ValueDimension::Capability,
                subject: Some("reposeiri".to_string()),
                action: Some("audit".to_string()),
                object: Some("repository".to_string()),
                qualifiers: vec!["bounded".to_string(), "local".to_string()],
                polarity: ClaimPolarity::Positive,
                input_nodes: vec![capability_id(2)],
                output_nodes: vec![capability_id(3)],
                condition_nodes: vec![capability_id(4)],
                semantic_support: CapabilitySupport::Inferred,
            },
        )
    }

    #[test]
    fn claim_draft_rejects_prose_tokens_and_polarity_modality_mismatch() {
        assert_eq!(
            ClaimDraftSemantics::try_new(
                ValueDimension::Capability,
                Some("RepoSeiri".to_string()),
                Some("audits!".to_string()),
                Some("repository structure".to_string()),
                Vec::new(),
                None,
                ClaimPolarity::Positive,
                ClaimModality::Asserted,
                DocumentLanguage::English,
            ),
            Err(AppealIrError::NonCanonicalClaimDraftSemantics)
        );
        assert_eq!(
            ClaimDraftSemantics::try_new(
                ValueDimension::Constraint,
                Some("reposeiri".to_string()),
                Some("write".to_string()),
                None,
                Vec::new(),
                None,
                ClaimPolarity::Negative,
                ClaimModality::Asserted,
                DocumentLanguage::English,
            ),
            Err(AppealIrError::NonCanonicalClaimDraftSemantics)
        );
    }

    #[test]
    fn claim_draft_realization_cannot_exceed_ceiling() {
        let direct_semantics = ClaimDraftSemantics::try_new(
            ValueDimension::Capability,
            Some("reposeiri".to_string()),
            Some("audit".to_string()),
            Some("repository structure".to_string()),
            vec!["bounded".to_string(), "local".to_string()],
            None,
            ClaimPolarity::Positive,
            ClaimModality::Asserted,
            DocumentLanguage::English,
        )
        .expect("canonical direct draft semantics");
        assert_eq!(
            ClaimDraft::try_new(
                draft_id(1),
                SourceSpan::new(1, 1, 0, 10),
                vec![atom_id(1)],
                vec![capability_id(1)],
                direct_semantics,
                ClaimMode::Direct,
                ClaimMode::Qualified,
            ),
            Err(AppealIrError::ClaimDraftCeilingExceeded(draft_id(1)))
        );
        assert_eq!(
            ClaimDraft::try_new(
                draft_id(1),
                SourceSpan::new(1, 1, 0, 10),
                vec![atom_id(1)],
                vec![capability_id(1)],
                claim_draft_semantics(),
                ClaimMode::Direct,
                ClaimMode::Direct,
            ),
            Err(AppealIrError::ClaimDraftRealizationMismatch(draft_id(1)))
        );
    }

    #[test]
    fn claim_draft_ir_rejects_scope_paths_and_unbounded_spans() {
        let ir = claim_draft_ir();
        assert_eq!(ir.draft(draft_id(1)), ir.drafts.first());
        assert!(!ir.is_empty());
        assert!(ClaimDraftIR::default().is_empty());

        assert_eq!(
            ClaimDraftIR::try_new(
                "../README.md",
                ir.source_digest,
                ir.source_byte_len,
                ir.baseline_unknown_count,
                ir.drafts.clone(),
            ),
            Err(AppealIrError::InvalidClaimDraftPath)
        );

        let mut outside = ir.drafts[0].clone();
        outside.target_span = SourceSpan::new(1, 1, 0, 21);
        assert_eq!(
            ClaimDraftIR::try_new("README.md", ir.source_digest, 20, 1, vec![outside],),
            Err(AppealIrError::ClaimDraftSpanOutOfBounds(draft_id(1)))
        );
    }

    #[test]
    fn claim_draft_reaudit_stale_source_dominates_candidate_checks() {
        let ir = claim_draft_ir();
        let result = ClaimDraftReauditResult::evaluate(
            &ir,
            draft_id(1),
            ClaimDraftReauditObservation::new(
                PatchBaseDigest::from_bytes(b"stale-source-0000000"),
                ir.source_byte_len,
                "../README.md",
                PatchBaseDigest::from_bytes(b"candidate"),
                9,
                ir.baseline_unknown_count + 10,
                None,
                ClaimMode::Direct,
                false,
            ),
        )
        .expect("stale source is a typed hold");

        assert_eq!(result.decision, ClaimDraftReauditDecision::Held);
        assert_eq!(
            result.holds,
            vec![ClaimDraftReauditHoldReason::StaleSourceDigest]
        );
        assert!(!result.candidate_checks_performed);
    }

    #[test]
    fn claim_draft_reaudit_emits_all_candidate_holds_in_canonical_order() {
        let ir = claim_draft_ir();
        let result = ClaimDraftReauditResult::evaluate(
            &ir,
            draft_id(1),
            ClaimDraftReauditObservation::new(
                ir.source_digest,
                ir.source_byte_len,
                "docs/README.md",
                PatchBaseDigest::from_bytes(b"candidate"),
                9,
                ir.baseline_unknown_count + 1,
                None,
                ClaimMode::Direct,
                false,
            ),
        )
        .expect("candidate failures become typed holds");

        assert_eq!(
            result.holds,
            vec![
                ClaimDraftReauditHoldReason::UnknownIncreased,
                ClaimDraftReauditHoldReason::CeilingExceeded,
                ClaimDraftReauditHoldReason::SemanticDrift,
                ClaimDraftReauditHoldReason::ScopeEscape,
            ]
        );
        assert!(result.candidate_checks_performed);
    }

    #[test]
    fn accepted_claim_draft_reaudit_contains_no_write_and_omitted_mode_holds() {
        fn assert_serde<T: Serialize + for<'de> Deserialize<'de>>() {}
        assert_serde::<ClaimDraftIR>();
        assert_serde::<ClaimDraftReauditResult>();

        let ir = claim_draft_ir();
        let observation = |candidate_mode| {
            ClaimDraftReauditObservation::new(
                ir.source_digest,
                ir.source_byte_len,
                "README.md",
                PatchBaseDigest::from_bytes(b"candidate"),
                9,
                ir.baseline_unknown_count,
                Some(claim_draft_semantics()),
                candidate_mode,
                true,
            )
        };
        let accepted =
            ClaimDraftReauditResult::evaluate(&ir, draft_id(1), observation(ClaimMode::Qualified))
                .expect("equivalent candidate");
        assert_eq!(accepted.decision, ClaimDraftReauditDecision::Accepted);
        assert!(accepted.holds.is_empty());
        assert!(!accepted.writes_files());

        let omitted =
            ClaimDraftReauditResult::evaluate(&ir, draft_id(1), observation(ClaimMode::Omitted))
                .expect("omitted candidate remains reviewable");
        assert_eq!(
            omitted.holds,
            vec![ClaimDraftReauditHoldReason::SemanticDrift]
        );
    }

    #[test]
    fn claim_draft_source_validation_rejects_revision_path_digest_and_length_tamper() {
        let source = b"01234567890123456789";
        let ir = claim_draft_ir();
        assert_eq!(ir.validate_against_source("README.md", source), Ok(()));

        let mut revision = ir.clone();
        revision.semantic_revision = "seiri.claim-draft.tampered".to_string();
        assert_eq!(
            revision.validate_against_source("README.md", source),
            Err(AppealIrError::ClaimDraftSemanticRevisionMismatch)
        );
        assert_eq!(
            ir.validate_against_source("docs/README.md", source),
            Err(AppealIrError::ClaimDraftSourcePathMismatch)
        );
        assert_eq!(
            ir.validate_against_source("README.md", b"01234567890123456788"),
            Err(AppealIrError::ClaimDraftSourceDigestMismatch)
        );
        assert_eq!(
            ir.validate_against_source("README.md", b"short"),
            Err(AppealIrError::ClaimDraftSourceLengthMismatch)
        );
    }

    #[test]
    fn claim_draft_source_validation_rejects_invalid_utf8_and_split_codepoint_spans() {
        let split = ClaimDraft::try_new(
            draft_id(1),
            SourceSpan::new(1, 2, 1, 2),
            vec![atom_id(1)],
            vec![capability_id(1)],
            claim_draft_semantics(),
            ClaimMode::Qualified,
            ClaimMode::Qualified,
        )
        .expect("byte-bounded draft");
        let source = "é";
        let ir = ClaimDraftIR::try_new(
            "README.md",
            PatchBaseDigest::from_bytes(source.as_bytes()),
            source.len(),
            0,
            vec![split],
        )
        .expect("source-bound IR");
        assert_eq!(
            ir.validate_against_source("README.md", source),
            Err(AppealIrError::ClaimDraftSpanNotCharBoundary(draft_id(1)))
        );

        let invalid = [0xff];
        let invalid_ir = ClaimDraftIR::try_new(
            "README.md",
            PatchBaseDigest::from_bytes(&invalid),
            invalid.len(),
            0,
            vec![ClaimDraft::try_new(
                draft_id(1),
                SourceSpan::new(1, 1, 0, 1),
                Vec::new(),
                vec![capability_id(1)],
                claim_draft_semantics(),
                ClaimMode::Qualified,
                ClaimMode::Qualified,
            )
            .expect("byte-bounded draft")],
        )
        .expect("source-bound IR");
        assert_eq!(
            invalid_ir.validate_against_source("README.md", invalid),
            Err(AppealIrError::ClaimDraftSourceNotUtf8)
        );
    }

    #[test]
    fn public_claim_draft_validation_rejects_direct_field_tamper() {
        let ir = claim_draft_ir();
        let mut semantics = ir.drafts[0].semantics.clone();
        semantics.action = Some("Audit".to_string());
        assert_eq!(
            semantics.validate(),
            Err(AppealIrError::NonCanonicalClaimDraftSemantics)
        );

        let mut duplicate_reference = ir.drafts[0].clone();
        duplicate_reference.capability_nodes.push(capability_id(1));
        assert_eq!(
            duplicate_reference.validate(),
            Err(AppealIrError::NonCanonicalClaimDraftReferences(draft_id(1)))
        );

        let mut unbounded = ir;
        unbounded.baseline_unknown_count = CLAIM_DRAFT_MAX_UNKNOWN_COUNT + 1;
        assert_eq!(
            unbounded.validate(),
            Err(AppealIrError::NonCanonicalClaimDrafts)
        );
    }

    #[test]
    fn public_appeal_ir_validators_reject_semantic_revision_tamper() {
        let mut grammar = ReadmeGrammarIR::default();
        assert_eq!(grammar.validate(), Ok(()));
        grammar.semantic_revision = "tampered.grammar".to_string();
        assert_eq!(
            grammar.validate(),
            Err(AppealIrError::ReadmeGrammarSemanticRevisionMismatch)
        );

        let mut translation = ReadmeTranslationAlignmentIR::default();
        assert_eq!(translation.validate(), Ok(()));
        translation.semantic_revision = "tampered.translation".to_string();
        assert_eq!(
            translation.validate(),
            Err(AppealIrError::ReadmeTranslationSemanticRevisionMismatch)
        );

        let mut capabilities = RepositoryCapabilityIR::default();
        assert_eq!(capabilities.validate(), Ok(()));
        capabilities.semantic_revision = "tampered.capability".to_string();
        assert_eq!(
            capabilities.validate(),
            Err(AppealIrError::RepositoryCapabilitySemanticRevisionMismatch)
        );

        let mut membrane = ClaimCapabilityMembrane::default();
        assert_eq!(membrane.validate(), Ok(()));
        membrane.semantic_revision = "tampered.membrane".to_string();
        assert_eq!(
            membrane.validate(),
            Err(AppealIrError::ClaimCapabilityMembraneSemanticRevisionMismatch)
        );
    }

    #[test]
    fn claim_draft_reaudit_validation_rejects_fact_and_binding_tamper() {
        let ir = claim_draft_ir();
        let observation = ClaimDraftReauditObservation::new(
            ir.source_digest,
            ir.source_byte_len,
            "README.md",
            PatchBaseDigest::from_bytes(b"candidate"),
            9,
            ir.baseline_unknown_count,
            Some(claim_draft_semantics()),
            ClaimMode::Qualified,
            true,
        );
        let accepted = ClaimDraftReauditResult::evaluate(&ir, draft_id(1), observation)
            .expect("canonical accepted result");
        assert_eq!(accepted.validate_against_drafts(&ir), Ok(()));

        let mut forged_decision = accepted.clone();
        forged_decision.decision = ClaimDraftReauditDecision::Held;
        assert_eq!(
            forged_decision.validate(),
            Err(AppealIrError::NonCanonicalClaimDraftReaudit)
        );

        let mut forged_binding = accepted;
        forged_binding.claim_ceiling = ClaimMode::Direct;
        assert_eq!(
            forged_binding.validate_against_drafts(&ir),
            Err(AppealIrError::ClaimDraftReauditBindingMismatch(draft_id(1)))
        );
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
    fn claim_atom_constructor_enforces_node_alignment_and_canonical_qualifiers() {
        let (node, atom) = claim_atom_fixture();
        let valid = ReadmeGrammarIR::try_new_with_claim_atoms(
            "README.md",
            CoverageStatus::Complete,
            vec![node.clone()],
            Vec::new(),
            Vec::new(),
            ClaimAtomIR {
                semantic_revision: README_CLAIM_ATOM_REVISION.to_string(),
                atoms: vec![atom.clone()],
            },
        );
        assert!(valid.is_ok());

        let mut mismatched = atom.clone();
        mismatched.polarity = ClaimPolarity::Negative;
        assert_eq!(
            ReadmeGrammarIR::try_new_with_claim_atoms(
                "README.md",
                CoverageStatus::Complete,
                vec![node.clone()],
                Vec::new(),
                Vec::new(),
                ClaimAtomIR {
                    semantic_revision: README_CLAIM_ATOM_REVISION.to_string(),
                    atoms: vec![mismatched],
                },
            ),
            Err(AppealIrError::ClaimAtomPolarityMismatch(atom_id(1)))
        );

        let mut noncanonical = atom.clone();
        noncanonical.qualifiers.reverse();
        assert_eq!(
            ReadmeGrammarIR::try_new_with_claim_atoms(
                "README.md",
                CoverageStatus::Complete,
                vec![node.clone()],
                Vec::new(),
                Vec::new(),
                ClaimAtomIR {
                    semantic_revision: README_CLAIM_ATOM_REVISION.to_string(),
                    atoms: vec![noncanonical],
                },
            ),
            Err(AppealIrError::NonCanonicalClaimAtomQualifiers(atom_id(1)))
        );

        let mut dangling = atom;
        dangling.grammar_node = grammar_id(2);
        assert_eq!(
            ReadmeGrammarIR::try_new_with_claim_atoms(
                "README.md",
                CoverageStatus::Complete,
                vec![node],
                Vec::new(),
                Vec::new(),
                ClaimAtomIR {
                    semantic_revision: README_CLAIM_ATOM_REVISION.to_string(),
                    atoms: vec![dangling],
                },
            ),
            Err(AppealIrError::DanglingClaimAtomGrammarNode(atom_id(1)))
        );
    }

    #[test]
    fn bilingual_section_alignment_binds_claims_path_span_and_digest() {
        let (nodes, claim_atoms) = bilingual_claim_fixture(ClaimPolarity::Positive);
        let digest = PatchBaseDigest::from_bytes(b"01234567890123456789");
        let translation = ReadmeTranslationAlignmentIR::try_new(
            "README.md",
            digest,
            20,
            bilingual_sections(),
            vec![ReadmeSectionAlignment {
                source_section: section_id(1),
                counterpart_section: section_id(2),
                state: TranslationAlignmentState::Aligned,
                claims: vec![TranslationClaimAlignment {
                    source_claim: atom_id(1),
                    counterpart_claim: Some(atom_id(2)),
                    state: TranslationAlignmentState::Aligned,
                    divergences: Vec::new(),
                }],
            }],
            Vec::new(),
        )
        .expect("bounded bilingual alignment");
        let grammar = ReadmeGrammarIR::try_new_with_claim_atoms_and_translation_alignment(
            "README.md",
            CoverageStatus::Complete,
            nodes,
            Vec::new(),
            Vec::new(),
            claim_atoms,
            translation,
        )
        .expect("translation-bound grammar");

        assert_eq!(grammar.translation_alignment.source_digest, digest);
        assert_eq!(grammar.translation_alignment.source_byte_len, 20);
        assert_eq!(
            grammar.translation_alignment.sections[0].language,
            ReadmeSectionLanguage::Japanese
        );
    }

    #[test]
    fn polarity_divergence_is_typed_and_checked_against_claim_atoms() {
        let (nodes, claim_atoms) = bilingual_claim_fixture(ClaimPolarity::Negative);
        let translation = ReadmeTranslationAlignmentIR::try_new(
            "README.md",
            PatchBaseDigest::from_bytes(b"01234567890123456789"),
            20,
            bilingual_sections(),
            vec![ReadmeSectionAlignment {
                source_section: section_id(1),
                counterpart_section: section_id(2),
                state: TranslationAlignmentState::Divergent,
                claims: vec![TranslationClaimAlignment {
                    source_claim: atom_id(1),
                    counterpart_claim: Some(atom_id(2)),
                    state: TranslationAlignmentState::Divergent,
                    divergences: vec![TranslationDivergenceKind::Polarity],
                }],
            }],
            Vec::new(),
        )
        .expect("typed polarity divergence");
        assert!(
            ReadmeGrammarIR::try_new_with_claim_atoms_and_translation_alignment(
                "README.md",
                CoverageStatus::Complete,
                nodes,
                Vec::new(),
                Vec::new(),
                claim_atoms,
                translation,
            )
            .is_ok()
        );
    }

    #[test]
    fn translation_pair_requires_matching_semantic_terms() {
        let (nodes, mut claim_atoms) = bilingual_claim_fixture(ClaimPolarity::Positive);
        claim_atoms.atoms[1].object = Some("cloud".to_string());

        assert_eq!(
            ReadmeGrammarIR::try_new_with_claim_atoms_and_translation_alignment(
                "README.md",
                CoverageStatus::Complete,
                nodes,
                Vec::new(),
                Vec::new(),
                claim_atoms,
                bilingual_translation(TranslationAlignmentState::Aligned, Vec::new()),
            ),
            Err(AppealIrError::TranslationSemanticTermMismatch(atom_id(1)))
        );
    }

    #[test]
    fn translation_qualifier_and_condition_markers_are_exact() {
        let (nodes, mut claim_atoms) = bilingual_claim_fixture(ClaimPolarity::Positive);
        claim_atoms.atoms[1].qualifiers = vec!["local".to_string()];
        assert_eq!(
            ReadmeGrammarIR::try_new_with_claim_atoms_and_translation_alignment(
                "README.md",
                CoverageStatus::Complete,
                nodes.clone(),
                Vec::new(),
                Vec::new(),
                claim_atoms,
                bilingual_translation(TranslationAlignmentState::Aligned, Vec::new()),
            ),
            Err(AppealIrError::TranslationQualifierAssessmentMismatch(
                atom_id(1)
            ))
        );

        let (_, mut claim_atoms) = bilingual_claim_fixture(ClaimPolarity::Positive);
        claim_atoms.atoms[1].condition = Some("offline".to_string());
        assert_eq!(
            ReadmeGrammarIR::try_new_with_claim_atoms_and_translation_alignment(
                "README.md",
                CoverageStatus::Complete,
                nodes.clone(),
                Vec::new(),
                Vec::new(),
                claim_atoms,
                bilingual_translation(TranslationAlignmentState::Aligned, Vec::new()),
            ),
            Err(AppealIrError::TranslationConditionAssessmentMismatch(
                atom_id(1)
            ))
        );

        let (_, claim_atoms) = bilingual_claim_fixture(ClaimPolarity::Positive);
        assert_eq!(
            ReadmeGrammarIR::try_new_with_claim_atoms_and_translation_alignment(
                "README.md",
                CoverageStatus::Complete,
                nodes,
                Vec::new(),
                Vec::new(),
                claim_atoms,
                bilingual_translation(
                    TranslationAlignmentState::Divergent,
                    vec![TranslationDivergenceKind::Qualifier],
                ),
            ),
            Err(AppealIrError::TranslationQualifierAssessmentMismatch(
                atom_id(1)
            ))
        );
    }

    #[test]
    fn mixed_and_ambiguous_sections_remain_unknown() {
        let translation = ReadmeTranslationAlignmentIR::try_new(
            "README.md",
            PatchBaseDigest::from_bytes(b"01234567890123456789"),
            20,
            vec![
                ReadmeSection {
                    id: section_id(1),
                    language: ReadmeSectionLanguage::Mixed,
                    span: SourceSpan::new(1, 1, 0, 10),
                    claim_atoms: Vec::new(),
                },
                ReadmeSection {
                    id: section_id(2),
                    language: ReadmeSectionLanguage::Ambiguous(UnknownReason::ParseFailed),
                    span: SourceSpan::new(2, 1, 10, 20),
                    claim_atoms: Vec::new(),
                },
            ],
            vec![ReadmeSectionAlignment {
                source_section: section_id(1),
                counterpart_section: section_id(2),
                state: TranslationAlignmentState::Unknown(UnknownReason::UnsupportedSyntax),
                claims: Vec::new(),
            }],
            Vec::new(),
        )
        .expect("unknown mixed/ambiguous alignment");

        assert_eq!(
            translation.unknown_reasons,
            vec![UnknownReason::ParseFailed, UnknownReason::UnsupportedSyntax]
        );
    }

    #[test]
    fn translation_alignment_rejects_unbounded_and_noncanonical_input() {
        let mut sections = bilingual_sections();
        sections[1].span = SourceSpan::new(2, 1, 10, 21);
        assert_eq!(
            ReadmeTranslationAlignmentIR::try_new(
                "README.md",
                PatchBaseDigest::from_bytes(b"01234567890123456789"),
                20,
                sections,
                Vec::new(),
                Vec::new(),
            ),
            Err(AppealIrError::ReadmeSectionSpanOutOfBounds(section_id(2)))
        );

        let mut sections = bilingual_sections();
        sections.reverse();
        assert_eq!(
            ReadmeTranslationAlignmentIR::try_new(
                "README.md",
                PatchBaseDigest::from_bytes(b"01234567890123456789"),
                20,
                sections,
                Vec::new(),
                Vec::new(),
            ),
            Err(AppealIrError::NonCanonicalReadmeSections)
        );

        let legacy = ReadmeGrammarIR::try_new(
            "README.md",
            CoverageStatus::Complete,
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .expect("compatible legacy constructor");
        assert!(legacy.translation_alignment.is_empty());
    }

    #[test]
    fn translation_claim_can_participate_in_only_one_section_alignment() {
        let sections = vec![
            ReadmeSection {
                id: section_id(1),
                language: ReadmeSectionLanguage::Japanese,
                span: SourceSpan::new(1, 1, 0, 10),
                claim_atoms: vec![atom_id(1)],
            },
            ReadmeSection {
                id: section_id(2),
                language: ReadmeSectionLanguage::English,
                span: SourceSpan::new(2, 1, 10, 20),
                claim_atoms: vec![atom_id(2)],
            },
            ReadmeSection {
                id: section_id(3),
                language: ReadmeSectionLanguage::English,
                span: SourceSpan::new(3, 1, 20, 30),
                claim_atoms: vec![atom_id(3)],
            },
        ];
        let align = |counterpart_section, counterpart_claim| ReadmeSectionAlignment {
            source_section: section_id(1),
            counterpart_section,
            state: TranslationAlignmentState::Aligned,
            claims: vec![TranslationClaimAlignment {
                source_claim: atom_id(1),
                counterpart_claim: Some(counterpart_claim),
                state: TranslationAlignmentState::Aligned,
                divergences: Vec::new(),
            }],
        };

        assert_eq!(
            ReadmeTranslationAlignmentIR::try_new(
                "README.md",
                PatchBaseDigest::from_bytes(b"012345678901234567890123456789"),
                30,
                sections,
                vec![
                    align(section_id(2), atom_id(2)),
                    align(section_id(3), atom_id(3)),
                ],
                Vec::new(),
            ),
            Err(AppealIrError::DuplicateTranslationClaim(atom_id(1)))
        );
    }

    #[test]
    fn one_section_alignment_reports_missing_claims_from_both_languages() {
        let sections = vec![
            ReadmeSection {
                id: section_id(1),
                language: ReadmeSectionLanguage::Japanese,
                span: SourceSpan::new(1, 1, 0, 10),
                claim_atoms: vec![atom_id(1), atom_id(3)],
            },
            ReadmeSection {
                id: section_id(2),
                language: ReadmeSectionLanguage::English,
                span: SourceSpan::new(2, 1, 10, 20),
                claim_atoms: vec![atom_id(2), atom_id(4)],
            },
        ];
        let missing = |source_claim| TranslationClaimAlignment {
            source_claim,
            counterpart_claim: None,
            state: TranslationAlignmentState::Divergent,
            divergences: vec![TranslationDivergenceKind::MissingCounterpart],
        };
        let translation = ReadmeTranslationAlignmentIR::try_new(
            "README.md",
            PatchBaseDigest::from_bytes(b"01234567890123456789"),
            20,
            sections,
            vec![ReadmeSectionAlignment {
                source_section: section_id(1),
                counterpart_section: section_id(2),
                state: TranslationAlignmentState::Divergent,
                claims: vec![
                    TranslationClaimAlignment {
                        source_claim: atom_id(1),
                        counterpart_claim: Some(atom_id(2)),
                        state: TranslationAlignmentState::Aligned,
                        divergences: Vec::new(),
                    },
                    missing(atom_id(3)),
                    missing(atom_id(4)),
                ],
            }],
            Vec::new(),
        )
        .expect("bidirectional missing-claim alignment");

        assert_eq!(translation.alignments[0].claims.len(), 3);
        assert_eq!(translation.alignments[0].claims[2].source_claim, atom_id(4));
    }

    #[test]
    fn translation_counterpart_must_belong_to_opposite_section() {
        let sections = vec![
            ReadmeSection {
                id: section_id(1),
                language: ReadmeSectionLanguage::Japanese,
                span: SourceSpan::new(1, 1, 0, 10),
                claim_atoms: vec![atom_id(1), atom_id(3)],
            },
            ReadmeSection {
                id: section_id(2),
                language: ReadmeSectionLanguage::English,
                span: SourceSpan::new(2, 1, 10, 20),
                claim_atoms: vec![atom_id(2)],
            },
        ];

        assert_eq!(
            ReadmeTranslationAlignmentIR::try_new(
                "README.md",
                PatchBaseDigest::from_bytes(b"01234567890123456789"),
                20,
                sections,
                vec![ReadmeSectionAlignment {
                    source_section: section_id(1),
                    counterpart_section: section_id(2),
                    state: TranslationAlignmentState::Aligned,
                    claims: vec![TranslationClaimAlignment {
                        source_claim: atom_id(1),
                        counterpart_claim: Some(atom_id(3)),
                        state: TranslationAlignmentState::Aligned,
                        divergences: Vec::new(),
                    }],
                }],
                Vec::new(),
            ),
            Err(AppealIrError::TranslationCounterpartOnSameSide(atom_id(3)))
        );
    }

    #[test]
    fn translation_claim_span_must_be_contained_by_its_section() {
        let (nodes, claim_atoms) = bilingual_claim_fixture(ClaimPolarity::Positive);
        let mut sections = bilingual_sections();
        sections[0].span = SourceSpan::new(1, 2, 1, 10);
        let translation = ReadmeTranslationAlignmentIR::try_new(
            "README.md",
            PatchBaseDigest::from_bytes(b"01234567890123456789"),
            20,
            sections,
            vec![ReadmeSectionAlignment {
                source_section: section_id(1),
                counterpart_section: section_id(2),
                state: TranslationAlignmentState::Aligned,
                claims: vec![TranslationClaimAlignment {
                    source_claim: atom_id(1),
                    counterpart_claim: Some(atom_id(2)),
                    state: TranslationAlignmentState::Aligned,
                    divergences: Vec::new(),
                }],
            }],
            Vec::new(),
        )
        .expect("structurally bounded translation");

        assert_eq!(
            ReadmeGrammarIR::try_new_with_claim_atoms_and_translation_alignment(
                "README.md",
                CoverageStatus::Complete,
                nodes,
                Vec::new(),
                Vec::new(),
                claim_atoms,
                translation,
            ),
            Err(AppealIrError::TranslationClaimSpanOutsideSection(atom_id(
                1
            )))
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
    fn capability_diagnostics_are_source_bound_and_canonical() {
        let diagnostic = CapabilityDiagnostic {
            path: "src/generated.rs".to_string(),
            span: Some(SourceSpan::new(3, 1, 20, 28)),
            reason: UnknownReason::UnsupportedSyntax,
        };
        let result = RepositoryCapabilityIR::try_new_with_diagnostics(
            CoverageStatus::Partial(crate::CoverageIncompleteReason::UnsupportedSyntax),
            Vec::new(),
            Vec::new(),
            vec![diagnostic.clone()],
            vec![UnknownReason::UnsupportedSyntax],
        )
        .expect("canonical diagnostic");
        assert_eq!(result.diagnostics, vec![diagnostic.clone()]);

        assert_eq!(
            RepositoryCapabilityIR::try_new_with_diagnostics(
                CoverageStatus::Partial(crate::CoverageIncompleteReason::UnsupportedSyntax),
                Vec::new(),
                Vec::new(),
                vec![diagnostic.clone(), diagnostic],
                vec![UnknownReason::UnsupportedSyntax],
            ),
            Err(AppealIrError::NonCanonicalCapabilityDiagnostics)
        );
    }

    #[test]
    fn capability_semantic_signature_is_bounded_and_relation_backed() {
        let (nodes, edges, signature) = capability_semantic_fixture();
        let result = RepositoryCapabilityIR::try_new_with_semantic_signatures_and_diagnostics(
            CoverageStatus::Complete,
            nodes,
            edges,
            vec![signature.clone()],
            Vec::new(),
            Vec::new(),
        )
        .expect("canonical source-derived semantic signature");

        assert_eq!(result.semantic_revision, REPOSITORY_CAPABILITY_REVISION);
        assert_eq!(result.semantic_signatures, vec![signature]);
    }

    #[test]
    fn source_syntax_semantics_cannot_claim_observed_support() {
        let (nodes, edges, mut signature) = capability_semantic_fixture();
        signature.semantic_support = CapabilitySupport::Observed;

        assert_eq!(
            RepositoryCapabilityIR::try_new_with_semantic_signatures_and_diagnostics(
                CoverageStatus::Complete,
                nodes,
                edges,
                vec![signature],
                Vec::new(),
                Vec::new(),
            ),
            Err(AppealIrError::SourceSyntaxSemanticSupportMustBeInferred(
                capability_id(1)
            ))
        );
    }

    #[test]
    fn capability_semantic_signature_validates_owner_refs_kinds_and_relations() {
        let (nodes, edges, signature) = capability_semantic_fixture();

        let mut dangling_owner = signature.clone();
        dangling_owner.capability_node = capability_id(9);
        assert_eq!(
            RepositoryCapabilityIR::try_new_with_semantic_signatures_and_diagnostics(
                CoverageStatus::Complete,
                nodes.clone(),
                edges.clone(),
                vec![dangling_owner],
                Vec::new(),
                Vec::new(),
            ),
            Err(AppealIrError::DanglingCapabilitySemanticSignatureOwner(
                capability_id(9)
            ))
        );

        let mut dangling_reference = signature.clone();
        dangling_reference.input_nodes = vec![capability_id(9)];
        assert_eq!(
            RepositoryCapabilityIR::try_new_with_semantic_signatures_and_diagnostics(
                CoverageStatus::Complete,
                nodes.clone(),
                edges.clone(),
                vec![dangling_reference],
                Vec::new(),
                Vec::new(),
            ),
            Err(AppealIrError::DanglingCapabilitySemanticReference(
                capability_id(1)
            ))
        );

        let mut wrong_kind = signature.clone();
        wrong_kind.output_nodes = vec![capability_id(2)];
        assert_eq!(
            RepositoryCapabilityIR::try_new_with_semantic_signatures_and_diagnostics(
                CoverageStatus::Complete,
                nodes.clone(),
                edges.clone(),
                vec![wrong_kind],
                Vec::new(),
                Vec::new(),
            ),
            Err(AppealIrError::CapabilitySemanticReferenceKindMismatch(
                capability_id(1)
            ))
        );

        let edges_without_output = edges
            .into_iter()
            .filter(|edge| edge.relation != CapabilityRelation::Produces)
            .collect();
        assert_eq!(
            RepositoryCapabilityIR::try_new_with_semantic_signatures_and_diagnostics(
                CoverageStatus::Complete,
                nodes,
                edges_without_output,
                vec![signature],
                Vec::new(),
                Vec::new(),
            ),
            Err(AppealIrError::MissingCapabilitySemanticRelation(
                capability_id(1)
            ))
        );
    }

    #[test]
    fn capability_semantic_signature_requires_canonical_collections() {
        let (nodes, edges, signature) = capability_semantic_fixture();
        let mut duplicate_reference = signature.clone();
        duplicate_reference.input_nodes = vec![capability_id(2), capability_id(2)];
        assert_eq!(
            RepositoryCapabilityIR::try_new_with_semantic_signatures_and_diagnostics(
                CoverageStatus::Complete,
                nodes.clone(),
                edges.clone(),
                vec![duplicate_reference],
                Vec::new(),
                Vec::new(),
            ),
            Err(AppealIrError::NonCanonicalCapabilitySemanticReferences(
                capability_id(1)
            ))
        );

        assert_eq!(
            RepositoryCapabilityIR::try_new_with_semantic_signatures_and_diagnostics(
                CoverageStatus::Complete,
                nodes,
                edges,
                vec![signature.clone(), signature],
                Vec::new(),
                Vec::new(),
            ),
            Err(AppealIrError::NonCanonicalCapabilitySemanticSignatures)
        );
    }

    #[test]
    fn semantic_support_never_exceeds_inferred_owner() {
        let (mut nodes, edges, mut signature) = capability_semantic_fixture();
        nodes[0].support = CapabilitySupport::Inferred;
        nodes[0].provenance[0].kind = CapabilityProvenanceKind::Documentation;
        signature.semantic_support = CapabilitySupport::Observed;

        assert_eq!(
            RepositoryCapabilityIR::try_new_with_semantic_signatures_and_diagnostics(
                CoverageStatus::Complete,
                nodes,
                edges,
                vec![signature],
                Vec::new(),
                Vec::new(),
            ),
            Err(AppealIrError::CapabilitySemanticSupportExceedsOwner(
                capability_id(1)
            ))
        );
    }

    #[test]
    fn evidence_ceiling_special_dimensions_are_explicit() {
        let cases = [
            (
                CapabilityKind::FirstResult,
                ValueDimension::FirstResult,
                ClaimMode::Qualified,
            ),
            (
                CapabilityKind::Example,
                ValueDimension::FirstAction,
                ClaimMode::Qualified,
            ),
            (
                CapabilityKind::Example,
                ValueDimension::Evidence,
                ClaimMode::Generic,
            ),
            (
                CapabilityKind::Constraint,
                ValueDimension::Constraint,
                ClaimMode::Qualified,
            ),
            (
                CapabilityKind::ComparisonReceipt,
                ValueDimension::Differentiation,
                ClaimMode::Qualified,
            ),
        ];
        for (kind, dimension, expected) in cases {
            let owner = source_capability(1, kind, "evidence");
            let contribution = EvidenceCeilingContribution::try_new(
                &owner,
                dimension,
                CapabilitySupport::Observed,
            )
            .expect("explicit ceiling mapping");
            assert_eq!(contribution.dimension(), dimension);
            assert_eq!(contribution.maximum(), expected);
        }

        assert_eq!(
            CapabilityKind::Example.dimension(),
            ValueDimension::FirstAction
        );
        assert_eq!(
            CapabilityKind::ComparisonReceipt.dimension(),
            ValueDimension::Differentiation
        );
        assert_eq!(
            REPOSITORY_CAPABILITY_REVISION,
            "seiri.repository-capability.v2"
        );

        let output = source_capability(1, CapabilityKind::Output, "report");
        assert_eq!(
            EvidenceCeilingContribution::try_new(
                &output,
                ValueDimension::FirstResult,
                CapabilitySupport::Observed,
            ),
            Err(AppealIrError::EvidenceCeilingDimensionMismatch(
                capability_id(1)
            ))
        );
    }

    #[test]
    fn evidence_ceiling_preserves_unknown_and_caps_inferred() {
        let owner = source_capability(1, CapabilityKind::Operation, "audit");
        let observed = EvidenceCeilingContribution::try_new(
            &owner,
            ValueDimension::Capability,
            CapabilitySupport::Observed,
        )
        .expect("observed contribution");
        let inferred = EvidenceCeilingContribution::try_new(
            &owner,
            ValueDimension::Capability,
            CapabilitySupport::Inferred,
        )
        .expect("inferred contribution");
        let unknown = EvidenceCeilingContribution::try_new(
            &owner,
            ValueDimension::Capability,
            CapabilitySupport::Unknown(UnknownReason::UnsupportedSyntax),
        )
        .expect("unknown contribution");

        assert_eq!(observed.maximum(), ClaimMode::Direct);
        assert_eq!(inferred.maximum(), ClaimMode::Qualified);
        assert_eq!(unknown.maximum(), ClaimMode::Omitted);
        assert_eq!(
            unknown.support(),
            CapabilitySupport::Unknown(UnknownReason::UnsupportedSyntax)
        );
    }

    #[test]
    fn alignment_ceiling_is_claim_specific_and_fail_closed() {
        let owner = source_capability(1, CapabilityKind::Operation, "audit");
        let inferred = EvidenceCeilingContribution::try_new(
            &owner,
            ValueDimension::Capability,
            CapabilitySupport::Inferred,
        )
        .expect("inferred contribution");
        let aligned = ClaimCapabilityAlignment::try_new_with_evidence_ceiling(
            atom_id(1),
            vec![capability_id(1)],
            1,
            SupportState::Supported,
            &[inferred],
        )
        .expect("aligned contribution");
        assert_eq!(aligned.claim_ceiling, ClaimMode::Qualified);

        let legacy = ClaimCapabilityAlignment::try_new(
            atom_id(1),
            vec![capability_id(1)],
            1,
            SupportState::Supported,
        )
        .expect("legacy alignment");
        assert_eq!(legacy.claim_ceiling, ClaimMode::Omitted);

        assert_eq!(
            ClaimCapabilityAlignment::try_new_with_evidence_ceiling(
                atom_id(1),
                vec![capability_id(2)],
                1,
                SupportState::Supported,
                &[inferred],
            ),
            Err(AppealIrError::UnalignedEvidenceCeilingContribution(
                atom_id(1)
            ))
        );
    }

    #[test]
    fn claim_realization_retains_negative_constraint_and_unknown_state() {
        let (mut node, mut atom) = claim_atom_fixture();
        node.predicate = GrammarPredicate::StatesConstraint;
        node.modality = ClaimModality::Prohibited;
        node.negated = true;
        atom.dimension = ValueDimension::Constraint;
        atom.polarity = ClaimPolarity::Negative;
        atom.modality = ClaimModality::Prohibited;

        let negative = ClaimRealization::try_new(&atom, &node).expect("negative constraint");
        assert_eq!(negative.dimension, ValueDimension::Constraint);
        assert_eq!(negative.polarity, ClaimPolarity::Negative);
        assert_eq!(negative.mode, ClaimMode::Qualified);

        node.state = AnswerState::Unknown(UnknownReason::ParseFailed);
        let unknown = ClaimRealization::try_new(&atom, &node).expect("unknown realization");
        assert_eq!(
            unknown.state,
            AnswerState::Unknown(UnknownReason::ParseFailed)
        );
        assert_eq!(unknown.mode, ClaimMode::Omitted);
    }

    #[test]
    fn overclaim_risk_kind_covers_every_value_dimension() {
        let risks = ValueDimension::ALL.map(OverclaimRiskKind::for_dimension);
        assert_eq!(risks[0], OverclaimRiskKind::IdentityNotObserved);
        assert_eq!(risks[2], OverclaimRiskKind::ProblemNotObserved);
        assert_eq!(risks[7], OverclaimRiskKind::EvidenceNotEstablished);
        assert_eq!(risks[8], OverclaimRiskKind::ConstraintNotEstablished);
        assert_eq!(risks[9], OverclaimRiskKind::DifferentiationNotEstablished);
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

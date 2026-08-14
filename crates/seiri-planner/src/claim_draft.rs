use seiri_core::{
    AppealIrError, CapabilityNodeId, CapabilitySemanticSignature, CapabilitySupport, ClaimAtom,
    ClaimDraft, ClaimDraftIR, ClaimDraftId, ClaimDraftSemantics, ClaimModality, ClaimMode,
    ClaimPolarity, CoverageStatus, DocumentLanguage, EvidenceCeilingContribution, GateKind,
    ReadmeSectionLanguage, RepositoryAnalysis, SourceSpan, SupportState, TextDocumentBase,
    UnderclaimOpportunity, UnderclaimOpportunityKind,
};
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::num::NonZeroU32;

#[derive(Clone, PartialEq, Eq)]
struct DraftSeed {
    target_span: SourceSpan,
    source_claims: Vec<seiri_core::ClaimAtomId>,
    capability_nodes: Vec<CapabilityNodeId>,
    semantics: ClaimDraftSemantics,
    realization_mode: ClaimMode,
    claim_ceiling: ClaimMode,
}

/// Projects evidence-bounded underclaim opportunities into structured review
/// candidates. No rendered README prose or write operation is produced.
pub fn plan_claim_drafts(analysis: &RepositoryAnalysis) -> Result<ClaimDraftIR, AppealIrError> {
    let readme = analysis
        .readme_document
        .as_ref()
        .ok_or(AppealIrError::SourceMismatch)?;
    let source = analysis
        .source_store()
        .get(readme.path())
        .ok_or(AppealIrError::SourceMismatch)?;
    let base = TextDocumentBase::from_bytes(source.bytes());
    if base != *readme.base()
        || readme.source_bytes() != source.bytes().len()
        || analysis.readme_grammar.path != readme.path()
    {
        return Err(AppealIrError::SourceMismatch);
    }
    let translation = &analysis.readme_grammar.translation_alignment;
    if !translation.is_empty()
        && (translation.path != readme.path()
            || translation.source_digest != base.digest()
            || translation.source_byte_len != base.byte_len())
    {
        return Err(AppealIrError::SourceMismatch);
    }

    let mut seeds = Vec::new();
    for opportunity in analysis
        .claim_capability_membrane
        .opportunities
        .iter()
        .filter(|opportunity| eligible_opportunity(analysis, opportunity))
    {
        collect_opportunity_seeds(analysis, opportunity, &mut seeds)?;
    }
    seeds.sort_by(compare_seed);
    seeds.dedup();

    let drafts = seeds
        .into_iter()
        .enumerate()
        .map(|(index, seed)| {
            let id = ClaimDraftId::new(
                NonZeroU32::new((index + 1) as u32).expect("bounded claim draft count is non-zero"),
            );
            ClaimDraft::try_new(
                id,
                seed.target_span,
                seed.source_claims,
                seed.capability_nodes,
                seed.semantics,
                seed.realization_mode,
                seed.claim_ceiling,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    ClaimDraftIR::try_new(
        readme.path(),
        base.digest(),
        base.byte_len(),
        seiri_markdown::readme_grammar_unknown_count(&analysis.readme_grammar),
        drafts,
    )
}

fn eligible_opportunity(
    analysis: &RepositoryAnalysis,
    opportunity: &UnderclaimOpportunity,
) -> bool {
    if opportunity.gate != GateKind::Guarded
        || !matches!(
            opportunity.kind,
            UnderclaimOpportunityKind::MissingSupportedValue
                | UnderclaimOpportunityKind::ModeBelowFloor
        )
    {
        return false;
    }
    let membrane = &analysis.claim_capability_membrane;
    let supported = membrane.relations.iter().any(|relation| {
        relation.dimension == opportunity.dimension && relation.state == SupportState::Supported
    });
    let ceiling = membrane
        .ceiling
        .limits
        .get(&opportunity.dimension)
        .copied()
        .unwrap_or(ClaimMode::Omitted);
    supported && ceiling > ClaimMode::Generic
}

fn collect_opportunity_seeds(
    analysis: &RepositoryAnalysis,
    opportunity: &UnderclaimOpportunity,
    seeds: &mut Vec<DraftSeed>,
) -> Result<(), AppealIrError> {
    for signature in analysis
        .repository_capabilities
        .semantic_signatures
        .iter()
        .filter(|signature| {
            signature.dimension == opportunity.dimension
                && opportunity
                    .capability_nodes
                    .contains(&signature.capability_node)
                && !matches!(signature.semantic_support, CapabilitySupport::Unknown(_))
        })
    {
        let Some((capability_nodes, contribution_ceiling)) =
            signature_ceiling(analysis, signature)?
        else {
            continue;
        };
        let global_ceiling = analysis
            .claim_capability_membrane
            .ceiling
            .limits
            .get(&opportunity.dimension)
            .copied()
            .unwrap_or(ClaimMode::Omitted);
        let signature_ceiling = global_ceiling.min(contribution_ceiling);
        if signature_ceiling <= ClaimMode::Generic {
            continue;
        }

        match opportunity.kind {
            UnderclaimOpportunityKind::MissingSupportedValue => {
                for (language, span) in missing_value_targets(analysis) {
                    if let Some(seed) = seed_from_signature(
                        signature,
                        language,
                        span,
                        Vec::new(),
                        capability_nodes.clone(),
                        signature_ceiling,
                    ) {
                        seeds.push(seed);
                    }
                }
            }
            UnderclaimOpportunityKind::ModeBelowFloor => {
                collect_below_floor_seeds(
                    analysis,
                    opportunity,
                    signature,
                    &capability_nodes,
                    signature_ceiling,
                    seeds,
                );
            }
            _ => {}
        }
    }
    Ok(())
}

fn signature_ceiling(
    analysis: &RepositoryAnalysis,
    signature: &CapabilitySemanticSignature,
) -> Result<Option<(Vec<CapabilityNodeId>, ClaimMode)>, AppealIrError> {
    let Some(owner) = analysis
        .repository_capabilities
        .nodes
        .iter()
        .find(|node| node.id == signature.capability_node)
    else {
        return Ok(None);
    };
    let contribution = EvidenceCeilingContribution::try_new(
        owner,
        signature.dimension,
        signature.semantic_support,
    )?;
    if contribution.maximum() <= ClaimMode::Generic {
        return Ok(None);
    }
    let mut nodes = vec![signature.capability_node];
    nodes.extend(signature.input_nodes.iter().copied());
    nodes.extend(signature.output_nodes.iter().copied());
    nodes.extend(signature.condition_nodes.iter().copied());
    nodes.sort_unstable();
    nodes.dedup();
    Ok(Some((nodes, contribution.maximum())))
}

fn collect_below_floor_seeds(
    analysis: &RepositoryAnalysis,
    opportunity: &UnderclaimOpportunity,
    signature: &CapabilitySemanticSignature,
    capability_nodes: &[CapabilityNodeId],
    signature_ceiling: ClaimMode,
    seeds: &mut Vec<DraftSeed>,
) {
    for atom in analysis
        .readme_grammar
        .claim_atoms
        .atoms
        .iter()
        .filter(|atom| opportunity.grammar_nodes.contains(&atom.grammar_node))
    {
        let Some(alignment) =
            analysis
                .claim_capability_membrane
                .alignments
                .iter()
                .find(|alignment| {
                    alignment.claim_atom == atom.id
                        && alignment.state == SupportState::Supported
                        && alignment
                            .capability_nodes
                            .contains(&signature.capability_node)
                })
        else {
            continue;
        };
        let Some(span) = atom.span else {
            continue;
        };
        let ceiling = signature_ceiling.min(alignment.claim_ceiling);
        if ceiling <= ClaimMode::Generic || atom.polarity != signature.polarity {
            continue;
        }
        if let Some(seed) = seed_from_atom(atom, span, capability_nodes.to_vec(), ceiling) {
            seeds.push(seed);
        }
    }
}

fn seed_from_signature(
    signature: &CapabilitySemanticSignature,
    language: DocumentLanguage,
    target_span: SourceSpan,
    source_claims: Vec<seiri_core::ClaimAtomId>,
    capability_nodes: Vec<CapabilityNodeId>,
    claim_ceiling: ClaimMode,
) -> Option<DraftSeed> {
    let (realization_mode, modality) = target_mode(signature.polarity, claim_ceiling)?;
    let semantics = ClaimDraftSemantics::try_new(
        signature.dimension,
        signature.subject.clone(),
        signature.action.clone(),
        signature.object.clone(),
        signature.qualifiers.clone(),
        None,
        signature.polarity,
        modality,
        language,
    )
    .ok()?;
    Some(DraftSeed {
        target_span,
        source_claims,
        capability_nodes,
        semantics,
        realization_mode,
        claim_ceiling,
    })
}

fn seed_from_atom(
    atom: &ClaimAtom,
    target_span: SourceSpan,
    capability_nodes: Vec<CapabilityNodeId>,
    claim_ceiling: ClaimMode,
) -> Option<DraftSeed> {
    let (realization_mode, modality) = target_mode(atom.polarity, claim_ceiling)?;
    let semantics = ClaimDraftSemantics::try_new(
        atom.dimension,
        atom.subject.clone(),
        atom.action.clone(),
        atom.object.clone(),
        atom.qualifiers.clone(),
        atom.condition.clone(),
        atom.polarity,
        modality,
        atom.language,
    )
    .ok()?;
    Some(DraftSeed {
        target_span,
        source_claims: vec![atom.id],
        capability_nodes,
        semantics,
        realization_mode,
        claim_ceiling,
    })
}

const fn target_mode(
    polarity: ClaimPolarity,
    claim_ceiling: ClaimMode,
) -> Option<(ClaimMode, ClaimModality)> {
    match (polarity, claim_ceiling) {
        (ClaimPolarity::Negative, ClaimMode::Direct) => {
            Some((ClaimMode::Direct, ClaimModality::Prohibited))
        }
        (ClaimPolarity::Positive, ClaimMode::Direct) => {
            Some((ClaimMode::Direct, ClaimModality::Asserted))
        }
        (ClaimPolarity::Positive, ClaimMode::Qualified) => {
            Some((ClaimMode::Qualified, ClaimModality::Qualified))
        }
        (_, ClaimMode::Omitted | ClaimMode::Generic | ClaimMode::Qualified) => None,
    }
}

fn missing_value_targets(analysis: &RepositoryAnalysis) -> Vec<(DocumentLanguage, SourceSpan)> {
    let mut targets = BTreeMap::new();
    for section in &analysis.readme_grammar.translation_alignment.sections {
        let language = match section.language {
            ReadmeSectionLanguage::Japanese => DocumentLanguage::Japanese,
            ReadmeSectionLanguage::English => DocumentLanguage::English,
            ReadmeSectionLanguage::Mixed | ReadmeSectionLanguage::Ambiguous(_) => continue,
        };
        targets
            .entry(language)
            .and_modify(|current: &mut SourceSpan| {
                if section.span.byte_end > current.byte_end {
                    *current = section.span;
                }
            })
            .or_insert(section.span);
    }
    if targets.is_empty() {
        for node in &analysis.readme_grammar.nodes {
            let Some(span) = node.span else {
                continue;
            };
            targets
                .entry(node.language)
                .and_modify(|current: &mut SourceSpan| {
                    if span.byte_end > current.byte_end {
                        *current = span;
                    }
                })
                .or_insert(span);
        }
    }
    if targets.is_empty()
        && analysis.readme_grammar.coverage == CoverageStatus::Complete
        && analysis
            .readme_document
            .as_ref()
            .is_some_and(|readme| readme.source_bytes() == 0)
    {
        targets.insert(DocumentLanguage::English, SourceSpan::new(1, 1, 0, 0));
    }
    targets.into_iter().collect()
}

fn compare_seed(left: &DraftSeed, right: &DraftSeed) -> Ordering {
    left.target_span
        .line
        .cmp(&right.target_span.line)
        .then(left.target_span.column.cmp(&right.target_span.column))
        .then(
            left.target_span
                .byte_start
                .cmp(&right.target_span.byte_start),
        )
        .then(left.target_span.byte_end.cmp(&right.target_span.byte_end))
        .then(left.semantics.language.cmp(&right.semantics.language))
        .then(left.semantics.dimension.cmp(&right.semantics.dimension))
        .then(left.source_claims.cmp(&right.source_claims))
        .then(left.capability_nodes.cmp(&right.capability_nodes))
        .then(left.semantics.subject.cmp(&right.semantics.subject))
        .then(left.semantics.action.cmp(&right.semantics.action))
        .then(left.semantics.object.cmp(&right.semantics.object))
        .then(left.semantics.qualifiers.cmp(&right.semantics.qualifiers))
        .then(left.semantics.condition.cmp(&right.semantics.condition))
        .then(polarity_rank(left.semantics.polarity).cmp(&polarity_rank(right.semantics.polarity)))
        .then(modality_rank(left.semantics.modality).cmp(&modality_rank(right.semantics.modality)))
        .then(left.realization_mode.cmp(&right.realization_mode))
        .then(left.claim_ceiling.cmp(&right.claim_ceiling))
}

const fn polarity_rank(polarity: ClaimPolarity) -> u8 {
    match polarity {
        ClaimPolarity::Positive => 0,
        ClaimPolarity::Negative => 1,
    }
}

const fn modality_rank(modality: ClaimModality) -> u8 {
    match modality {
        ClaimModality::Asserted => 0,
        ClaimModality::Qualified => 1,
        ClaimModality::Possible => 2,
        ClaimModality::Required => 3,
        ClaimModality::Prohibited => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use seiri_core::{
        AppealLossVector, CapabilityKind, CapabilityNode, CapabilityProvenance,
        CapabilityProvenanceKind, ClaimCapabilityAlignment, ClaimCapabilityMembrane, ClaimCeiling,
        ClaimFloor, CoverageIncompleteReason, CoverageStatus, RepositoryCapabilityIR,
        SourceDocument, SourceStore, SupportRelation, CLAIM_CAPABILITY_MEMBRANE_REVISION,
    };
    use seiri_markdown::{
        analyze_readme_grammar_with_source, scan_document_with_options, DocumentScanOptions,
        ReadmeGrammarOptions,
    };
    use std::collections::BTreeMap;

    const SOURCE: &str = "# RepoSeiri\n\nRepoSeiri audits repositories locally.\n";

    fn capability_id(value: u32) -> CapabilityNodeId {
        CapabilityNodeId::new(NonZeroU32::new(value).expect("non-zero"))
    }

    fn analysis_fixture(source: &str) -> RepositoryAnalysis {
        let options = DocumentScanOptions::derived_for_source(source.len());
        let document = scan_document_with_options("README.md", source, &options).unwrap();
        let grammar =
            analyze_readme_grammar_with_source(&document, source, &ReadmeGrammarOptions::default())
                .unwrap();
        let owner = CapabilityNode {
            id: capability_id(1),
            kind: CapabilityKind::Operation,
            support: CapabilitySupport::Observed,
            symbol: "audit_repository".to_string(),
            provenance: vec![CapabilityProvenance {
                path: "src/lib.rs".to_string(),
                kind: CapabilityProvenanceKind::SourceSyntax,
                span: Some(SourceSpan::new(1, 1, 0, 10)),
            }],
        };
        let signature = CapabilitySemanticSignature {
            capability_node: owner.id,
            dimension: seiri_core::ValueDimension::Capability,
            subject: Some("reposeiri".to_string()),
            action: Some("audit".to_string()),
            object: Some("repository".to_string()),
            qualifiers: vec!["bounded".to_string(), "local".to_string()],
            polarity: ClaimPolarity::Positive,
            input_nodes: Vec::new(),
            output_nodes: Vec::new(),
            condition_nodes: Vec::new(),
            semantic_support: CapabilitySupport::Inferred,
        };
        let capabilities =
            RepositoryCapabilityIR::try_new_with_semantic_signatures_and_diagnostics(
                CoverageStatus::Complete,
                vec![owner],
                Vec::new(),
                vec![signature],
                Vec::new(),
                Vec::new(),
            )
            .unwrap();
        let mut ceiling = BTreeMap::new();
        ceiling.insert(seiri_core::ValueDimension::Capability, ClaimMode::Qualified);
        let relation = SupportRelation::try_new(
            seiri_core::ValueDimension::Capability,
            Vec::new(),
            vec![capability_id(1)],
            1,
            SupportState::Supported,
        )
        .unwrap();
        let opportunity = UnderclaimOpportunity {
            kind: UnderclaimOpportunityKind::MissingSupportedValue,
            dimension: seiri_core::ValueDimension::Capability,
            gate: GateKind::Guarded,
            grammar_nodes: Vec::new(),
            capability_nodes: vec![capability_id(1)],
        };
        let membrane = ClaimCapabilityMembrane {
            semantic_revision: CLAIM_CAPABILITY_MEMBRANE_REVISION.to_string(),
            floor: ClaimFloor {
                requirements: BTreeMap::new(),
            },
            ceiling: ClaimCeiling { limits: ceiling },
            relations: vec![relation],
            alignments: Vec::new(),
            opportunities: vec![opportunity],
            risks: Vec::new(),
            losses: AppealLossVector::default(),
        };

        let mut analysis = RepositoryAnalysis::new(".");
        analysis.readme_document = Some(document);
        analysis.readme_grammar = grammar;
        analysis.repository_capabilities = capabilities;
        analysis.claim_capability_membrane = membrane;
        analysis.attach_source_session(
            SourceStore::try_new(vec![SourceDocument::from_bytes(
                "README.md".to_string(),
                source.as_bytes().to_vec(),
            )])
            .unwrap(),
        );
        analysis
    }

    #[test]
    fn guarded_supported_missing_value_produces_bounded_draft() {
        let analysis = analysis_fixture(SOURCE);
        let drafts = plan_claim_drafts(&analysis).unwrap();

        assert_eq!(drafts.path, "README.md");
        assert_eq!(drafts.drafts.len(), 1);
        let draft = &drafts.drafts[0];
        assert!(draft.source_claims.is_empty());
        assert_eq!(draft.capability_nodes, vec![capability_id(1)]);
        assert_eq!(draft.semantics.action.as_deref(), Some("audit"));
        assert_eq!(draft.semantics.language, DocumentLanguage::English);
        assert_eq!(draft.realization_mode, ClaimMode::Qualified);
        assert_eq!(draft.claim_ceiling, ClaimMode::Qualified);
    }

    #[test]
    fn manual_unknown_generic_and_weak_negative_candidates_are_excluded() {
        let mut manual = analysis_fixture(SOURCE);
        manual.claim_capability_membrane.opportunities[0].gate = GateKind::Manual;
        assert!(plan_claim_drafts(&manual).unwrap().drafts.is_empty());

        let mut unknown = analysis_fixture(SOURCE);
        unknown.repository_capabilities.semantic_signatures[0].semantic_support =
            CapabilitySupport::Unknown(seiri_core::UnknownReason::UnsupportedSyntax);
        assert!(plan_claim_drafts(&unknown).unwrap().drafts.is_empty());

        let mut generic = analysis_fixture(SOURCE);
        generic
            .claim_capability_membrane
            .ceiling
            .limits
            .insert(seiri_core::ValueDimension::Capability, ClaimMode::Generic);
        assert!(plan_claim_drafts(&generic).unwrap().drafts.is_empty());

        let mut negative = analysis_fixture(SOURCE);
        negative.repository_capabilities.semantic_signatures[0].polarity = ClaimPolarity::Negative;
        assert!(plan_claim_drafts(&negative).unwrap().drafts.is_empty());

        let mut direct_negative = analysis_fixture(SOURCE);
        direct_negative.repository_capabilities.nodes[0].provenance[0].kind =
            CapabilityProvenanceKind::Documentation;
        direct_negative.repository_capabilities.semantic_signatures[0].semantic_support =
            CapabilitySupport::Observed;
        direct_negative.repository_capabilities.semantic_signatures[0].polarity =
            ClaimPolarity::Negative;
        direct_negative
            .claim_capability_membrane
            .ceiling
            .limits
            .insert(seiri_core::ValueDimension::Capability, ClaimMode::Direct);
        let drafts = plan_claim_drafts(&direct_negative).unwrap();
        assert_eq!(drafts.drafts.len(), 1);
        assert_eq!(drafts.drafts[0].realization_mode, ClaimMode::Direct);
        assert_eq!(
            drafts.drafts[0].semantics.modality,
            ClaimModality::Prohibited
        );
    }

    #[test]
    fn below_floor_draft_preserves_source_claim_semantics_and_alignment() {
        let mut analysis = analysis_fixture(SOURCE);
        let atom = analysis
            .readme_grammar
            .claim_atoms
            .atoms
            .iter_mut()
            .find(|atom| atom.action.as_deref() == Some("audit"))
            .expect("audit atom");
        atom.subject = Some("sourceclaim".to_string());
        atom.condition = Some("when requested".to_string());
        let atom_id = atom.id;
        let grammar_node = atom.grammar_node;
        let owner = &analysis.repository_capabilities.nodes[0];
        let contribution = EvidenceCeilingContribution::try_new(
            owner,
            seiri_core::ValueDimension::Capability,
            CapabilitySupport::Inferred,
        )
        .unwrap();
        analysis.claim_capability_membrane.alignments =
            vec![ClaimCapabilityAlignment::try_new_with_evidence_ceiling(
                atom_id,
                vec![capability_id(1)],
                1,
                SupportState::Supported,
                &[contribution],
            )
            .unwrap()];
        analysis.claim_capability_membrane.opportunities[0].kind =
            UnderclaimOpportunityKind::ModeBelowFloor;
        analysis.claim_capability_membrane.opportunities[0].grammar_nodes = vec![grammar_node];

        let drafts = plan_claim_drafts(&analysis).unwrap();
        assert_eq!(drafts.drafts.len(), 1);
        let draft = &drafts.drafts[0];
        assert_eq!(draft.source_claims, vec![atom_id]);
        assert_eq!(draft.semantics.subject.as_deref(), Some("sourceclaim"));
        assert_eq!(draft.semantics.condition.as_deref(), Some("when requested"));
    }

    #[test]
    fn empty_complete_readme_has_bounded_english_fallback_but_partial_does_not() {
        let complete = analysis_fixture("");
        let drafts = plan_claim_drafts(&complete).unwrap();
        assert_eq!(drafts.drafts.len(), 1);
        assert_eq!(drafts.drafts[0].target_span, SourceSpan::new(1, 1, 0, 0));
        assert_eq!(
            drafts.drafts[0].semantics.language,
            DocumentLanguage::English
        );

        let mut partial = analysis_fixture("");
        partial.readme_grammar.coverage =
            CoverageStatus::Partial(CoverageIncompleteReason::ParseFailed);
        assert!(plan_claim_drafts(&partial).unwrap().drafts.is_empty());
    }

    #[test]
    fn stale_readme_source_fails_before_planning() {
        let mut analysis = analysis_fixture(SOURCE);
        analysis.attach_source_session(
            SourceStore::try_new(vec![SourceDocument::from_bytes(
                "README.md".to_string(),
                b"stale".to_vec(),
            )])
            .unwrap(),
        );

        assert_eq!(
            plan_claim_drafts(&analysis),
            Err(AppealIrError::SourceMismatch)
        );
    }
}

use seiri_appeal::{
    evaluate_claim_capability_membrane as try_evaluate_claim_capability_membrane,
    update_incremental_membrane, verify_incremental_against_scalar, IncrementalMembraneState,
};
use seiri_core::{
    AnswerState, CapabilityKind, CapabilityNode, CapabilityNodeId, CapabilityProvenance,
    CapabilityProvenanceKind, CapabilitySemanticSignature, CapabilitySupport, ClaimAtom,
    ClaimAtomIR, ClaimAtomId, ClaimModality, ClaimPolarity, CoverageIncompleteReason,
    CoverageStatus, DocumentLanguage, GrammarNode, GrammarNodeId, GrammarPredicate, ProfileKind,
    ReadmeGrammarIR, RepositoryCapabilityIR, SourceDocument, SourceSpan, SourceStore, SupportState,
    UnknownReason, ValueDimension, README_CLAIM_ATOM_REVISION,
};
use seiri_program_local::{analyze_rust_capabilities, ProgramAnalysisOptions, ProgramSourceReport};
use std::num::NonZeroU32;

fn id(value: u32) -> NonZeroU32 {
    NonZeroU32::new(value).expect("non-zero id")
}

fn evaluate_claim_capability_membrane(
    grammar: &ReadmeGrammarIR,
    capabilities: &RepositoryCapabilityIR,
    profile: ProfileKind,
) -> seiri_core::ClaimCapabilityMembrane {
    try_evaluate_claim_capability_membrane(grammar, capabilities, profile)
        .expect("valid membrane inputs")
}

fn grammar(action: &str, object: &str, polarity: ClaimPolarity) -> ReadmeGrammarIR {
    let node_id = GrammarNodeId::new(id(1));
    let span = SourceSpan::new(1, 1, 0, 40);
    let modality = if polarity == ClaimPolarity::Negative {
        ClaimModality::Prohibited
    } else {
        ClaimModality::Asserted
    };
    ReadmeGrammarIR::try_new_with_claim_atoms(
        "README.md",
        CoverageStatus::Complete,
        vec![GrammarNode {
            id: node_id,
            predicate: GrammarPredicate::PerformsOperation,
            state: AnswerState::Explicit,
            language: DocumentLanguage::English,
            modality,
            negated: polarity == ClaimPolarity::Negative,
            span: Some(span),
        }],
        Vec::new(),
        Vec::new(),
        ClaimAtomIR {
            semantic_revision: README_CLAIM_ATOM_REVISION.to_string(),
            atoms: vec![ClaimAtom {
                id: ClaimAtomId::new(id(1)),
                grammar_node: node_id,
                dimension: ValueDimension::Capability,
                subject: Some("reposeiri".to_string()),
                action: Some(action.to_string()),
                object: Some(object.to_string()),
                qualifiers: Vec::new(),
                condition: None,
                polarity,
                modality,
                language: DocumentLanguage::English,
                span: Some(span),
            }],
        },
    )
    .expect("claim grammar")
}

fn legacy_grammar_without_atoms() -> ReadmeGrammarIR {
    ReadmeGrammarIR::try_new(
        "README.md",
        CoverageStatus::Complete,
        vec![GrammarNode {
            id: GrammarNodeId::new(id(1)),
            predicate: GrammarPredicate::PerformsOperation,
            state: AnswerState::Explicit,
            language: DocumentLanguage::English,
            modality: ClaimModality::Asserted,
            negated: false,
            span: Some(SourceSpan::new(1, 1, 0, 20)),
        }],
        Vec::new(),
        Vec::new(),
    )
    .expect("legacy grammar")
}

fn program(source: &str) -> RepositoryCapabilityIR {
    let store = SourceStore::try_new(vec![SourceDocument::from_bytes(
        "src/lib.rs".to_string(),
        source.as_bytes().to_vec(),
    )])
    .expect("source store");
    let report = ProgramSourceReport {
        coverage: CoverageStatus::Complete,
        candidate_files: 1,
        selected_files: 1,
        selected_bytes: store.total_bytes(),
        skipped_existing: 0,
        issues: Vec::new(),
    };
    analyze_rust_capabilities(&store, &report, &ProgramAnalysisOptions::default())
}

fn capability_relation(
    membrane: &seiri_core::ClaimCapabilityMembrane,
) -> &seiri_core::SupportRelation {
    membrane
        .relations
        .iter()
        .find(|relation| relation.dimension == ValueDimension::Capability)
        .expect("capability relation")
}

#[test]
fn unrelated_same_dimension_functions_do_not_support_a_claim() {
    let grammar = grammar("delete", "cloud", ClaimPolarity::Positive);
    let capabilities = program(
        "pub fn audit_repository() {}\n\
         pub fn inspect_configuration() {}\n\
         pub fn review_routes() {}\n",
    );
    let membrane = evaluate_claim_capability_membrane(&grammar, &capabilities, ProfileKind::Common);
    let relation = capability_relation(&membrane);

    assert_eq!(relation.state, SupportState::InsufficientEvidence);
    assert!(relation.capability_nodes.is_empty());
    assert_eq!(membrane.alignments.len(), 1);
    assert_eq!(
        membrane.alignments[0].state,
        SupportState::InsufficientEvidence
    );
    assert!(membrane.risks.iter().any(|risk| {
        risk.dimension == ValueDimension::Capability
            && risk.kind == seiri_core::OverclaimRiskKind::UnsupportedCapability
    }));
}

#[test]
fn normalized_action_and_object_alignment_is_supported() {
    let grammar = grammar("audit", "repositories", ClaimPolarity::Positive);
    let capabilities = program("pub fn audit_repository(path: &str) -> Report { todo!() }\n");
    let membrane = evaluate_claim_capability_membrane(&grammar, &capabilities, ProfileKind::Common);
    let relation = capability_relation(&membrane);

    assert_eq!(relation.state, SupportState::Supported);
    assert_eq!(relation.capability_nodes.len(), 1);
    assert!(relation.evidence_count > 0);
    assert_eq!(membrane.alignments[0].state, SupportState::Supported);
    assert_eq!(
        membrane.alignments[0].capability_nodes,
        relation.capability_nodes
    );
}

#[test]
fn opposite_polarity_is_contradicted_never_supported() {
    let grammar = grammar("audit", "repositories", ClaimPolarity::Negative);
    let capabilities = program("pub fn audit_repository() {}\n");
    let membrane = evaluate_claim_capability_membrane(&grammar, &capabilities, ProfileKind::Common);

    assert_eq!(
        capability_relation(&membrane).state,
        SupportState::Contradicted
    );
    assert_eq!(membrane.alignments[0].state, SupportState::Contradicted);
    assert!(!membrane
        .alignments
        .iter()
        .any(|alignment| { alignment.state == SupportState::Supported }));
}

#[test]
fn unknown_signature_and_partial_coverage_survive_the_wire() {
    let owner_id = CapabilityNodeId::new(id(1));
    let capabilities = RepositoryCapabilityIR::try_new_with_semantic_signatures_and_diagnostics(
        CoverageStatus::Partial(CoverageIncompleteReason::UnsupportedSyntax),
        vec![CapabilityNode {
            id: owner_id,
            kind: CapabilityKind::Operation,
            support: CapabilitySupport::Unknown(UnknownReason::UnsupportedSyntax),
            symbol: "audit_repository".to_string(),
            provenance: vec![CapabilityProvenance {
                path: "src/generated.rs".to_string(),
                kind: CapabilityProvenanceKind::SourceSyntax,
                span: Some(SourceSpan::new(1, 1, 0, 16)),
            }],
        }],
        Vec::new(),
        vec![CapabilitySemanticSignature {
            capability_node: owner_id,
            dimension: ValueDimension::Capability,
            subject: None,
            action: Some("audit".to_string()),
            object: Some("repository".to_string()),
            qualifiers: Vec::new(),
            polarity: ClaimPolarity::Positive,
            input_nodes: Vec::new(),
            output_nodes: Vec::new(),
            condition_nodes: Vec::new(),
            semantic_support: CapabilitySupport::Unknown(UnknownReason::UnsupportedSyntax),
        }],
        Vec::new(),
        Vec::new(),
    )
    .expect("unknown signature");
    assert_eq!(
        capabilities.unknown_reasons,
        [UnknownReason::UnsupportedSyntax]
    );
    let membrane = evaluate_claim_capability_membrane(
        &grammar("audit", "repositories", ClaimPolarity::Positive),
        &capabilities,
        ProfileKind::Common,
    );

    assert_eq!(
        capability_relation(&membrane).state,
        SupportState::Unknown(UnknownReason::UnsupportedSyntax)
    );
    let json = serde_json::to_string(&membrane).expect("serialize membrane");
    let roundtrip: seiri_core::ClaimCapabilityMembrane =
        serde_json::from_str(&json).expect("deserialize membrane");
    assert_eq!(roundtrip, membrane);
}

#[test]
fn dimension_only_legacy_join_is_insufficient() {
    let membrane = evaluate_claim_capability_membrane(
        &legacy_grammar_without_atoms(),
        &program("pub fn audit_repository() {}\n"),
        ProfileKind::Common,
    );
    let relation = capability_relation(&membrane);
    assert_eq!(relation.state, SupportState::InsufficientEvidence);
    assert!(relation.capability_nodes.is_empty());
    assert!(membrane.alignments.is_empty());
}

#[test]
fn signature_change_is_incrementally_equivalent_to_scalar() {
    let grammar = grammar("audit", "repositories", ClaimPolarity::Positive);
    let before = program("pub fn inspect_repository() {}\n");
    let after = program("pub fn audit_repository() {}\n");
    let state = IncrementalMembraneState::from_scalar(&grammar, &before, ProfileKind::Common)
        .expect("valid scalar state");
    let update = update_incremental_membrane(
        &state,
        &grammar,
        &after,
        ProfileKind::Common,
        &["src/lib.rs".to_string()],
    )
    .expect("valid incremental update");

    assert!(
        verify_incremental_against_scalar(&update, &grammar, &after, ProfileKind::Common,)
            .expect("valid scalar verification")
            .equivalent
    );
    assert_eq!(
        capability_relation(update.state.membrane()).state,
        SupportState::Supported
    );
}

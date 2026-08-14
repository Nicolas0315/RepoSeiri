use seiri_appeal::{
    evaluate_claim_capability_membrane as try_evaluate_claim_capability_membrane,
    update_incremental_membrane, verify_incremental_against_scalar, IncrementalMembraneState,
};
use seiri_codex::CodexQueryKind;
use seiri_core::{
    AnswerState, CapabilityKind, CapabilityNode, CapabilityNodeId, CapabilityProvenance,
    CapabilityProvenanceKind, CapabilitySemanticSignature, CapabilitySupport, ClaimAtom,
    ClaimAtomIR, ClaimAtomId, ClaimModality, ClaimMode, ClaimPolarity, CoverageIncompleteReason,
    CoverageStatus, DocumentLanguage, GrammarNode, GrammarNodeId, GrammarPredicate,
    OverclaimRiskKind, ProfileKind, ReadmeGrammarIR, RepositoryCapabilityIR, SourceSpan,
    SupportState, UnknownReason, ValueDimension, CODEX_SCHEMA_VERSION, README_CLAIM_ATOM_REVISION,
};
use std::collections::BTreeSet;
use std::num::NonZeroU32;

fn nonzero(value: u32) -> NonZeroU32 {
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

fn grammar(
    predicate: GrammarPredicate,
    action: &str,
    object: &str,
    qualifiers: &[&str],
    polarity: ClaimPolarity,
    modality: ClaimModality,
) -> ReadmeGrammarIR {
    let node_id = GrammarNodeId::new(nonzero(1));
    let span = SourceSpan::new(1, 1, 0, 32);
    let dimension = predicate.dimension();
    ReadmeGrammarIR::try_new_with_claim_atoms(
        "README.md",
        CoverageStatus::Complete,
        vec![GrammarNode {
            id: node_id,
            predicate,
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
                id: ClaimAtomId::new(nonzero(1)),
                grammar_node: node_id,
                dimension,
                subject: Some("reposeiri".to_string()),
                action: Some(action.to_string()),
                object: Some(object.to_string()),
                qualifiers: qualifiers
                    .iter()
                    .map(|value| (*value).to_string())
                    .collect(),
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

fn empty_grammar() -> ReadmeGrammarIR {
    ReadmeGrammarIR::try_new(
        "README.md",
        CoverageStatus::Complete,
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .expect("empty grammar")
}

fn node(id: u32, kind: CapabilityKind, support: CapabilitySupport, path: &str) -> CapabilityNode {
    CapabilityNode {
        id: CapabilityNodeId::new(nonzero(id)),
        kind,
        support,
        symbol: format!("{kind:?}-{id}"),
        provenance: vec![CapabilityProvenance {
            path: path.to_string(),
            kind: CapabilityProvenanceKind::Documentation,
            span: Some(SourceSpan::new(
                id as usize,
                1,
                id as usize,
                id as usize + 1,
            )),
        }],
    }
}

fn signature(
    owner: &CapabilityNode,
    action: &str,
    object: &str,
    qualifiers: &[&str],
    polarity: ClaimPolarity,
    support: CapabilitySupport,
) -> CapabilitySemanticSignature {
    CapabilitySemanticSignature {
        capability_node: owner.id,
        dimension: owner.kind.dimension(),
        subject: Some("reposeiri".to_string()),
        action: Some(action.to_string()),
        object: Some(object.to_string()),
        qualifiers: qualifiers
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
        polarity,
        input_nodes: Vec::new(),
        output_nodes: Vec::new(),
        condition_nodes: Vec::new(),
        semantic_support: support,
    }
}

fn capabilities(
    coverage: CoverageStatus,
    nodes: Vec<CapabilityNode>,
    signatures: Vec<CapabilitySemanticSignature>,
) -> RepositoryCapabilityIR {
    RepositoryCapabilityIR::try_new_with_semantic_signatures_and_diagnostics(
        coverage,
        nodes,
        Vec::new(),
        signatures,
        Vec::new(),
        Vec::new(),
    )
    .expect("capability IR")
}

fn relation(
    membrane: &seiri_core::ClaimCapabilityMembrane,
    dimension: ValueDimension,
) -> &seiri_core::SupportRelation {
    membrane
        .relations
        .iter()
        .find(|relation| relation.dimension == dimension)
        .expect("dimension relation")
}

#[test]
fn generic_output_never_establishes_first_result() {
    let output = node(
        1,
        CapabilityKind::Output,
        CapabilitySupport::Observed,
        "src/lib.rs",
    );
    let capabilities = capabilities(CoverageStatus::Complete, vec![output], Vec::new());
    let empty =
        evaluate_claim_capability_membrane(&empty_grammar(), &capabilities, ProfileKind::Common);
    assert_eq!(
        empty.ceiling.limits[&ValueDimension::Outcome],
        ClaimMode::Qualified
    );
    assert_eq!(
        empty.ceiling.limits[&ValueDimension::FirstResult],
        ClaimMode::Omitted
    );

    let claimed = evaluate_claim_capability_membrane(
        &grammar(
            GrammarPredicate::DescribesFirstResult,
            "produce",
            "report",
            &[],
            ClaimPolarity::Positive,
            ClaimModality::Qualified,
        ),
        &capabilities,
        ProfileKind::Common,
    );
    assert_ne!(
        relation(&claimed, ValueDimension::FirstResult).state,
        SupportState::Supported
    );
    assert!(claimed.risks.iter().any(|risk| {
        risk.dimension == ValueDimension::FirstResult
            && risk.kind == OverclaimRiskKind::RuntimeSuccessNotObserved
    }));
}

#[test]
fn example_is_first_action_not_runtime_success_or_free_evidence() {
    let example = node(
        1,
        CapabilityKind::Example,
        CapabilitySupport::Observed,
        "examples/demo.rs",
    );
    let membrane = evaluate_claim_capability_membrane(
        &empty_grammar(),
        &capabilities(CoverageStatus::Complete, vec![example], Vec::new()),
        ProfileKind::Common,
    );
    assert_eq!(
        membrane.ceiling.limits[&ValueDimension::FirstAction],
        ClaimMode::Qualified
    );
    assert_eq!(
        membrane.ceiling.limits[&ValueDimension::FirstResult],
        ClaimMode::Omitted
    );
    assert_eq!(
        membrane.ceiling.limits[&ValueDimension::Evidence],
        ClaimMode::Omitted
    );
}

#[test]
fn negative_constraint_absence_is_not_proof() {
    let membrane = evaluate_claim_capability_membrane(
        &grammar(
            GrammarPredicate::StatesConstraint,
            "write",
            "files",
            &[],
            ClaimPolarity::Negative,
            ClaimModality::Prohibited,
        ),
        &capabilities(CoverageStatus::Complete, Vec::new(), Vec::new()),
        ProfileKind::Common,
    );
    assert_eq!(
        relation(&membrane, ValueDimension::Constraint).state,
        SupportState::InsufficientEvidence
    );
    assert_eq!(
        membrane.ceiling.limits[&ValueDimension::Constraint],
        ClaimMode::Omitted
    );
    assert!(membrane.risks.iter().any(|risk| {
        risk.dimension == ValueDimension::Constraint
            && risk.kind == OverclaimRiskKind::ConstraintNotEstablished
    }));
}

#[test]
fn partial_coverage_keeps_negative_claim_unknown_even_with_a_match() {
    let owner = node(
        1,
        CapabilityKind::Constraint,
        CapabilitySupport::Observed,
        "policy/limits.md",
    );
    let semantic = signature(
        &owner,
        "write",
        "files",
        &[],
        ClaimPolarity::Negative,
        CapabilitySupport::Observed,
    );
    let membrane = evaluate_claim_capability_membrane(
        &grammar(
            GrammarPredicate::StatesConstraint,
            "write",
            "files",
            &[],
            ClaimPolarity::Negative,
            ClaimModality::Prohibited,
        ),
        &capabilities(
            CoverageStatus::Partial(CoverageIncompleteReason::Unavailable),
            vec![owner],
            vec![semantic],
        ),
        ProfileKind::Common,
    );
    assert_eq!(
        relation(&membrane, ValueDimension::Constraint).state,
        SupportState::Unknown(UnknownReason::Unavailable)
    );
    assert_eq!(membrane.alignments[0].claim_ceiling, ClaimMode::Omitted);
}

#[test]
fn only_comparison_receipt_can_raise_differentiation() {
    let performance_claim = grammar(
        GrammarPredicate::StatesDifferentiation,
        "outperform",
        "alternatives",
        &["faster"],
        ClaimPolarity::Positive,
        ClaimModality::Qualified,
    );
    let test_node = node(
        1,
        CapabilityKind::Test,
        CapabilitySupport::Observed,
        "tests/performance.rs",
    );
    let without_receipt = evaluate_claim_capability_membrane(
        &performance_claim,
        &capabilities(CoverageStatus::Complete, vec![test_node], Vec::new()),
        ProfileKind::Common,
    );
    assert_eq!(
        without_receipt.ceiling.limits[&ValueDimension::Differentiation],
        ClaimMode::Omitted
    );
    assert!(without_receipt.risks.iter().any(|risk| {
        risk.kind == OverclaimRiskKind::PerformanceNotMeasured
            && risk.dimension == ValueDimension::Differentiation
    }));

    let receipt = node(
        1,
        CapabilityKind::ComparisonReceipt,
        CapabilitySupport::Observed,
        "benchmarks/comparison.json",
    );
    let semantic = signature(
        &receipt,
        "outperform",
        "alternatives",
        &["faster"],
        ClaimPolarity::Positive,
        CapabilitySupport::Observed,
    );
    let with_receipt = evaluate_claim_capability_membrane(
        &performance_claim,
        &capabilities(CoverageStatus::Complete, vec![receipt], vec![semantic]),
        ProfileKind::Common,
    );
    assert_eq!(
        relation(&with_receipt, ValueDimension::Differentiation).state,
        SupportState::Supported
    );
    assert_eq!(
        with_receipt.alignments[0].claim_ceiling,
        ClaimMode::Qualified
    );
    assert!(!with_receipt
        .risks
        .iter()
        .any(|risk| risk.dimension == ValueDimension::Differentiation));
}

#[test]
fn inferred_semantics_never_authorize_a_direct_claim() {
    let owner = node(
        1,
        CapabilityKind::Operation,
        CapabilitySupport::Observed,
        "src/lib.rs",
    );
    let semantic = signature(
        &owner,
        "audit",
        "repository",
        &[],
        ClaimPolarity::Positive,
        CapabilitySupport::Inferred,
    );
    let membrane = evaluate_claim_capability_membrane(
        &grammar(
            GrammarPredicate::PerformsOperation,
            "audit",
            "repository",
            &[],
            ClaimPolarity::Positive,
            ClaimModality::Asserted,
        ),
        &capabilities(CoverageStatus::Complete, vec![owner], vec![semantic]),
        ProfileKind::Common,
    );
    assert_eq!(
        relation(&membrane, ValueDimension::Capability).state,
        SupportState::Supported
    );
    assert_eq!(
        membrane.ceiling.limits[&ValueDimension::Capability],
        ClaimMode::Qualified
    );
    assert_eq!(membrane.alignments[0].claim_ceiling, ClaimMode::Qualified);
    assert!(membrane.risks.iter().any(|risk| {
        risk.dimension == ValueDimension::Capability
            && risk.kind == OverclaimRiskKind::UnsupportedCapability
    }));
}

#[test]
fn unknown_semantics_never_raise_either_ceiling() {
    let owner = node(
        1,
        CapabilityKind::Operation,
        CapabilitySupport::Unknown(UnknownReason::UnsupportedSyntax),
        "src/generated.rs",
    );
    let semantic = signature(
        &owner,
        "audit",
        "repository",
        &[],
        ClaimPolarity::Positive,
        CapabilitySupport::Unknown(UnknownReason::UnsupportedSyntax),
    );
    let membrane = evaluate_claim_capability_membrane(
        &grammar(
            GrammarPredicate::PerformsOperation,
            "audit",
            "repository",
            &[],
            ClaimPolarity::Positive,
            ClaimModality::Qualified,
        ),
        &capabilities(
            CoverageStatus::Partial(CoverageIncompleteReason::UnsupportedSyntax),
            vec![owner],
            vec![semantic],
        ),
        ProfileKind::Common,
    );
    assert_eq!(
        relation(&membrane, ValueDimension::Capability).state,
        SupportState::Unknown(UnknownReason::UnsupportedSyntax)
    );
    assert_eq!(
        membrane.ceiling.limits[&ValueDimension::Capability],
        ClaimMode::Omitted
    );
    assert_eq!(membrane.alignments[0].claim_ceiling, ClaimMode::Omitted);
}

#[test]
fn positive_and_negative_evidence_is_a_contradiction() {
    let positive = node(
        1,
        CapabilityKind::Operation,
        CapabilitySupport::Observed,
        "contracts/positive.json",
    );
    let negative = node(
        2,
        CapabilityKind::Operation,
        CapabilitySupport::Observed,
        "contracts/negative.json",
    );
    let positive_signature = signature(
        &positive,
        "audit",
        "repository",
        &[],
        ClaimPolarity::Positive,
        CapabilitySupport::Observed,
    );
    let negative_signature = signature(
        &negative,
        "audit",
        "repository",
        &[],
        ClaimPolarity::Negative,
        CapabilitySupport::Observed,
    );
    let membrane = evaluate_claim_capability_membrane(
        &grammar(
            GrammarPredicate::PerformsOperation,
            "audit",
            "repository",
            &[],
            ClaimPolarity::Positive,
            ClaimModality::Asserted,
        ),
        &capabilities(
            CoverageStatus::Complete,
            vec![positive, negative],
            vec![positive_signature, negative_signature],
        ),
        ProfileKind::Common,
    );
    assert_eq!(
        relation(&membrane, ValueDimension::Capability).state,
        SupportState::Contradicted
    );
    assert_eq!(membrane.alignments[0].capability_nodes.len(), 2);
    assert_eq!(membrane.alignments[0].claim_ceiling, ClaimMode::Omitted);
}

#[test]
fn every_realized_dimension_has_a_fail_closed_risk() {
    let predicates = [
        GrammarPredicate::DefinesIdentity,
        GrammarPredicate::TargetsAudience,
        GrammarPredicate::StatesProblem,
        GrammarPredicate::PerformsOperation,
        GrammarPredicate::ProducesOutcome,
        GrammarPredicate::DescribesFirstAction,
        GrammarPredicate::DescribesFirstResult,
        GrammarPredicate::ProvidesEvidence,
        GrammarPredicate::StatesConstraint,
        GrammarPredicate::StatesDifferentiation,
    ];
    let mut nodes = Vec::new();
    let mut atoms = Vec::new();
    for (index, predicate) in predicates.into_iter().enumerate() {
        let raw_id = (index + 1) as u32;
        let grammar_node = GrammarNodeId::new(nonzero(raw_id));
        let span = SourceSpan::new(index + 1, 1, index * 8, index * 8 + 4);
        nodes.push(GrammarNode {
            id: grammar_node,
            predicate,
            state: AnswerState::Explicit,
            language: DocumentLanguage::English,
            modality: ClaimModality::Asserted,
            negated: false,
            span: Some(span),
        });
        atoms.push(ClaimAtom {
            id: ClaimAtomId::new(nonzero(raw_id)),
            grammar_node,
            dimension: predicate.dimension(),
            subject: Some("reposeiri".to_string()),
            action: Some(format!("claim-{raw_id}")),
            object: Some("value".to_string()),
            qualifiers: Vec::new(),
            condition: None,
            polarity: ClaimPolarity::Positive,
            modality: ClaimModality::Asserted,
            language: DocumentLanguage::English,
            span: Some(span),
        });
    }
    let grammar = ReadmeGrammarIR::try_new_with_claim_atoms(
        "README.md",
        CoverageStatus::Complete,
        nodes,
        Vec::new(),
        Vec::new(),
        ClaimAtomIR {
            semantic_revision: README_CLAIM_ATOM_REVISION.to_string(),
            atoms,
        },
    )
    .expect("all-dimension grammar");
    let membrane = evaluate_claim_capability_membrane(
        &grammar,
        &capabilities(CoverageStatus::Complete, Vec::new(), Vec::new()),
        ProfileKind::Common,
    );
    let risk_dimensions = membrane
        .risks
        .iter()
        .map(|risk| risk.dimension)
        .collect::<BTreeSet<_>>();
    assert_eq!(risk_dimensions, ValueDimension::ALL.into_iter().collect());
    assert_eq!(membrane.losses.above_ceiling_dimensions, 10);

    let first = serde_json::to_string(&membrane).expect("serialize membrane");
    let second = serde_json::to_string(&membrane).expect("serialize membrane again");
    assert_eq!(first, second);
    let roundtrip: seiri_core::ClaimCapabilityMembrane =
        serde_json::from_str(&first).expect("roundtrip membrane");
    assert_eq!(roundtrip, membrane);
}

#[test]
fn first_result_unknown_to_observed_is_incrementally_scalar_equivalent() {
    let grammar = grammar(
        GrammarPredicate::DescribesFirstResult,
        "produce",
        "report",
        &[],
        ClaimPolarity::Positive,
        ClaimModality::Qualified,
    );
    let unknown_owner = node(
        1,
        CapabilityKind::FirstResult,
        CapabilitySupport::Unknown(UnknownReason::UnsupportedSyntax),
        "examples/output.txt",
    );
    let unknown_signature = signature(
        &unknown_owner,
        "produce",
        "report",
        &[],
        ClaimPolarity::Positive,
        CapabilitySupport::Unknown(UnknownReason::UnsupportedSyntax),
    );
    let before = capabilities(
        CoverageStatus::Partial(CoverageIncompleteReason::UnsupportedSyntax),
        vec![unknown_owner],
        vec![unknown_signature],
    );
    let observed_owner = node(
        1,
        CapabilityKind::FirstResult,
        CapabilitySupport::Observed,
        "examples/output.txt",
    );
    let observed_signature = signature(
        &observed_owner,
        "produce",
        "report",
        &[],
        ClaimPolarity::Positive,
        CapabilitySupport::Observed,
    );
    let after = capabilities(
        CoverageStatus::Complete,
        vec![observed_owner],
        vec![observed_signature],
    );
    let state = IncrementalMembraneState::from_scalar(&grammar, &before, ProfileKind::Common)
        .expect("valid scalar state");
    let update = update_incremental_membrane(
        &state,
        &grammar,
        &after,
        ProfileKind::Common,
        &["examples/output.txt".to_string()],
    )
    .expect("valid incremental update");
    assert!(
        verify_incremental_against_scalar(&update, &grammar, &after, ProfileKind::Common)
            .expect("valid scalar verification")
            .equivalent
    );
    assert_eq!(
        relation(update.state.membrane(), ValueDimension::FirstResult).state,
        SupportState::Supported
    );
    assert_eq!(
        update.state.membrane().alignments[0].claim_ceiling,
        ClaimMode::Qualified
    );
}

#[test]
fn codex_outer_contract_remains_v2_with_ten_queries() {
    assert_eq!(CODEX_SCHEMA_VERSION, "seiri.codex.v2");
    assert_eq!(CodexQueryKind::ALL.len(), 10);
    assert_eq!(
        CodexQueryKind::ALL.map(CodexQueryKind::slug),
        [
            "summary",
            "routes",
            "evidence",
            "documents",
            "governance",
            "patches",
            "linter",
            "actions",
            "remote",
            "pr-body",
        ]
    );
}

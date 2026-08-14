use seiri_core::{
    AnswerState, ClaimAtom, ClaimPolarity, CoverageIncompleteReason, CoverageStatus,
    GrammarPredicate, ReadmeGrammarIR, UnknownReason,
};
use seiri_markdown::{analyze_readme_grammar_with_source, scan_document, ReadmeGrammarOptions};
use std::collections::BTreeSet;

fn analyze(source: &str) -> ReadmeGrammarIR {
    let scan = scan_document("README.md", source).expect("scan README");
    analyze_readme_grammar_with_source(&scan, source, &ReadmeGrammarOptions::default())
        .expect("analyze claim atoms")
}

#[test]
fn claim_atoms_are_unique_and_source_bound_to_grammar_nodes() {
    let source = "RepoSeiri audits repositories locally and produces output.\n";
    let grammar = analyze(source);
    let node_ids = grammar
        .nodes
        .iter()
        .map(|node| node.id)
        .collect::<BTreeSet<_>>();
    let atom_node_ids = grammar
        .claim_atoms
        .atoms
        .iter()
        .map(|atom| atom.grammar_node)
        .collect::<BTreeSet<_>>();
    assert_eq!(grammar.claim_atoms.atoms.len(), grammar.nodes.len());
    assert_eq!(node_ids, atom_node_ids);

    for atom in &grammar.claim_atoms.atoms {
        let node = grammar
            .nodes
            .iter()
            .find(|node| node.id == atom.grammar_node)
            .expect("atom grammar node");
        assert_eq!(atom.span, node.span);
        let span = atom.span.expect("source-bound atom span");
        assert!(source.get(span.byte_start..span.byte_end).is_some());
    }
}

#[test]
fn positive_audit_claim_has_canonical_semantic_terms() {
    let grammar = analyze("RepoSeiri audits repositories locally.\n");
    let atom = atom_for(&grammar, GrammarPredicate::PerformsOperation);
    assert_eq!(atom.subject.as_deref(), Some("reposeiri"));
    assert_eq!(atom.action.as_deref(), Some("audit"));
    assert_eq!(atom.object.as_deref(), Some("repositories"));
    assert_eq!(atom.qualifiers, ["locally"]);
    assert_eq!(atom.polarity, ClaimPolarity::Positive);
}

#[test]
fn network_negation_does_not_change_the_following_audit_atom() {
    let grammar = analyze("RepoSeiri does not use the network; audits repositories locally.\n");
    let constraint = atom_for(&grammar, GrammarPredicate::StatesConstraint);
    assert_eq!(constraint.action.as_deref(), Some("use"));
    assert_eq!(constraint.object.as_deref(), Some("network"));
    assert_eq!(constraint.polarity, ClaimPolarity::Negative);

    let operation = atom_for(&grammar, GrammarPredicate::PerformsOperation);
    assert_eq!(operation.action.as_deref(), Some("audit"));
    assert_eq!(operation.object.as_deref(), Some("repositories"));
    assert_eq!(operation.polarity, ClaimPolarity::Positive);
}

#[test]
fn inline_formatting_preserves_the_semantic_atom_projection() {
    let plain = analyze("RepoSeiri does not write files; it audits repositories locally.\n");
    let formatted =
        analyze("RepoSeiri does **not** write files; it `audits` repositories locally.\n");
    assert_eq!(semantic_projection(&plain), semantic_projection(&formatted));
}

#[test]
fn json_roundtrip_preserves_claim_atoms_unknown_and_partial_coverage() {
    let mut grammar = analyze("RepoSeiri audits repositories locally.\n");
    grammar.nodes[0].state = AnswerState::Unknown(UnknownReason::ParseFailed);
    grammar.coverage = CoverageStatus::Partial(CoverageIncompleteReason::ParseFailed);
    let rebuilt = ReadmeGrammarIR::try_new_with_claim_atoms(
        grammar.path.clone(),
        grammar.coverage,
        grammar.nodes.clone(),
        grammar.edges.clone(),
        grammar.diagnostics.clone(),
        grammar.claim_atoms.clone(),
    )
    .expect("rebuild partial grammar");

    let json = serde_json::to_string(&rebuilt).expect("serialize grammar");
    let roundtrip: ReadmeGrammarIR = serde_json::from_str(&json).expect("deserialize grammar");
    assert_eq!(roundtrip, rebuilt);
    assert_eq!(
        roundtrip.nodes[0].state,
        AnswerState::Unknown(UnknownReason::ParseFailed)
    );
    assert_eq!(
        roundtrip.coverage,
        CoverageStatus::Partial(CoverageIncompleteReason::ParseFailed)
    );
}

fn atom_for(grammar: &ReadmeGrammarIR, predicate: GrammarPredicate) -> &ClaimAtom {
    let node = grammar
        .nodes
        .iter()
        .find(|node| node.predicate == predicate)
        .expect("grammar predicate");
    grammar
        .claim_atoms
        .atoms
        .iter()
        .find(|atom| atom.grammar_node == node.id)
        .expect("claim atom")
}

type SemanticProjection<'a> = (
    seiri_core::ValueDimension,
    Option<&'a str>,
    Option<&'a str>,
    Option<&'a str>,
    Vec<&'a str>,
    Option<&'a str>,
    ClaimPolarity,
);

fn semantic_projection(grammar: &ReadmeGrammarIR) -> Vec<SemanticProjection<'_>> {
    let mut projection = grammar
        .claim_atoms
        .atoms
        .iter()
        .map(|atom| {
            (
                atom.dimension,
                atom.subject.as_deref(),
                atom.action.as_deref(),
                atom.object.as_deref(),
                atom.qualifiers.iter().map(String::as_str).collect(),
                atom.condition.as_deref(),
                atom.polarity,
            )
        })
        .collect::<Vec<_>>();
    projection.sort_unstable();
    projection
}

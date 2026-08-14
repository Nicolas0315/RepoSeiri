use seiri_core::{
    ClaimModality, ClaimPolarity, CoverageStatus, GrammarPredicate, README_CLAIM_ATOM_REVISION,
    README_GRAMMAR_REVISION, README_TRANSLATION_ALIGNMENT_REVISION,
};
use seiri_markdown::{analyze_readme_grammar_with_source, scan_document, ReadmeGrammarOptions};

fn analyze(source: &str) -> seiri_core::ReadmeGrammarIR {
    let scan = scan_document("README.md", source).expect("scan source");
    analyze_readme_grammar_with_source(&scan, source, &ReadmeGrammarOptions::default())
        .expect("analyze source")
}

#[test]
fn english_negation_is_scoped_to_its_predicate() {
    let grammar =
        analyze("# RepoSeiri\n\nRepoSeiri does not use network and audits repositories locally.\n");
    let constraint = grammar
        .nodes
        .iter()
        .find(|node| node.predicate == GrammarPredicate::StatesConstraint && node.negated)
        .expect("negative constraint node");
    let operation = grammar
        .nodes
        .iter()
        .find(|node| node.predicate == GrammarPredicate::PerformsOperation)
        .expect("operation node");
    assert!(constraint.negated);
    assert_eq!(constraint.modality, ClaimModality::Prohibited);
    assert!(!operation.negated);
    assert_eq!(operation.modality, ClaimModality::Asserted);
    assert_eq!(
        grammar
            .claim_atoms
            .atoms
            .iter()
            .find(|atom| atom.grammar_node == operation.id)
            .expect("operation atom")
            .polarity,
        ClaimPolarity::Positive
    );
}

#[test]
fn substring_and_japanese_adjective_do_not_create_negation() {
    let english = analyze("# RepoSeiri\n\nNevertheless, RepoSeiri audits repositories.\n");
    assert!(english.nodes.iter().all(|node| !node.negated));

    let japanese = analyze("# RepoSeiri\n\n少ない設定でリポジトリを監査します。\n");
    assert_eq!(japanese.coverage, CoverageStatus::Complete);
    assert!(japanese.nodes.iter().all(|node| !node.negated));
    assert!(!japanese
        .nodes
        .iter()
        .any(|node| node.predicate == GrammarPredicate::StatesConstraint));
}

#[test]
fn explicit_japanese_negative_constraint_remains_negative() {
    let grammar = analyze("# 制約\n\nネットワークへ接続しない。\n");
    let constraint = grammar
        .nodes
        .iter()
        .find(|node| node.predicate == GrammarPredicate::StatesConstraint && node.negated)
        .expect("negative constraint node");
    assert!(constraint.negated);
    assert_eq!(constraint.modality, ClaimModality::Prohibited);
}

#[test]
fn polarity_semantics_have_explicit_revision_bumps() {
    assert_eq!(README_GRAMMAR_REVISION, "seiri.readme-grammar.v2");
    assert_eq!(README_CLAIM_ATOM_REVISION, "seiri.readme-claim-atom.v2");
    assert_eq!(
        README_TRANSLATION_ALIGNMENT_REVISION,
        "seiri.readme-translation-alignment.v2"
    );
}

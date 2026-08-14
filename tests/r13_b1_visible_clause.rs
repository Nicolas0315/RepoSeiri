use seiri_core::{
    AppealIrError, ClaimModality, CoverageIncompleteReason, CoverageStatus, GrammarDiagnosticKind,
    GrammarNode, GrammarPredicate,
};
use seiri_markdown::{analyze_readme_grammar_with_source, scan_document, ReadmeGrammarOptions};
use std::collections::BTreeSet;

fn analyze(source: &str) -> seiri_core::ReadmeGrammarIR {
    let scan = scan_document("README.md", source).expect("scan README");
    analyze_readme_grammar_with_source(&scan, source, &ReadmeGrammarOptions::default())
        .expect("analyze source-aware README grammar")
}

fn source_text<'a>(source: &'a str, node: &GrammarNode) -> &'a str {
    let span = node.span.expect("explicit grammar node span");
    source
        .get(span.byte_start..span.byte_end)
        .expect("grammar span remains on UTF-8 boundaries")
}

fn without_inline_markers(value: &str) -> String {
    value
        .chars()
        .filter(|character| !matches!(character, '*' | '`'))
        .collect()
}

#[test]
fn semicolon_bounds_negation_to_the_constraint_clause() {
    let grammar = analyze("Does not use the network; audits repositories locally.\n");

    let constraint = grammar
        .nodes
        .iter()
        .find(|node| node.predicate == GrammarPredicate::StatesConstraint)
        .expect("network constraint");
    assert!(constraint.negated);
    assert_eq!(constraint.modality, ClaimModality::Prohibited);

    let operation = grammar
        .nodes
        .iter()
        .find(|node| node.predicate == GrammarPredicate::PerformsOperation)
        .expect("repository audit operation");
    assert!(!operation.negated);
    assert_eq!(operation.modality, ClaimModality::Asserted);
}

#[test]
fn inline_markup_reconstructs_clause_text_without_heading_duplicates() {
    let source = "# Audit tool\n\ndoes **not** write files and `audits` repositories.\n";
    let grammar = analyze(source);
    let body_start = source.find("does").expect("body start");

    let constraint = grammar
        .nodes
        .iter()
        .find(|node| node.predicate == GrammarPredicate::StatesConstraint)
        .expect("write constraint");
    assert!(
        without_inline_markers(source_text(source, constraint)).contains("does not write files"),
        "constraint span must cover the reconstructed emphasized phrase"
    );

    let body_operation = grammar
        .nodes
        .iter()
        .find(|node| {
            node.predicate == GrammarPredicate::PerformsOperation
                && node.span.is_some_and(|span| span.byte_start >= body_start)
        })
        .expect("inline-code audit operation");
    assert!(
        without_inline_markers(source_text(source, body_operation)).contains("audits repositories"),
        "operation span must cover the reconstructed inline-code phrase"
    );

    let mut predicate_spans = BTreeSet::new();
    for node in &grammar.nodes {
        let span = node.span.expect("explicit grammar node span");
        assert!(
            predicate_spans.insert((node.predicate, span.byte_start, span.byte_end)),
            "the same heading predicate and span must not be emitted twice"
        );
    }
    assert_eq!(
        grammar
            .nodes
            .iter()
            .filter(|node| node.predicate == GrammarPredicate::DefinesIdentity)
            .count(),
        1,
        "the heading identity must be analyzed once"
    );
}

#[test]
fn source_aware_analysis_rejects_same_length_source_mismatch() {
    let scanned_source = "audits repositories.\n";
    let mismatched_source = "audits repositories!\n";
    assert_eq!(scanned_source.len(), mismatched_source.len());
    let scan = scan_document("README.md", scanned_source).expect("scan README");

    let result = analyze_readme_grammar_with_source(
        &scan,
        mismatched_source,
        &ReadmeGrammarOptions::default(),
    );

    assert_eq!(result, Err(AppealIrError::SourceMismatch));
}

#[test]
fn node_budget_keeps_partial_limit_exceeded_state() {
    let source = "A tool for teams that audits repositories and produces output.\n";
    let scan = scan_document("README.md", source).expect("scan README");
    let grammar = analyze_readme_grammar_with_source(
        &scan,
        source,
        &ReadmeGrammarOptions {
            max_nodes: 1,
            max_edges: 1,
        },
    )
    .expect("bounded grammar analysis");

    assert_eq!(
        grammar.coverage,
        CoverageStatus::Partial(CoverageIncompleteReason::LimitExceeded)
    );
    assert_eq!(grammar.nodes.len(), 1);
    assert!(grammar
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.kind == GrammarDiagnosticKind::NodeLimitExceeded));
}

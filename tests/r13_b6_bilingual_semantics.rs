use seiri_appeal::{
    analyze_document_value_coverage, update_incremental_membrane,
    verify_incremental_against_scalar, IncrementalMembraneState, IncrementalUpdateMode,
};
use seiri_codex::CodexQueryKind;
use seiri_core::{
    AnswerState, AppealIrError, ClaimAtom, ClaimAtomIR, ClaimAtomId, ClaimModality, ClaimPolarity,
    CoverageIncompleteReason, CoverageStatus, DocumentLanguage, GateKind, GrammarNode,
    GrammarNodeId, GrammarPredicate, NarrativeRelation, PatchBaseDigest, ProfileKind,
    ReadmeGrammarIR, ReadmeSection, ReadmeSectionAlignment, ReadmeSectionId, ReadmeSectionLanguage,
    ReadmeTranslationAlignmentIR, RepositoryCapabilityIR, SourceSpan, TranslationAlignmentState,
    TranslationClaimAlignment, TranslationDivergenceKind, UnderclaimOpportunityKind, UnknownReason,
    ValueDimension, CODEX_SCHEMA_VERSION, README_CLAIM_ATOM_REVISION,
};
use seiri_markdown::{analyze_readme_grammar_with_source, scan_document, ReadmeGrammarOptions};
use std::collections::BTreeSet;
use std::num::NonZeroU32;

const ALIGNED_JA_FIRST: &str = "## 日本語\nRepoSeiri はリポジトリをローカルで監査します。\n\n## English\nRepoSeiri audits repositories locally.\n";
const ALIGNED_EN_FIRST: &str = "## English\nRepoSeiri audits repositories locally.\n\n## 日本語\nRepoSeiri はリポジトリをローカルで監査します。\n";

type AtomSemantics = (
    DocumentLanguage,
    ValueDimension,
    ClaimPolarity,
    Option<String>,
    Option<String>,
    Option<String>,
    Vec<String>,
    Option<String>,
);

#[derive(Debug, Clone, Copy)]
enum AlignmentCase {
    Aligned,
    MissingCounterpart,
    Unknown,
}

fn nonzero(value: u32) -> NonZeroU32 {
    NonZeroU32::new(value).expect("non-zero test id")
}

fn grammar_id(value: u32) -> GrammarNodeId {
    GrammarNodeId::new(nonzero(value))
}

fn atom_id(value: u32) -> ClaimAtomId {
    ClaimAtomId::new(nonzero(value))
}

fn section_id(value: u32) -> ReadmeSectionId {
    ReadmeSectionId::new(nonzero(value))
}

fn translation_only_grammar(case: AlignmentCase) -> ReadmeGrammarIR {
    let japanese_span = SourceSpan::new(1, 1, 0, 10);
    let english_span = SourceSpan::new(2, 1, 10, 20);
    let nodes = vec![
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
            modality: ClaimModality::Asserted,
            negated: false,
            span: Some(english_span),
        },
    ];
    let claim_atoms = ClaimAtomIR {
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
                polarity: ClaimPolarity::Positive,
                modality: ClaimModality::Asserted,
                language: DocumentLanguage::English,
                span: Some(english_span),
            },
        ],
    };
    let sections = vec![
        ReadmeSection {
            id: section_id(1),
            language: ReadmeSectionLanguage::Japanese,
            span: japanese_span,
            claim_atoms: vec![atom_id(1)],
        },
        ReadmeSection {
            id: section_id(2),
            language: ReadmeSectionLanguage::English,
            span: english_span,
            claim_atoms: vec![atom_id(2)],
        },
    ];
    let (state, claim) = match case {
        AlignmentCase::Aligned => (
            TranslationAlignmentState::Aligned,
            TranslationClaimAlignment {
                source_claim: atom_id(1),
                counterpart_claim: Some(atom_id(2)),
                state: TranslationAlignmentState::Aligned,
                divergences: Vec::new(),
            },
        ),
        AlignmentCase::MissingCounterpart => (
            TranslationAlignmentState::Divergent,
            TranslationClaimAlignment {
                source_claim: atom_id(1),
                counterpart_claim: None,
                state: TranslationAlignmentState::Divergent,
                divergences: vec![TranslationDivergenceKind::MissingCounterpart],
            },
        ),
        AlignmentCase::Unknown => (
            TranslationAlignmentState::Unknown(UnknownReason::ParseFailed),
            TranslationClaimAlignment {
                source_claim: atom_id(1),
                counterpart_claim: Some(atom_id(2)),
                state: TranslationAlignmentState::Unknown(UnknownReason::ParseFailed),
                divergences: Vec::new(),
            },
        ),
    };
    let translation_alignment = ReadmeTranslationAlignmentIR::try_new(
        "README.md",
        PatchBaseDigest::from_bytes(b"01234567890123456789"),
        20,
        sections,
        vec![ReadmeSectionAlignment {
            source_section: section_id(1),
            counterpart_section: section_id(2),
            state,
            claims: vec![claim],
        }],
        Vec::new(),
    )
    .expect("bounded translation alignment");

    ReadmeGrammarIR::try_new_with_claim_atoms_and_translation_alignment(
        "README.md",
        CoverageStatus::Complete,
        nodes,
        Vec::new(),
        Vec::new(),
        claim_atoms,
        translation_alignment,
    )
    .expect("translation-bound README grammar")
}

fn empty_capabilities() -> RepositoryCapabilityIR {
    RepositoryCapabilityIR::try_new(CoverageStatus::Complete, Vec::new(), Vec::new(), Vec::new())
        .expect("empty complete capability IR")
}

fn analyze(source: &str) -> ReadmeGrammarIR {
    let scan = scan_document("README.md", source).expect("scan source README");
    analyze_readme_grammar_with_source(&scan, source, &ReadmeGrammarOptions::default())
        .expect("analyze bilingual README grammar")
}

fn atom_semantics(grammar: &ReadmeGrammarIR) -> Vec<AtomSemantics> {
    let mut values = grammar
        .claim_atoms
        .atoms
        .iter()
        .map(|atom| {
            (
                atom.language,
                atom.dimension,
                atom.polarity,
                atom.subject.clone(),
                atom.action.clone(),
                atom.object.clone(),
                atom.qualifiers.clone(),
                atom.condition.clone(),
            )
        })
        .collect::<Vec<_>>();
    values.sort();
    values
}

fn single_alignment(grammar: &ReadmeGrammarIR) -> &ReadmeSectionAlignment {
    assert_eq!(
        grammar.translation_alignment.alignments.len(),
        1,
        "test input must form exactly one JA/EN section pair"
    );
    &grammar.translation_alignment.alignments[0]
}

fn claim_atom(grammar: &ReadmeGrammarIR, id: ClaimAtomId) -> &ClaimAtom {
    grammar
        .claim_atoms
        .atoms
        .iter()
        .find(|atom| atom.id == id)
        .expect("translation alignment references a public claim atom")
}

type ClaimAlignmentShape = (TranslationAlignmentState, Vec<TranslationDivergenceKind>);
type SectionAlignmentShape = (TranslationAlignmentState, Vec<ClaimAlignmentShape>);

fn alignment_shape(grammar: &ReadmeGrammarIR) -> Vec<SectionAlignmentShape> {
    grammar
        .translation_alignment
        .alignments
        .iter()
        .map(|alignment| {
            (
                alignment.state,
                alignment
                    .claims
                    .iter()
                    .map(|claim| (claim.state, claim.divergences.clone()))
                    .collect(),
            )
        })
        .collect()
}

fn section_language_rank(language: ReadmeSectionLanguage) -> u8 {
    match language {
        ReadmeSectionLanguage::Japanese => 0,
        ReadmeSectionLanguage::English => 1,
        ReadmeSectionLanguage::Mixed => 2,
        ReadmeSectionLanguage::Ambiguous(_) => 3,
    }
}

#[test]
fn ja_first_and_en_second_align_at_claim_semantics() {
    let grammar = analyze(ALIGNED_JA_FIRST);
    assert_eq!(
        grammar
            .translation_alignment
            .sections
            .iter()
            .map(|section| section.language)
            .collect::<Vec<_>>(),
        vec![
            ReadmeSectionLanguage::Japanese,
            ReadmeSectionLanguage::English,
        ]
    );

    let alignment = single_alignment(&grammar);
    assert_eq!(alignment.state, TranslationAlignmentState::Aligned);
    assert_eq!(alignment.claims.len(), 1);
    assert_eq!(
        alignment.claims[0].state,
        TranslationAlignmentState::Aligned
    );
    assert!(alignment.claims[0].divergences.is_empty());
    assert!(grammar
        .edges
        .iter()
        .any(|edge| edge.relation == NarrativeRelation::Translates));

    let atoms = atom_semantics(&grammar);
    assert_eq!(atoms.len(), 2);
    for atom in atoms {
        assert_eq!(atom.1, ValueDimension::Capability);
        assert_eq!(atom.2, ClaimPolarity::Positive);
        assert_eq!(atom.3.as_deref(), Some("reposeiri"));
        assert_eq!(atom.4.as_deref(), Some("audit"));
        assert_eq!(atom.5.as_deref(), Some("repositories"));
        assert_eq!(atom.6, vec!["locally"]);
        assert_eq!(atom.7, None);
    }
}

#[test]
fn polarity_qualifier_and_condition_differences_are_typed() {
    let cases = [
        (
            "## 日本語\nRepoSeiri はリポジトリを監査しません。\n\n## English\nRepoSeiri audits repositories.\n",
            TranslationDivergenceKind::Polarity,
        ),
        (
            "## 日本語\nRepoSeiri はリポジトリをローカルで監査します。\n\n## English\nRepoSeiri audits repositories.\n",
            TranslationDivergenceKind::Qualifier,
        ),
        (
            "## 日本語\nRepoSeiri はリポジトリを障害の場合に監査します。\n\n## English\nRepoSeiri audits repositories.\n",
            TranslationDivergenceKind::Condition,
        ),
    ];

    for (source, expected) in cases {
        let grammar = analyze(source);
        let alignment = single_alignment(&grammar);
        assert_eq!(
            alignment.state,
            TranslationAlignmentState::Divergent,
            "source: {source}"
        );
        assert!(alignment.claims.iter().any(|claim| {
            claim.state == TranslationAlignmentState::Divergent
                && claim.divergences == vec![expected]
        }));
    }
}

#[test]
fn an_untranslated_claim_keeps_a_typed_missing_counterpart() {
    let grammar =
        analyze("## 日本語\nRepoSeiri はリポジトリを監査します。\n\n## English\nReference text.\n");
    let alignment = single_alignment(&grammar);
    assert_eq!(alignment.state, TranslationAlignmentState::Divergent);
    assert_eq!(alignment.claims.len(), 1);
    assert_eq!(alignment.claims[0].counterpart_claim, None);
    assert_eq!(
        alignment.claims[0].divergences,
        vec![TranslationDivergenceKind::MissingCounterpart]
    );
}

#[test]
fn mixed_inline_language_is_not_coerced_into_a_confident_claim() {
    let grammar = analyze("## 日本語 / English\nRepoSeiri は repositories を audit します。\n");
    assert_eq!(
        grammar.translation_alignment.sections[0].language,
        ReadmeSectionLanguage::Mixed
    );
    assert!(grammar.claim_atoms.atoms.is_empty());
    assert!(grammar.translation_alignment.alignments.is_empty());
    assert!(grammar
        .translation_alignment
        .unknown_reasons
        .contains(&UnknownReason::UnsupportedSyntax));
    assert_eq!(
        grammar.coverage,
        CoverageStatus::Partial(CoverageIncompleteReason::UnsupportedSyntax)
    );
}

#[test]
fn han_only_claims_remain_ambiguous_unknown() {
    let grammar = analyze("## 概要\n監査\n");
    assert_eq!(
        grammar.translation_alignment.sections[0].language,
        ReadmeSectionLanguage::Ambiguous(UnknownReason::UnsupportedSyntax)
    );
    assert!(grammar.claim_atoms.atoms.is_empty());
    assert!(grammar
        .translation_alignment
        .unknown_reasons
        .contains(&UnknownReason::UnsupportedSyntax));
    assert_eq!(
        grammar.coverage,
        CoverageStatus::Partial(CoverageIncompleteReason::UnsupportedSyntax)
    );
}

#[test]
fn reversing_language_sections_preserves_the_semantic_projection() {
    let ja_first = analyze(ALIGNED_JA_FIRST);
    let en_first = analyze(ALIGNED_EN_FIRST);

    assert_ne!(
        ja_first.translation_alignment.source_digest,
        en_first.translation_alignment.source_digest
    );
    assert_eq!(atom_semantics(&ja_first), atom_semantics(&en_first));
    assert_eq!(alignment_shape(&ja_first), alignment_shape(&en_first));
    assert_eq!(
        ja_first
            .translation_alignment
            .sections
            .iter()
            .map(|section| section_language_rank(section.language))
            .collect::<BTreeSet<_>>(),
        en_first
            .translation_alignment
            .sections
            .iter()
            .map(|section| section_language_rank(section.language))
            .collect::<BTreeSet<_>>()
    );
}

#[test]
fn inline_markdown_does_not_change_bilingual_meaning() {
    let plain = analyze(ALIGNED_JA_FIRST);
    let formatted = analyze(
        "## 日本語\n**RepoSeiri** は`リポジトリ`をローカルで**監査**します。\n\n## English\n**RepoSeiri** `audits` repositories **locally**.\n",
    );

    assert_eq!(atom_semantics(&plain), atom_semantics(&formatted));
    assert_eq!(alignment_shape(&plain), alignment_shape(&formatted));
}

#[test]
fn multiple_sections_pair_by_semantics_instead_of_position_or_distance() {
    let grammar = analyze(
        "## 日本語 一\nRepoSeiri はリポジトリを監査します。\n\n## 日本語 二\nRepoSeiri はファイルを解析します。\n\n## English Two\nRepoSeiri analyzes files.\n\n## English One\nRepoSeiri audits repositories.\n",
    );
    assert_eq!(grammar.translation_alignment.sections.len(), 4);
    assert_eq!(grammar.translation_alignment.alignments.len(), 2);
    assert_eq!(
        grammar
            .translation_alignment
            .alignments
            .iter()
            .map(|alignment| {
                (
                    alignment.source_section.get(),
                    alignment.counterpart_section.get(),
                )
            })
            .collect::<Vec<_>>(),
        vec![(1, 4), (2, 3)],
        "reversed EN sections must pair by proposition overlap, not zip order"
    );

    let mut paired_meanings = BTreeSet::new();
    for alignment in &grammar.translation_alignment.alignments {
        assert_eq!(alignment.state, TranslationAlignmentState::Aligned);
        assert_eq!(alignment.claims.len(), 1);
        let claim = &alignment.claims[0];
        assert_eq!(claim.state, TranslationAlignmentState::Aligned);
        let source = claim_atom(&grammar, claim.source_claim);
        let counterpart = claim_atom(
            &grammar,
            claim.counterpart_claim.expect("aligned counterpart claim"),
        );
        assert_eq!(source.action, counterpart.action);
        assert_eq!(source.object, counterpart.object);
        paired_meanings.insert((source.action.clone(), source.object.clone()));
    }
    assert_eq!(
        paired_meanings,
        BTreeSet::from([
            (Some("analyze".to_string()), Some("files".to_string())),
            (Some("audit".to_string()), Some("repositories".to_string()),),
        ])
    );
}

#[test]
fn an_english_only_claim_is_also_retained_as_missing() {
    let grammar = analyze(
        "## 日本語\nRepoSeiri はリポジトリを監査します。\n\n## English\nRepoSeiri audits repositories. RepoSeiri analyzes files.\n",
    );
    let alignment = single_alignment(&grammar);
    assert_eq!(alignment.state, TranslationAlignmentState::Divergent);
    assert!(alignment
        .claims
        .iter()
        .any(|claim| claim.state == TranslationAlignmentState::Aligned));

    let missing = alignment
        .claims
        .iter()
        .find(|claim| claim.divergences == vec![TranslationDivergenceKind::MissingCounterpart])
        .expect("English-only proposition remains a typed missing counterpart");
    assert_eq!(missing.counterpart_claim, None);
    let source = claim_atom(&grammar, missing.source_claim);
    assert_eq!(source.language, DocumentLanguage::English);
    assert_eq!(source.action.as_deref(), Some("analyze"));
    assert_eq!(source.object.as_deref(), Some("files"));
    let claim_section = grammar
        .translation_alignment
        .sections
        .iter()
        .find(|section| section.claim_atoms.contains(&missing.source_claim))
        .expect("section containing the directed missing claim");
    assert_eq!(claim_section.language, ReadmeSectionLanguage::English);
}

#[test]
fn appeal_projection_distinguishes_aligned_missing_and_unknown_translation() {
    let aligned =
        analyze_document_value_coverage(&translation_only_grammar(AlignmentCase::Aligned));
    assert_eq!(aligned.narrative.translation_divergences, 0);
    assert!(!aligned
        .opportunities
        .iter()
        .any(|item| item.kind == UnderclaimOpportunityKind::TranslationDivergence));

    let missing = analyze_document_value_coverage(&translation_only_grammar(
        AlignmentCase::MissingCounterpart,
    ));
    assert_eq!(missing.narrative.translation_divergences, 1);
    let translation_opportunities = missing
        .opportunities
        .iter()
        .filter(|item| item.kind == UnderclaimOpportunityKind::TranslationDivergence)
        .collect::<Vec<_>>();
    assert_eq!(translation_opportunities.len(), 1);
    assert_eq!(translation_opportunities[0].gate, GateKind::Guarded);
    assert_eq!(
        translation_opportunities[0].dimension,
        ValueDimension::Capability
    );
    assert_eq!(
        translation_opportunities[0].grammar_nodes,
        vec![grammar_id(1)]
    );

    let unknown =
        analyze_document_value_coverage(&translation_only_grammar(AlignmentCase::Unknown));
    assert_eq!(unknown.narrative.translation_divergences, 0);
    assert!(!unknown
        .opportunities
        .iter()
        .any(|item| item.kind == UnderclaimOpportunityKind::TranslationDivergence));
}

#[test]
fn translation_only_alignment_changes_invalidate_incremental_reuse() {
    let before = translation_only_grammar(AlignmentCase::Aligned);
    let capabilities = empty_capabilities();

    for after in [
        translation_only_grammar(AlignmentCase::MissingCounterpart),
        translation_only_grammar(AlignmentCase::Unknown),
    ] {
        assert_eq!(before.nodes, after.nodes);
        assert_eq!(before.claim_atoms, after.claim_atoms);
        assert_ne!(before.translation_alignment, after.translation_alignment);

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
}

#[test]
fn bilingual_ir_json_is_deterministic_and_round_trips() {
    let grammar = analyze(ALIGNED_JA_FIRST);
    let first = serde_json::to_string(&grammar).expect("serialize bilingual grammar");
    let second = serde_json::to_string(&grammar).expect("serialize bilingual grammar again");
    assert_eq!(first, second);

    let decoded: ReadmeGrammarIR = serde_json::from_str(&first).expect("deserialize grammar");
    assert_eq!(decoded, grammar);
    assert_eq!(
        serde_json::to_string(&decoded).expect("re-serialize grammar"),
        first
    );
}

#[test]
fn source_aware_bilingual_analysis_keeps_typed_source_binding() {
    let scanned_source = "## Japanese\nRepoSeiri audits repositories locally.\n";
    let mismatched_source = "## Japanese\nRepoSeiri audits repositories locally!\n";
    assert_eq!(scanned_source.len(), mismatched_source.len());
    let scan = scan_document("README.md", scanned_source).expect("scan README");

    assert_eq!(
        analyze_readme_grammar_with_source(
            &scan,
            mismatched_source,
            &ReadmeGrammarOptions::default(),
        ),
        Err(AppealIrError::SourceMismatch)
    );
}

#[test]
fn b6_keeps_the_ten_query_and_outer_schema_contract() {
    assert_eq!(CODEX_SCHEMA_VERSION, "seiri.codex.v2");
    assert_eq!(CodexQueryKind::ALL.len(), 10);
    assert_eq!(
        CodexQueryKind::ALL
            .into_iter()
            .map(CodexQueryKind::slug)
            .collect::<BTreeSet<_>>()
            .len(),
        10,
        "the ten public query slugs must remain unique"
    );
}

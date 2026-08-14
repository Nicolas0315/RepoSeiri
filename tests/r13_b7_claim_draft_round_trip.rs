use seiri_appeal::{
    update_incremental_membrane, verify_incremental_against_scalar, IncrementalMembraneState,
    IncrementalUpdateMode,
};
use seiri_codex::CodexQueryKind;
use seiri_core::{
    AppealIrError, CapabilityNodeId, ClaimAtomId, ClaimDraft, ClaimDraftIR, ClaimDraftId,
    ClaimDraftReauditDecision, ClaimDraftReauditHoldReason, ClaimDraftReauditObservation,
    ClaimDraftReauditResult, ClaimDraftSemantics, ClaimModality, ClaimMode, ClaimPolarity,
    ClaimRealization, CoverageStatus, DocumentLanguage, DocumentScan, PatchBaseDigest, ProfileKind,
    ReadmeGrammarIR, RepositoryCapabilityIR, SourceSpan, ValueDimension, CLAIM_DRAFT_REVISION,
    CODEX_SCHEMA_VERSION,
};
use seiri_markdown::{
    analyze_readme_grammar_with_source, readme_grammar_unknown_count,
    reaudit_claim_draft_candidate, scan_document_with_options, DocumentScanOptions,
    ReadmeGrammarOptions,
};
use serde_json::Value;
use std::collections::BTreeSet;
use std::num::NonZeroU32;

const SOURCE: &str = "# RepoSeiri\nRepoSeiri audits repositories locally.\n";
const CANDIDATE: &str = "# RepoSeiri\nRepoSeiri audits repositories locally.\n";

fn nonzero(value: u32) -> NonZeroU32 {
    NonZeroU32::new(value).expect("non-zero test id")
}

fn draft_id(value: u32) -> ClaimDraftId {
    ClaimDraftId::new(nonzero(value))
}

fn claim_id(value: u32) -> ClaimAtomId {
    ClaimAtomId::new(nonzero(value))
}

fn capability_id(value: u32) -> CapabilityNodeId {
    CapabilityNodeId::new(nonzero(value))
}

fn audit_semantics() -> ClaimDraftSemantics {
    ClaimDraftSemantics::try_new(
        ValueDimension::Capability,
        Some("reposeiri".to_string()),
        Some("audit".to_string()),
        Some("repositories".to_string()),
        vec!["locally".to_string()],
        None,
        ClaimPolarity::Positive,
        ClaimModality::Qualified,
        DocumentLanguage::English,
    )
    .expect("canonical audit semantics")
}

fn analyze_semantics() -> ClaimDraftSemantics {
    ClaimDraftSemantics::try_new(
        ValueDimension::Capability,
        Some("reposeiri".to_string()),
        Some("analyze".to_string()),
        Some("files".to_string()),
        Vec::new(),
        None,
        ClaimPolarity::Positive,
        ClaimModality::Qualified,
        DocumentLanguage::English,
    )
    .expect("canonical analyze semantics")
}

fn semantics_for_realization(
    semantics: ClaimDraftSemantics,
    realization_mode: ClaimMode,
) -> ClaimDraftSemantics {
    let modality = match (semantics.polarity, realization_mode) {
        (ClaimPolarity::Positive, ClaimMode::Direct) => ClaimModality::Asserted,
        (ClaimPolarity::Positive, ClaimMode::Qualified) => ClaimModality::Qualified,
        (ClaimPolarity::Negative, ClaimMode::Direct) => ClaimModality::Prohibited,
        _ => panic!("test fixture requires a representable realization mode"),
    };
    ClaimDraftSemantics::try_new(
        semantics.dimension,
        semantics.subject,
        semantics.action,
        semantics.object,
        semantics.qualifiers,
        semantics.condition,
        semantics.polarity,
        modality,
        semantics.language,
    )
    .expect("realization-consistent draft semantics")
}

fn draft(
    id: u32,
    semantics: ClaimDraftSemantics,
    realization_mode: ClaimMode,
    claim_ceiling: ClaimMode,
) -> ClaimDraft {
    let semantics = semantics_for_realization(semantics, realization_mode);
    ClaimDraft::try_new(
        draft_id(id),
        SourceSpan::new(2, 1, 13, SOURCE.len()),
        vec![claim_id(id)],
        vec![capability_id(id)],
        semantics,
        realization_mode,
        claim_ceiling,
    )
    .expect("bounded claim draft")
}

fn draft_ir(baseline_unknown_count: usize, claim_ceiling: ClaimMode) -> ClaimDraftIR {
    ClaimDraftIR::try_new(
        "README.md",
        PatchBaseDigest::from_bytes(SOURCE.as_bytes()),
        SOURCE.len(),
        baseline_unknown_count,
        vec![draft(
            1,
            audit_semantics(),
            ClaimMode::Qualified,
            claim_ceiling,
        )],
    )
    .expect("source-bound draft IR")
}

fn parsed_draft_fixture(
    realization_mode: ClaimMode,
    claim_ceiling: ClaimMode,
) -> (DocumentScan, ClaimDraftIR) {
    let scan_options = DocumentScanOptions::derived_for_source(SOURCE.len());
    let document = scan_document_with_options("README.md", SOURCE, &scan_options)
        .expect("scan baseline README");
    let grammar =
        analyze_readme_grammar_with_source(&document, SOURCE, &ReadmeGrammarOptions::default())
            .expect("analyze baseline README");
    let atom = grammar
        .claim_atoms
        .atoms
        .iter()
        .find(|atom| atom.action.as_deref() == Some("audit"))
        .expect("baseline audit atom");
    let node = grammar
        .nodes
        .iter()
        .find(|node| node.id == atom.grammar_node)
        .expect("baseline audit grammar node");
    let parsed_mode = ClaimRealization::try_new(atom, node)
        .expect("valid claim realization")
        .mode;
    assert_eq!(parsed_mode, ClaimMode::Direct);
    let semantics = ClaimDraftSemantics::try_new(
        atom.dimension,
        atom.subject.clone(),
        atom.action.clone(),
        atom.object.clone(),
        atom.qualifiers.clone(),
        atom.condition.clone(),
        atom.polarity,
        atom.modality,
        atom.language,
    )
    .expect("candidate-compatible draft semantics");
    let semantics = semantics_for_realization(semantics, realization_mode);
    let draft = ClaimDraft::try_new(
        draft_id(1),
        SourceSpan::new(1, 1, 0, SOURCE.len()),
        vec![atom.id],
        vec![capability_id(1)],
        semantics,
        realization_mode,
        claim_ceiling,
    )
    .expect("candidate-compatible draft");
    let drafts = ClaimDraftIR::try_new(
        "README.md",
        document.base().digest(),
        document.source_bytes(),
        readme_grammar_unknown_count(&grammar),
        vec![draft],
    )
    .expect("candidate-compatible draft IR");
    (document, drafts)
}

#[test]
fn claim_draft_rejects_semantic_realization_mode_mismatch() {
    assert_eq!(
        ClaimDraft::try_new(
            draft_id(1),
            SourceSpan::new(2, 1, 13, SOURCE.len()),
            vec![claim_id(1)],
            vec![capability_id(1)],
            audit_semantics(),
            ClaimMode::Direct,
            ClaimMode::Direct,
        ),
        Err(AppealIrError::ClaimDraftRealizationMismatch(draft_id(1)))
    );
}

fn observation(
    drafts: &ClaimDraftIR,
    candidate_path: &str,
    candidate_unknown_count: usize,
    candidate_semantics: Option<ClaimDraftSemantics>,
    candidate_mode: ClaimMode,
    scope_preserved: bool,
) -> ClaimDraftReauditObservation {
    ClaimDraftReauditObservation::new(
        drafts.source_digest,
        drafts.source_byte_len,
        candidate_path,
        PatchBaseDigest::from_bytes(CANDIDATE.as_bytes()),
        CANDIDATE.len(),
        candidate_unknown_count,
        candidate_semantics,
        candidate_mode,
        scope_preserved,
    )
}

fn evaluate(
    drafts: &ClaimDraftIR,
    observation: ClaimDraftReauditObservation,
) -> ClaimDraftReauditResult {
    ClaimDraftReauditResult::evaluate(drafts, draft_id(1), observation)
        .expect("known draft re-audit")
}

fn assert_no_prose_or_write_keys(value: &Value) {
    const FORBIDDEN: [&str; 9] = [
        "body",
        "candidate_source",
        "edit",
        "markdown",
        "prose",
        "rendered",
        "replacement",
        "text",
        "writes_files",
    ];
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                assert!(
                    !FORBIDDEN.contains(&key.as_str()),
                    "claim draft wire must remain prose-free and non-mutating: {key}"
                );
                assert_no_prose_or_write_keys(value);
            }
        }
        Value::Array(values) => {
            for value in values {
                assert_no_prose_or_write_keys(value);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

#[test]
fn claim_draft_ir_is_prose_free_canonical_and_deterministic() {
    let drafts = ClaimDraftIR::try_new(
        "README.md",
        PatchBaseDigest::from_bytes(SOURCE.as_bytes()),
        SOURCE.len(),
        0,
        vec![
            draft(
                1,
                audit_semantics(),
                ClaimMode::Qualified,
                ClaimMode::Direct,
            ),
            draft(
                2,
                analyze_semantics(),
                ClaimMode::Qualified,
                ClaimMode::Direct,
            ),
        ],
    )
    .expect("canonical multi-draft IR");
    assert_eq!(drafts.semantic_revision, CLAIM_DRAFT_REVISION);
    assert_eq!(drafts.drafts.len(), 2);

    let first = serde_json::to_string(&drafts).expect("serialize draft IR");
    let second = serde_json::to_string(&drafts).expect("serialize draft IR again");
    assert_eq!(first, second);
    let value = serde_json::to_value(&drafts).expect("draft JSON value");
    assert_no_prose_or_write_keys(&value);

    let decoded: ClaimDraftIR = serde_json::from_str(&first).expect("deserialize draft IR");
    assert_eq!(decoded, drafts);
    assert_eq!(
        serde_json::to_string(&decoded).expect("re-serialize draft IR"),
        first
    );

    let mut reversed = drafts.drafts.clone();
    reversed.reverse();
    assert_eq!(
        ClaimDraftIR::try_new(
            "README.md",
            drafts.source_digest,
            drafts.source_byte_len,
            0,
            reversed,
        ),
        Err(AppealIrError::NonCanonicalClaimDrafts)
    );
}

#[test]
fn stale_source_digest_holds_before_candidate_checks() {
    let drafts = draft_ir(0, ClaimMode::Qualified);
    let stale = ClaimDraftReauditObservation::new(
        PatchBaseDigest::from_bytes(b"stale source"),
        drafts.source_byte_len,
        "../README.md",
        PatchBaseDigest::from_bytes(b"candidate"),
        9,
        99,
        Some(analyze_semantics()),
        ClaimMode::Direct,
        false,
    );
    let result = evaluate(&drafts, stale);

    assert_eq!(result.decision, ClaimDraftReauditDecision::Held);
    assert_eq!(
        result.holds,
        vec![ClaimDraftReauditHoldReason::StaleSourceDigest]
    );
    assert!(!result.candidate_checks_performed);
    assert!(!result.writes_files());
}

#[test]
fn unknown_increase_is_held_instead_of_collapsed() {
    let drafts = draft_ir(1, ClaimMode::Qualified);
    let result = evaluate(
        &drafts,
        observation(
            &drafts,
            "README.md",
            2,
            Some(audit_semantics()),
            ClaimMode::Qualified,
            true,
        ),
    );

    assert_eq!(result.decision, ClaimDraftReauditDecision::Held);
    assert_eq!(
        result.holds,
        vec![ClaimDraftReauditHoldReason::UnknownIncreased]
    );
    assert!(result.candidate_checks_performed);
}

#[test]
fn evidence_ceiling_excess_is_held() {
    let drafts = draft_ir(0, ClaimMode::Qualified);
    let result = evaluate(
        &drafts,
        observation(
            &drafts,
            "README.md",
            0,
            Some(audit_semantics()),
            ClaimMode::Direct,
            true,
        ),
    );

    assert_eq!(result.decision, ClaimDraftReauditDecision::Held);
    assert_eq!(
        result.holds,
        vec![ClaimDraftReauditHoldReason::CeilingExceeded]
    );
}

#[test]
fn semantic_drift_is_held() {
    let drafts = draft_ir(0, ClaimMode::Qualified);
    let result = evaluate(
        &drafts,
        observation(
            &drafts,
            "README.md",
            0,
            Some(analyze_semantics()),
            ClaimMode::Qualified,
            true,
        ),
    );

    assert_eq!(result.decision, ClaimDraftReauditDecision::Held);
    assert_eq!(
        result.holds,
        vec![ClaimDraftReauditHoldReason::SemanticDrift]
    );
}

#[test]
fn scope_and_path_escape_are_held() {
    let drafts = draft_ir(0, ClaimMode::Qualified);
    for (path, scope_preserved) in [("../README.md", true), ("README.md", false)] {
        let result = evaluate(
            &drafts,
            observation(
                &drafts,
                path,
                0,
                Some(audit_semantics()),
                ClaimMode::Qualified,
                scope_preserved,
            ),
        );
        assert_eq!(result.decision, ClaimDraftReauditDecision::Held);
        assert_eq!(result.holds, vec![ClaimDraftReauditHoldReason::ScopeEscape]);
    }
}

#[test]
fn exact_round_trip_is_accepted_and_never_writes() {
    let drafts = draft_ir(0, ClaimMode::Qualified);
    let result = evaluate(
        &drafts,
        observation(
            &drafts,
            "README.md",
            0,
            Some(audit_semantics()),
            ClaimMode::Qualified,
            true,
        ),
    );

    assert_eq!(result.decision, ClaimDraftReauditDecision::Accepted);
    assert!(result.holds.is_empty());
    assert!(result.candidate_checks_performed);
    assert!(!result.writes_files());

    let first = serde_json::to_string(&result).expect("serialize accepted result");
    let decoded: ClaimDraftReauditResult =
        serde_json::from_str(&first).expect("deserialize accepted result");
    assert_eq!(decoded, result);
    assert_eq!(
        serde_json::to_string(&decoded).expect("re-serialize accepted result"),
        first
    );
}

#[test]
fn simultaneous_holds_remain_canonical_and_deterministic() {
    let drafts = draft_ir(0, ClaimMode::Qualified);
    let result = evaluate(
        &drafts,
        observation(
            &drafts,
            "docs/README.md",
            3,
            Some(analyze_semantics()),
            ClaimMode::Direct,
            false,
        ),
    );

    assert_eq!(
        result.holds,
        vec![
            ClaimDraftReauditHoldReason::UnknownIncreased,
            ClaimDraftReauditHoldReason::CeilingExceeded,
            ClaimDraftReauditHoldReason::SemanticDrift,
            ClaimDraftReauditHoldReason::ScopeEscape,
        ]
    );
    let first = serde_json::to_string(&result).expect("serialize held result");
    let second = serde_json::to_string(&result).expect("serialize held result again");
    assert_eq!(first, second);
}

#[test]
fn public_candidate_reparse_accepts_exact_and_canonical_inline_round_trips() {
    let (document, drafts) = parsed_draft_fixture(ClaimMode::Direct, ClaimMode::Direct);
    let formatted = "# RepoSeiri\n**RepoSeiri** `audits` repositories **locally**.\n";

    for candidate in [SOURCE, formatted] {
        let result = reaudit_claim_draft_candidate(
            &drafts,
            draft_id(1),
            &document,
            "README.md",
            candidate,
            &DocumentScanOptions::derived_for_source(candidate.len()),
            &ReadmeGrammarOptions::default(),
        )
        .expect("in-memory candidate re-audit");

        assert_eq!(result.decision, ClaimDraftReauditDecision::Accepted);
        assert!(result.holds.is_empty());
        assert!(result.candidate_checks_performed);
        assert!(!result.writes_files());
        assert_eq!(
            result.candidate_source_digest,
            PatchBaseDigest::from_bytes(candidate.as_bytes())
        );
        assert_eq!(result.candidate_source_byte_len, candidate.len());
        assert_no_prose_or_write_keys(
            &serde_json::to_value(&result).expect("candidate result JSON"),
        );
    }
}

#[test]
fn public_candidate_reaudit_checks_staleness_before_reparse_budget() {
    let (_, drafts) = parsed_draft_fixture(ClaimMode::Direct, ClaimMode::Direct);
    let stale_source = "# Different source\n";
    let stale_document = scan_document_with_options(
        "README.md",
        stale_source,
        &DocumentScanOptions::derived_for_source(stale_source.len()),
    )
    .expect("scan stale observed source");

    let result = reaudit_claim_draft_candidate(
        &drafts,
        draft_id(1),
        &stale_document,
        "README.md",
        SOURCE,
        &DocumentScanOptions {
            max_source_bytes: 1,
            max_events: 1,
            max_diagnostics: 1,
        },
        &ReadmeGrammarOptions::default(),
    )
    .expect("stale hold precedes candidate scanning");

    assert_eq!(result.decision, ClaimDraftReauditDecision::Held);
    assert_eq!(
        result.holds,
        vec![ClaimDraftReauditHoldReason::StaleSourceDigest]
    );
    assert!(!result.candidate_checks_performed);
}

#[test]
fn parsed_candidate_unknown_increase_is_held() {
    let (document, drafts) = parsed_draft_fixture(ClaimMode::Direct, ClaimMode::Direct);
    let candidate = "## 日本語 / English\nRepoSeiri は repositories を audit します。\n";
    let result = reaudit_claim_draft_candidate(
        &drafts,
        draft_id(1),
        &document,
        "README.md",
        candidate,
        &DocumentScanOptions::derived_for_source(candidate.len()),
        &ReadmeGrammarOptions::default(),
    )
    .expect("ambiguous candidate re-audit");

    assert_eq!(result.decision, ClaimDraftReauditDecision::Held);
    assert!(result
        .holds
        .contains(&ClaimDraftReauditHoldReason::UnknownIncreased));
    assert!(result.candidate_unknown_count > result.baseline_unknown_count);
    assert!(result.candidate_checks_performed);
}

#[test]
fn parsed_candidate_cannot_exceed_the_draft_evidence_ceiling() {
    let (document, drafts) = parsed_draft_fixture(ClaimMode::Qualified, ClaimMode::Qualified);
    let result = reaudit_claim_draft_candidate(
        &drafts,
        draft_id(1),
        &document,
        "README.md",
        SOURCE,
        &DocumentScanOptions::derived_for_source(SOURCE.len()),
        &ReadmeGrammarOptions::default(),
    )
    .expect("ceiling-bounded candidate re-audit");

    assert_eq!(result.decision, ClaimDraftReauditDecision::Held);
    assert_eq!(
        result.holds,
        vec![
            ClaimDraftReauditHoldReason::CeilingExceeded,
            ClaimDraftReauditHoldReason::SemanticDrift,
        ]
    );
    assert_eq!(result.candidate_mode, ClaimMode::Direct);
}

#[test]
fn parsed_candidate_semantic_drift_and_scope_escape_are_held() {
    let (document, drafts) = parsed_draft_fixture(ClaimMode::Direct, ClaimMode::Direct);
    let drifted = "# RepoSeiri\nRepoSeiri audits repositories offline.\n";
    let drift = reaudit_claim_draft_candidate(
        &drafts,
        draft_id(1),
        &document,
        "README.md",
        drifted,
        &DocumentScanOptions::derived_for_source(drifted.len()),
        &ReadmeGrammarOptions::default(),
    )
    .expect("semantic drift candidate re-audit");
    assert_eq!(
        drift.holds,
        vec![ClaimDraftReauditHoldReason::SemanticDrift]
    );

    let escaped = reaudit_claim_draft_candidate(
        &drafts,
        draft_id(1),
        &document,
        "docs/README.md",
        SOURCE,
        &DocumentScanOptions::derived_for_source(SOURCE.len()),
        &ReadmeGrammarOptions::default(),
    )
    .expect("scope escape candidate re-audit");
    assert_eq!(
        escaped.holds,
        vec![ClaimDraftReauditHoldReason::ScopeEscape]
    );
}

#[test]
fn draft_reaudit_does_not_change_incremental_membrane_inputs() {
    let grammar = ReadmeGrammarIR::try_new(
        "README.md",
        CoverageStatus::Complete,
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .expect("empty grammar");
    let capabilities = RepositoryCapabilityIR::try_new(
        CoverageStatus::Complete,
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .expect("empty capabilities");
    let state = IncrementalMembraneState::from_scalar(&grammar, &capabilities, ProfileKind::Common)
        .expect("valid scalar state");

    let drafts = draft_ir(0, ClaimMode::Qualified);
    let result = evaluate(
        &drafts,
        observation(
            &drafts,
            "README.md",
            0,
            Some(audit_semantics()),
            ClaimMode::Qualified,
            true,
        ),
    );
    assert_eq!(result.decision, ClaimDraftReauditDecision::Accepted);

    let update =
        update_incremental_membrane(&state, &grammar, &capabilities, ProfileKind::Common, &[])
            .expect("valid incremental update");
    assert_eq!(update.mode, IncrementalUpdateMode::Reused);
    assert!(
        verify_incremental_against_scalar(&update, &grammar, &capabilities, ProfileKind::Common,)
            .expect("valid scalar verification")
            .equivalent
    );
}

#[test]
fn b7_keeps_the_ten_query_and_outer_schema_contract() {
    assert_eq!(CODEX_SCHEMA_VERSION, "seiri.codex.v2");
    assert_eq!(CodexQueryKind::ALL.len(), 10);
    assert_eq!(
        CodexQueryKind::ALL
            .into_iter()
            .map(CodexQueryKind::slug)
            .collect::<BTreeSet<_>>()
            .len(),
        10
    );
}

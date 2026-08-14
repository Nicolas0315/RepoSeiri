use seiri_codex::{CodexQueryKind, CodexView};
use seiri_core::{
    AnswerState, AppealIrError, CapabilityNodeId, ClaimAtomId, ClaimDraft, ClaimDraftIR,
    ClaimDraftId, ClaimDraftPlanState, ClaimDraftReauditDecision, ClaimDraftReauditHoldReason,
    ClaimDraftReauditObservation, ClaimDraftReauditResult, ClaimDraftSemantics, ClaimModality,
    ClaimMode, ClaimPolarity, ContractManifest, ContractValidationError, CoverageIncompleteReason,
    CoverageStatus, Digest32, DocumentLanguage, PatchBaseDigest, PatchPlan, PortableAuditSnapshot,
    PortableContractError, ProfileKind, ReadmeGrammarIR, SemanticRevisionKey, SourceSessionDigest,
    SourceSpan, UnknownReason, ValueDimension, CODEX_SCHEMA_VERSION,
};
use seiri_markdown::{analyze_readme_grammar_with_source, scan_document, ReadmeGrammarOptions};
use serde_json::json;
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};

const SOURCE: &str = "# RepoSeiri\nRepoSeiri audits repositories locally.\n";

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
    .expect("canonical qualified semantics")
}

fn draft(span: SourceSpan) -> ClaimDraft {
    ClaimDraft::try_new(
        draft_id(1),
        span,
        vec![claim_id(1)],
        vec![capability_id(1)],
        audit_semantics(),
        ClaimMode::Qualified,
        ClaimMode::Qualified,
    )
    .expect("canonical claim draft")
}

fn draft_ir() -> ClaimDraftIR {
    ClaimDraftIR::try_new(
        "README.md",
        PatchBaseDigest::from_bytes(SOURCE.as_bytes()),
        SOURCE.len(),
        0,
        vec![draft(SourceSpan::new(2, 1, 13, SOURCE.len()))],
    )
    .expect("source-bound claim draft IR")
}

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/readme-route-repo")
}

fn safe_plan_fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/safe-plan-repo")
}

#[test]
fn semantic_revision_tampering_is_reported_by_typed_contract_keys() {
    let current = ContractManifest::current("test");
    assert_eq!(current.schema_version, "seiri.contract.v6");
    assert_eq!(SemanticRevisionKey::ALL.len(), 31);
    assert_eq!(current.semantic_revisions.entries().len(), 31);
    assert_eq!(
        current.semantic_revisions.entries().map(|entry| entry.key),
        SemanticRevisionKey::ALL
    );

    for key in SemanticRevisionKey::ALL {
        let field = serde_json::to_value(key)
            .expect("serialize semantic revision key")
            .as_str()
            .expect("snake-case revision key")
            .to_string();
        let mut wire = serde_json::to_value(&current).expect("contract manifest JSON");
        wire["semantic_revisions"][&field] = json!("tampered.semantic-revision");
        let tampered: ContractManifest =
            serde_json::from_value(wire).expect("structurally valid tampered contract");
        assert_eq!(
            tampered.validate_current(),
            Err(ContractValidationError::SemanticRevision(key)),
            "tampered revision was accepted: {field}"
        );
    }
}

#[test]
fn claim_draft_paths_reject_absolute_parent_backslash_and_drive_forms() {
    for path in [
        "/README.md",
        "../README.md",
        "docs/../README.md",
        "docs\\README.md",
        "C:/README.md",
        "C:\\README.md",
        "//server/share/README.md",
    ] {
        assert_eq!(
            ClaimDraftIR::try_new(path, PatchBaseDigest::from_bytes(&[]), 0, 0, Vec::new(),),
            Err(AppealIrError::InvalidClaimDraftPath),
            "non-portable path accepted: {path}"
        );
    }
}

#[test]
fn source_spans_reject_reversed_wire_ranges_and_out_of_bounds_drafts() {
    let reversed = serde_json::from_value::<SourceSpan>(json!({
        "line": 1,
        "column": 1,
        "byte_start": 4,
        "byte_end": 3,
    }));
    assert!(reversed.is_err());

    let outside = draft(SourceSpan::new(1, 1, 0, SOURCE.len() + 1));
    assert_eq!(
        ClaimDraftIR::try_new(
            "README.md",
            PatchBaseDigest::from_bytes(SOURCE.as_bytes()),
            SOURCE.len(),
            0,
            vec![outside],
        ),
        Err(AppealIrError::ClaimDraftSpanOutOfBounds(draft_id(1)))
    );
}

#[test]
fn claim_draft_revision_digest_and_utf8_boundaries_are_revalidated() {
    let canonical = draft_ir();
    canonical
        .validate_against_source("README.md", SOURCE)
        .expect("exact source binding");

    let mut tampered_revision = canonical.clone();
    tampered_revision.semantic_revision = "tampered.claim-draft".to_string();
    assert_eq!(
        tampered_revision.validate(),
        Err(AppealIrError::ClaimDraftSemanticRevisionMismatch)
    );
    let mut revision_wire = serde_json::to_value(&canonical).expect("claim draft JSON");
    revision_wire["semantic_revision"] = json!("tampered.claim-draft");
    assert!(serde_json::from_value::<ClaimDraftIR>(revision_wire).is_err());

    assert_eq!(
        canonical.validate_against_source("docs/README.md", SOURCE),
        Err(AppealIrError::ClaimDraftSourcePathMismatch)
    );
    assert_eq!(
        canonical.validate_against_source("README.md", b"short"),
        Err(AppealIrError::ClaimDraftSourceLengthMismatch)
    );
    let same_length_stale = vec![b'x'; SOURCE.len()];
    assert_eq!(
        canonical.validate_against_source("README.md", same_length_stale),
        Err(AppealIrError::ClaimDraftSourceDigestMismatch)
    );

    let invalid_utf8 = [0xff_u8];
    let invalid_utf8_ir = ClaimDraftIR::try_new(
        "README.md",
        PatchBaseDigest::from_bytes(&invalid_utf8),
        invalid_utf8.len(),
        0,
        vec![draft(SourceSpan::new(1, 1, 0, 1))],
    )
    .expect("structurally bounded invalid UTF-8 fixture");
    assert_eq!(
        invalid_utf8_ir.validate_against_source("README.md", invalid_utf8),
        Err(AppealIrError::ClaimDraftSourceNotUtf8)
    );

    let multibyte = "éx";
    let split_codepoint = ClaimDraftIR::try_new(
        "README.md",
        PatchBaseDigest::from_bytes(multibyte.as_bytes()),
        multibyte.len(),
        0,
        vec![draft(SourceSpan::new(1, 2, 1, 2))],
    )
    .expect("structurally bounded split-codepoint fixture");
    assert_eq!(
        split_codepoint.validate_against_source("README.md", multibyte),
        Err(AppealIrError::ClaimDraftSpanNotCharBoundary(draft_id(1)))
    );
}

#[test]
fn stale_digest_dominates_scope_checks_and_scope_escape_is_portable() {
    let drafts = draft_ir();
    let stale = ClaimDraftReauditObservation::new(
        PatchBaseDigest::from_bytes(b"stale"),
        drafts.source_byte_len,
        "../README.md",
        PatchBaseDigest::from_bytes(b"candidate"),
        9,
        99,
        None,
        ClaimMode::Omitted,
        false,
    );
    let stale =
        ClaimDraftReauditResult::evaluate(&drafts, draft_id(1), stale).expect("typed stale result");
    assert_eq!(stale.decision, ClaimDraftReauditDecision::Held);
    assert_eq!(
        stale.holds,
        vec![ClaimDraftReauditHoldReason::StaleSourceDigest]
    );
    assert!(!stale.candidate_checks_performed);
    assert!(!stale.writes_files());
    stale
        .validate_against_drafts(&drafts)
        .expect("stale result remains bound to its draft IR");

    let first = serde_json::to_string(&stale).expect("serialize stale result");
    let second = serde_json::to_string(&stale).expect("serialize stale result again");
    assert_eq!(first, second);
    let decoded: ClaimDraftReauditResult =
        serde_json::from_str(&first).expect("validated stale result deserialize");
    assert_eq!(decoded, stale);

    let mut revision_tamper = stale.clone();
    revision_tamper.semantic_revision = "tampered.claim-draft-reaudit".to_string();
    assert_eq!(
        revision_tamper.validate(),
        Err(AppealIrError::NonCanonicalClaimDraftReaudit)
    );
    assert!(serde_json::from_value::<ClaimDraftReauditResult>(
        serde_json::to_value(revision_tamper).expect("tampered result JSON")
    )
    .is_err());

    let mut binding_tamper = stale.clone();
    binding_tamper.claim_ceiling = ClaimMode::Direct;
    assert_eq!(
        binding_tamper.validate_against_drafts(&drafts),
        Err(AppealIrError::ClaimDraftReauditBindingMismatch(draft_id(1)))
    );

    for candidate_path in [
        "../README.md",
        "/README.md",
        "docs\\README.md",
        "C:/README.md",
    ] {
        let observation = ClaimDraftReauditObservation::new(
            drafts.source_digest,
            drafts.source_byte_len,
            candidate_path,
            PatchBaseDigest::from_bytes(SOURCE.as_bytes()),
            SOURCE.len(),
            0,
            Some(audit_semantics()),
            ClaimMode::Qualified,
            true,
        );
        let escaped = ClaimDraftReauditResult::evaluate(&drafts, draft_id(1), observation)
            .expect("typed portable scope result");
        assert_eq!(
            escaped.holds,
            vec![ClaimDraftReauditHoldReason::ScopeEscape],
            "portable scope escape was not held: {candidate_path}"
        );
        assert!(!escaped.writes_files());
    }
}

#[test]
fn unknown_state_and_partial_coverage_survive_deterministic_json() {
    let document = scan_document("README.md", SOURCE).expect("scan README");
    let mut grammar =
        analyze_readme_grammar_with_source(&document, SOURCE, &ReadmeGrammarOptions::default())
            .expect("analyze README");
    grammar.nodes[0].state = AnswerState::Unknown(UnknownReason::UnsupportedSyntax);
    grammar.coverage = CoverageStatus::Partial(CoverageIncompleteReason::UnsupportedSyntax);
    let grammar = ReadmeGrammarIR::try_new_with_claim_atoms_and_translation_alignment(
        grammar.path.clone(),
        grammar.coverage,
        grammar.nodes,
        grammar.edges,
        grammar.diagnostics,
        grammar.claim_atoms,
        grammar.translation_alignment,
    )
    .expect("canonical partial grammar");

    let first = serde_json::to_string(&grammar).expect("serialize grammar");
    let second = serde_json::to_string(&grammar).expect("serialize grammar again");
    assert_eq!(first, second);
    let decoded: ReadmeGrammarIR = serde_json::from_str(&first).expect("deserialize grammar");
    assert_eq!(decoded, grammar);
    assert!(decoded
        .nodes
        .iter()
        .any(|node| { node.state == AnswerState::Unknown(UnknownReason::UnsupportedSyntax) }));
    assert_eq!(
        decoded.coverage,
        CoverageStatus::Partial(CoverageIncompleteReason::UnsupportedSyntax)
    );
}

#[test]
fn portable_snapshot_rejects_scope_and_digest_tampering_deterministically() {
    let analysis = seiri_report::audit_repository_with_profile(fixture(), ProfileKind::Cli)
        .expect("audit fixture");
    let snapshot = seiri_report::portable_audit_snapshot(&analysis).expect("portable snapshot");
    snapshot.validate().expect("canonical portable snapshot");
    seiri_delta::validate_portable_snapshot(&snapshot).expect("verified portable snapshot");

    let first = serde_json::to_string(&snapshot).expect("serialize portable snapshot");
    let second = serde_json::to_string(&snapshot).expect("serialize portable snapshot again");
    assert_eq!(first, second);
    let decoded: PortableAuditSnapshot =
        serde_json::from_str(&first).expect("validated snapshot deserialize");
    assert_eq!(decoded, snapshot);

    let mut stale_session = snapshot.clone();
    stale_session.digest.source_session = SourceSessionDigest::new(Digest32::new([7; 32]));
    assert_eq!(
        stale_session.validate(),
        Err(PortableContractError::SourceSessionDigestMismatch)
    );

    for path in [
        "/README.md",
        "../README.md",
        "docs/../README.md",
        "docs\\README.md",
        "C:/README.md",
    ] {
        let mut escaped = snapshot.clone();
        let mut document = escaped.documents[0].clone();
        document.path = path.to_string();
        escaped.documents = vec![document];
        assert_eq!(
            escaped.validate(),
            Err(PortableContractError::InvalidRepositoryPath),
            "portable snapshot accepted scope escape: {path}"
        );
        let escaped_wire = serde_json::to_value(&escaped).expect("escaped snapshot JSON");
        assert!(serde_json::from_value::<PortableAuditSnapshot>(escaped_wire).is_err());
    }

    let mut record_tamper = snapshot.clone();
    record_tamper.documents[0].digest = Digest32::new([8; 32]);
    assert_eq!(
        seiri_delta::validate_portable_snapshot(&record_tamper),
        Err(seiri_delta::DeltaError::RecordDigestMismatch)
    );

    let mut aggregate_tamper = snapshot;
    aggregate_tamper.digest.documents = Digest32::new([9; 32]);
    assert_eq!(
        seiri_delta::validate_portable_snapshot(&aggregate_tamper),
        Err(seiri_delta::DeltaError::SnapshotDigestMismatch)
    );
}

#[test]
fn patch_plan_rejects_scope_and_write_tampering_while_claim_drafts_stay_prose_free() {
    let analysis = seiri_report::audit_repository_subtree_with_profile(
        safe_plan_fixture(),
        ProfileKind::Common,
    )
    .expect("audit safe-plan fixture");
    let plan = seiri_planner::plan_patches(&analysis);
    plan.validate().expect("canonical patch plan");
    let operation = plan.operations.first().expect("safe docs operation");
    assert_eq!(operation.target_path, "docs/");

    for path in ["/docs/", "../docs/", "docs/../", "docs\\", "C:/docs/"] {
        let mut escaped = plan.clone();
        escaped.operations[0].target_path = path.to_string();
        assert_eq!(
            escaped.validate(),
            Err(PortableContractError::InvalidPatchTargetPath),
            "patch plan accepted scope escape: {path}"
        );
        let escaped_wire = serde_json::to_value(&escaped).expect("escaped plan JSON");
        assert!(serde_json::from_value::<PatchPlan>(escaped_wire).is_err());
    }

    let mut writes = plan.clone();
    writes.writes_files = true;
    assert_eq!(
        writes.validate(),
        Err(PortableContractError::PatchPlanWritesFiles)
    );
    assert!(serde_json::from_value::<PatchPlan>(
        serde_json::to_value(&writes).expect("write-tampered plan JSON")
    )
    .is_err());

    let claim_plan = PatchPlan {
        claim_drafts: draft_ir(),
        claim_draft_state: ClaimDraftPlanState::Ready,
        ..PatchPlan::default()
    };
    claim_plan.validate().expect("prose-free claim-draft plan");
    assert!(!claim_plan.writes_files);
    let first = serde_json::to_string(&claim_plan).expect("serialize claim-draft plan");
    let second = serde_json::to_string(&claim_plan).expect("serialize claim-draft plan again");
    assert_eq!(first, second);
    let wire = serde_json::to_value(&claim_plan).expect("claim-draft plan JSON");
    assert_eq!(wire["writes_files"], false);
    assert!(wire["claim_drafts"].get("writes_files").is_none());
    assert!(wire["claim_drafts"].get("body").is_none());
    let decoded: PatchPlan = serde_json::from_value(wire).expect("validated plan deserialize");
    assert_eq!(decoded, claim_plan);
}

#[test]
fn all_ten_queries_keep_the_exact_outer_wire_and_no_write_plan() {
    assert_eq!(CODEX_SCHEMA_VERSION, "seiri.codex.v2");
    assert_eq!(CodexQueryKind::ALL.len(), 10);

    let analysis = seiri_report::audit_repository_with_profile(fixture(), ProfileKind::Cli)
        .expect("audit fixture");
    let plan = seiri_planner::plan_patches(&analysis);
    assert!(!plan.writes_files);
    let view = CodexView::new(&analysis, &plan, None);

    for kind in CodexQueryKind::ALL {
        let wire = serde_json::to_value(view.query(kind)).expect("Codex query JSON");
        assert_eq!(wire["schema_version"], "seiri.codex.v2");
        assert_eq!(
            wire.as_object()
                .expect("outer query object")
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            [
                "boundary",
                "profile",
                "query",
                "repo_root",
                "schema_version",
            ]
        );
    }
}

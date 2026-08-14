use seiri_core::{
    ClaimDraftPlanHoldReason, ClaimDraftPlanState, ContentClaim, EvidenceId, PatchHoldReason,
    ProfileKind, RouteKind,
};
use seiri_delta::{evidence_fingerprints_for_ids, DeltaError};
use std::path::{Path, PathBuf};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/readme-route-repo")
}

fn missing_id(analysis: &seiri_core::RepositoryAnalysis) -> EvidenceId {
    EvidenceId::from_ordinal(analysis.evidence_kernel.facts().len() + 10_000)
        .expect("bounded missing evidence id")
}

#[test]
fn requested_missing_evidence_is_a_typed_error_not_an_empty_result() {
    let analysis = seiri_report::audit_repository_with_profile(fixture(), ProfileKind::Common)
        .expect("audit fixture");
    assert_eq!(
        evidence_fingerprints_for_ids(&analysis, &[missing_id(&analysis)]),
        Err(DeltaError::MissingEvidenceReference)
    );
}

#[test]
fn planner_projects_invalid_evidence_contract_to_a_typed_hold() {
    let mut analysis = seiri_report::audit_repository_with_profile(fixture(), ProfileKind::Common)
        .expect("audit fixture");
    let planned_routes = [
        RouteKind::Docs,
        RouteKind::Quickstart,
        RouteKind::Support,
        RouteKind::Intake,
        RouteKind::Contributing,
        RouteKind::Security,
        RouteKind::Release,
        RouteKind::Lifecycle,
        RouteKind::Governance,
        RouteKind::License,
        RouteKind::Automation,
        RouteKind::Ownership,
        RouteKind::Hygiene,
    ];
    let claim_index = analysis
        .claims
        .iter()
        .position(|claim| planned_routes.contains(&claim.route()))
        .expect("fixture claim on a planned route");
    let original = analysis.claims[claim_index].clone();
    analysis.claims[claim_index] = ContentClaim::new(
        1,
        original.route(),
        original.state(),
        original.strength(),
        vec![missing_id(&analysis)],
        original.allowed_meanings().to_vec(),
    );

    let plan = seiri_planner::plan_patches(&analysis);
    assert!(plan.operations.is_empty());
    assert!(!plan.held.is_empty());
    assert!(plan
        .held
        .iter()
        .all(|hold| hold.reason == PatchHoldReason::EvidenceContractInvalid));
    assert_eq!(
        plan.claim_draft_state,
        ClaimDraftPlanState::Held(ClaimDraftPlanHoldReason::InvalidContract)
    );
    assert!(!plan.writes_files);
}

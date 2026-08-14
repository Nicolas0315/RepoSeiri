use seiri_appeal::{
    update_incremental_membrane, verify_incremental_against_scalar, CapabilityEvaluationIndex,
    DeterministicPerformanceReceipt, IncrementalMembraneState, PerformanceReceiptContext,
    PerformanceReceiptError,
};
use seiri_core::{AnalysisScope, ProfileKind, ValueDimension};
use std::path::{Path, PathBuf};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join("readme-route-repo")
}

fn receipt(
    observations: Vec<u64>,
) -> Result<DeterministicPerformanceReceipt, PerformanceReceiptError> {
    let analysis = seiri_report::audit_repository_with_scope(
        fixture(),
        ProfileKind::Common,
        AnalysisScope::Subtree,
    )
    .expect("audit fixture");
    let state = IncrementalMembraneState::from_scalar(
        &analysis.readme_grammar,
        &analysis.repository_capabilities,
        ProfileKind::Common,
    )
    .expect("scalar state");
    let update = update_incremental_membrane(
        &state,
        &analysis.readme_grammar,
        &analysis.repository_capabilities,
        ProfileKind::Common,
        &[],
    )
    .expect("incremental update");
    let equivalence = verify_incremental_against_scalar(
        &update,
        &analysis.readme_grammar,
        &analysis.repository_capabilities,
        ProfileKind::Common,
    )
    .expect("scalar equivalence");
    let context = PerformanceReceiptContext::try_new(
        "x86_64-pc-windows-msvc",
        "windows-debug",
        "fixtures/readme-route-repo",
        vec![
            "cargo".to_string(),
            "test".to_string(),
            "--test".to_string(),
            "r13_hf_h8_performance_receipt".to_string(),
        ],
        observations.len(),
    )?;
    DeterministicPerformanceReceipt::try_new(context, &update, equivalence, observations)
}

#[test]
fn receipt_hard_gates_equivalence_but_never_elapsed_time() {
    let receipt = receipt(vec![120, 90]).expect("performance receipt");
    assert!(receipt.equivalent);
    assert_eq!(receipt.incremental_digest, receipt.scalar_digest);
    assert!(!receipt.timing_gate_applied);
    assert!(receipt.boundary.contains("no timing threshold is applied"));
    assert_eq!(receipt.sample_count, 2);
    assert_eq!(receipt.observed_elapsed_micros, vec![120, 90]);
    receipt.validate().expect("valid receipt");
}

#[test]
fn semantic_identity_is_reproducible_while_timing_observations_stay_separate() {
    let first = receipt(vec![120, 90]).expect("first receipt");
    let second = receipt(vec![180, 140]).expect("second receipt");
    assert_eq!(
        first.semantic_receipt_digest,
        second.semantic_receipt_digest
    );
    assert_ne!(first.observation_digest, second.observation_digest);

    let json = serde_json::to_string(&first).expect("receipt JSON");
    let roundtrip = serde_json::from_str::<DeterministicPerformanceReceipt>(&json)
        .expect("validated receipt roundtrip");
    assert_eq!(roundtrip, first);
    assert_eq!(
        serde_json::to_string(&roundtrip).expect("repeat JSON"),
        json
    );
}

#[test]
fn tampered_receipts_fail_closed() {
    let receipt = receipt(vec![120, 90]).expect("performance receipt");
    let mut timing_gate = serde_json::to_value(&receipt).expect("receipt value");
    timing_gate["timing_gate_applied"] = serde_json::json!(true);
    assert!(serde_json::from_value::<DeterministicPerformanceReceipt>(timing_gate).is_err());

    let mut split_digest = serde_json::to_value(&receipt).expect("receipt value");
    split_digest["observed_elapsed_micros"] = serde_json::json!([120]);
    assert!(serde_json::from_value::<DeterministicPerformanceReceipt>(split_digest).is_err());

    let mut reordered = serde_json::to_value(&receipt).expect("receipt value");
    reordered["frontier"] = serde_json::json!(["outcome", "identity"]);
    assert!(serde_json::from_value::<DeterministicPerformanceReceipt>(reordered).is_err());
}

#[test]
fn shared_capability_index_is_canonical_for_every_dimension() {
    let analysis = seiri_report::audit_repository_with_scope(
        fixture(),
        ProfileKind::Common,
        AnalysisScope::Subtree,
    )
    .expect("audit fixture");
    let index = CapabilityEvaluationIndex::try_new(&analysis.repository_capabilities)
        .expect("capability evaluation index");
    for dimension in ValueDimension::ALL {
        for nodes in [
            index.direct_nodes(dimension),
            index.relevant_nodes(dimension),
        ] {
            assert!(nodes.windows(2).all(|pair| pair[0].id < pair[1].id));
        }
        assert!(index
            .signatures(dimension)
            .windows(2)
            .all(|pair| pair[0].capability_node < pair[1].capability_node));
    }
}

use seiri_codex::CodexQueryKind;
use seiri_core::CODEX_SCHEMA_VERSION;

#[test]
fn hardening_adversarial_contract_is_canonical_and_complete() {
    let corpus: serde_json::Value = serde_json::from_str(include_str!(
        "../fixtures/r13-hardening-adversarial-v1.json"
    ))
    .expect("H0 corpus is valid JSON");
    assert_eq!(
        corpus["schema_version"],
        "reposeiri.r13-hardening-adversarial.v1"
    );
    let cases = corpus["cases"].as_array().expect("cases array");
    assert_eq!(cases.len(), 9);
    let ids = cases
        .iter()
        .map(|case| case["id"].as_str().expect("case id"))
        .collect::<Vec<_>>();
    assert_eq!(
        ids,
        [
            "H0-001", "H0-002", "H0-003", "H0-004", "H0-005", "H0-006", "H0-007", "H0-008",
            "H0-009"
        ]
    );
    for case in cases {
        assert!(!case["category"].as_str().unwrap_or_default().is_empty());
        assert!(!case["expected_boundary"]
            .as_str()
            .unwrap_or_default()
            .is_empty());
    }
}

#[test]
fn hardening_preserves_the_outer_codex_contract() {
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
            "pr-body"
        ]
    );
}

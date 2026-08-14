use seiri_codex::CodexQueryKind;
use seiri_core::{
    ContractManifest, SemanticRevisionKey, CODEX_SCHEMA_VERSION, CONTRACT_SCHEMA_VERSION,
    PATCH_PLANNER_SEMANTIC_REVISION, WORDING_LINT_SCHEMA_VERSION,
};

const UNIX_LAUNCHER: &str = include_str!("../plugins/reposeiri/scripts/reposeiri-codex.sh");
const POWERSHELL_LAUNCHER: &str = include_str!("../plugins/reposeiri/scripts/reposeiri-codex.ps1");

#[test]
fn contract_and_both_launchers_share_hardened_revisions() {
    let manifest = ContractManifest::current(env!("CARGO_PKG_VERSION"));
    manifest.validate_current().expect("current contract");
    assert_eq!(manifest.schema_version, CONTRACT_SCHEMA_VERSION);
    assert_eq!(manifest.codex_schema, CODEX_SCHEMA_VERSION);
    assert_eq!(manifest.wording_lint_schema, WORDING_LINT_SCHEMA_VERSION);
    assert_eq!(
        manifest.semantic_revisions.patch_planner,
        PATCH_PLANNER_SEMANTIC_REVISION
    );
    assert_eq!(manifest.semantic_revisions.entries().len(), 31);
    assert_eq!(SemanticRevisionKey::ALL.len(), 31);

    for (needle, value) in [
        ("readme_grammar", seiri_core::README_GRAMMAR_REVISION),
        ("readme_claim_atom", seiri_core::README_CLAIM_ATOM_REVISION),
        (
            "readme_translation_alignment",
            seiri_core::README_TRANSLATION_ALIGNMENT_REVISION,
        ),
        ("patch_planner", PATCH_PLANNER_SEMANTIC_REVISION),
    ] {
        assert!(
            UNIX_LAUNCHER.contains(&format!("require_contract_value {needle} {value}")),
            "Unix launcher omitted {needle}={value}"
        );
        assert!(
            POWERSHELL_LAUNCHER.contains(&format!("{needle} = \"{value}\"")),
            "PowerShell launcher omitted {needle}={value}"
        );
    }
    assert!(UNIX_LAUNCHER.contains(&format!(
        "require_contract_value wording_lint_schema {WORDING_LINT_SCHEMA_VERSION}"
    )));
    assert!(POWERSHELL_LAUNCHER.contains(&format!(
        "$Contract.wording_lint_schema -ne \"{WORDING_LINT_SCHEMA_VERSION}\""
    )));
    assert!(UNIX_LAUNCHER.contains("reposeiri.runtime-manifest.v4"));
    assert!(POWERSHELL_LAUNCHER.contains("reposeiri.runtime-manifest.v4"));
}

#[test]
fn outer_wire_and_exact_query_set_remain_frozen() {
    let expected = [
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
    ];
    assert_eq!(CODEX_SCHEMA_VERSION, "seiri.codex.v2");
    assert_eq!(CodexQueryKind::ALL.len(), 10);
    assert_eq!(CodexQueryKind::ALL.map(CodexQueryKind::slug), expected);
    assert!(UNIX_LAUNCHER.contains(&format!("expected_queries='{}'", expected.join(" "))));
    for query in expected {
        assert!(POWERSHELL_LAUNCHER.contains(&format!("\"{query}\"")));
    }
}

#[test]
fn public_schema_snapshots_cover_new_typed_contracts() {
    let wording: serde_json::Value =
        serde_json::from_str(include_str!("../schemas/seiri.wording-lint.v2.json"))
            .expect("wording schema JSON");
    assert_eq!(
        wording["properties"]["schema_version"]["const"],
        WORDING_LINT_SCHEMA_VERSION
    );
    assert!(wording["$defs"]["finding"]["required"]
        .as_array()
        .is_some_and(|fields| fields.iter().any(|field| field == "language")));
    assert!(wording["$defs"]["summary"]["required"]
        .as_array()
        .is_some_and(|fields| fields.iter().any(|field| field == "inspection_coverage")));

    let patch: serde_json::Value =
        serde_json::from_str(include_str!("../schemas/seiri.patch-plan.v2.json"))
            .expect("patch schema JSON");
    assert_eq!(
        patch["$defs"]["decision_basis"]["properties"]["planner_semantic_revision"]["const"],
        PATCH_PLANNER_SEMANTIC_REVISION
    );
    assert!(patch["$defs"]["hold"]["properties"]["reason"]["enum"]
        .as_array()
        .is_some_and(|values| values
            .iter()
            .any(|value| value == "evidence_contract_invalid")));
}

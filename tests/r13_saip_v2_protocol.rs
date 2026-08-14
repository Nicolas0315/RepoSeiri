use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn template() -> Value {
    let body = fs::read_to_string(root().join("docs/design/r13-saip-v2-template.json"))
        .expect("read R13-SAIP template");
    serde_json::from_str(&body).expect("valid R13-SAIP template")
}

#[test]
fn r13_saip_runs_b0_through_b11_in_dependency_order() {
    let value = template();
    assert_eq!(value["schema_version"], "reposeiri.r13-saip.v2");
    assert_eq!(value["protocol"], "R13-SAIP-v2");
    assert_eq!(value["execution"]["max_in_progress_units"], 1);
    assert_eq!(value["execution"]["max_parallel_lanes"], 4);

    let units = value["units"].as_array().expect("units");
    assert_eq!(units.len(), 12);
    for (index, unit) in units.iter().enumerate() {
        assert_eq!(unit["id"], format!("B{index}"));
        let dependencies = unit["depends_on"].as_array().expect("dependencies");
        if index == 0 {
            assert!(dependencies.is_empty());
        } else {
            assert_eq!(dependencies, &[Value::String(format!("B{}", index - 1))]);
        }
    }
}

#[test]
fn r13_saip_freezes_semantic_and_authority_boundaries() {
    let value = template();
    assert_eq!(value["authority_defaults"]["mutation"], true);
    assert_eq!(value["authority_defaults"]["verification"], true);
    for denied in [
        "commit",
        "push",
        "merge",
        "release",
        "publication",
        "visibility",
        "plugin_install",
        "restart",
    ] {
        assert_eq!(value["authority_defaults"][denied], false);
    }

    let invariants = &value["semantic_invariants"];
    assert_eq!(invariants["claim_alignment_required"], true);
    assert_eq!(invariants["dimension_only_support_is_sufficient"], false);
    assert_eq!(invariants["preserve_explicit_unknown"], true);
    assert_eq!(invariants["supported_requires_aligned_evidence"], true);
    assert_eq!(invariants["geometry_changes_claim_semantics"], false);
    assert_eq!(invariants["incremental_must_match_scalar_digest"], true);
    assert_eq!(invariants["planner_writes_files"], false);
    assert_eq!(invariants["codex_query_kind_count"], 10);
    assert_eq!(invariants["codex_schema"], "seiri.codex.v2");
}

#[test]
fn r13_documents_are_japanese_first_and_english_second() {
    for path in [
        "docs/design/roadmap-v13-semantic-claim-alignment.md",
        "docs/design/r13-saip-v2-protocol.md",
    ] {
        let body = fs::read_to_string(root().join(path)).expect("read R13 document");
        let japanese = body.find("## 日本語").expect("Japanese section");
        let english = body.find("## English").expect("English section");
        assert!(japanese < english);
        for term in ["B0", "B11", "Unknown", "seiri.codex.v2", "ready_for_git"] {
            assert!(body[japanese..english].contains(term), "{path}: {term}");
            assert!(body[english..].contains(term), "{path}: {term}");
        }
    }
}

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn template() -> Value {
    let body = fs::read_to_string(root().join("docs/design/r12-saip-v1-template.json"))
        .expect("read R12-SAIP template");
    serde_json::from_str(&body).expect("valid R12-SAIP template")
}

#[test]
fn r12_saip_runs_b0_through_b11_in_dependency_order() {
    let value = template();
    assert_eq!(value["schema_version"], "reposeiri.r12-saip.v1");
    assert_eq!(value["protocol"], "R12-SAIP-v1");
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
fn r12_saip_freezes_authority_and_semantic_boundaries() {
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
    assert_eq!(value["semantic_invariants"]["codex_query_kind_count"], 10);
    assert_eq!(
        value["semantic_invariants"]["codex_schema"],
        "seiri.codex.v2"
    );
    assert_eq!(
        value["semantic_invariants"]["geometry_changes_claim_semantics"],
        false
    );
}

#[test]
fn r12_documents_are_japanese_first_and_english_second() {
    for path in [
        "docs/design/roadmap-v12-semantic-appeal-integrity.md",
        "docs/design/r12-saip-v1-protocol.md",
    ] {
        let body = fs::read_to_string(root().join(path)).expect("read R12 document");
        let japanese = body.find("## 日本語").expect("Japanese section");
        let english = body.find("## English").expect("English section");
        assert!(japanese < english);
        for term in ["B0", "B11", "seiri.codex.v2", "ready_for_git"] {
            assert!(body[japanese..english].contains(term), "{path}: {term}");
            assert!(body[english..].contains(term), "{path}: {term}");
        }
    }
}

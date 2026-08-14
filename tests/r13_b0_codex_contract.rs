use seiri_codex::{CodexQueryKind, CodexView};
use seiri_core::{ProfileKind, CODEX_SCHEMA_VERSION};
use std::path::{Path, PathBuf};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/readme-route-repo")
}

#[test]
fn all_ten_codex_queries_keep_the_bounded_outer_contract() {
    let expected = [
        ("summary", "summary"),
        ("routes", "routes"),
        ("evidence", "evidence"),
        ("documents", "documents"),
        ("governance", "governance"),
        ("patches", "patches"),
        ("linter", "linter"),
        ("actions", "actions"),
        ("remote", "remote"),
        ("pr-body", "pr_body"),
    ];
    assert_eq!(CodexQueryKind::ALL.len(), expected.len());

    let root = fixture();
    let absolute_root = std::fs::canonicalize(&root)
        .expect("canonical fixture")
        .to_string_lossy()
        .replace('\\', "/");
    let analysis =
        seiri_report::audit_repository_with_profile(&root, ProfileKind::Cli).expect("audit");
    let plan = seiri_planner::plan_patches(&analysis);
    let adapter = CodexView::new(&analysis, &plan, None);

    for (kind, (slug, wire_kind)) in CodexQueryKind::ALL.into_iter().zip(expected) {
        assert_eq!(kind.slug(), slug);
        let view = adapter.query(kind);
        let value = serde_json::to_value(&view).expect("query JSON value");
        let object = value.as_object().expect("outer query object");
        let keys = object.keys().map(String::as_str).collect::<Vec<_>>();
        assert_eq!(
            keys,
            [
                "boundary",
                "profile",
                "query",
                "repo_root",
                "schema_version"
            ]
        );
        assert_eq!(value["schema_version"], CODEX_SCHEMA_VERSION);
        assert_eq!(value["repo_root"], ".");
        assert_eq!(value["query"]["kind"], wire_kind);
        assert!(!value["query"]["data"].is_null());
        assert!(value["boundary"]
            .as_str()
            .is_some_and(|boundary| boundary.contains("do not write files")));

        let json = serde_json::to_string(&value).expect("query JSON");
        assert!(!json.replace('\\', "/").contains(&absolute_root));

        let markdown = seiri_codex::render_query_markdown(&view);
        for marker in [
            "# RepoSeiri Codex Query",
            CODEX_SCHEMA_VERSION,
            "Repository: `.`",
            "- Boundary:",
        ] {
            assert!(markdown.contains(marker), "{slug}: missing {marker}");
        }
        assert!(!markdown.replace('\\', "/").contains(&absolute_root));
    }
}

#[test]
fn codex_baseline_keeps_source_binding_and_preview_only_actions() {
    let analysis =
        seiri_report::audit_repository_with_profile(fixture(), ProfileKind::Cli).expect("audit");
    let plan = seiri_planner::plan_patches(&analysis);
    assert!(!plan.writes_files);

    let adapter = CodexView::new(&analysis, &plan, None);
    let summary =
        serde_json::to_value(adapter.query(CodexQueryKind::Summary)).expect("summary JSON value");
    assert_eq!(
        summary["query"]["data"]["source_session_digest"],
        serde_json::to_value(analysis.analysis_configuration.source_session_digest)
            .expect("source digest value")
    );

    let patches =
        serde_json::to_value(adapter.query(CodexQueryKind::Patches)).expect("patches JSON value");
    assert_eq!(patches["query"]["data"]["writes_files"], false);

    let actions =
        serde_json::to_value(adapter.query(CodexQueryKind::Actions)).expect("actions JSON value");
    assert!(actions["query"]["data"]
        .as_array()
        .expect("actions")
        .iter()
        .all(|action| action["mutates_files"] == false));
}

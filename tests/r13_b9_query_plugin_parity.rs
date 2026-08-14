use seiri_codex::{CodexQuery, CodexQueryKind, CodexView};
use seiri_core::{
    AnalysisScope, ClaimDraftPlanState, ContractManifest, ProfileKind, SemanticRevisionKey,
    SupportState, CODEX_SCHEMA_VERSION, CONTRACT_SCHEMA_VERSION, PORTABLE_AUDIT_SCHEMA_VERSION,
};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const UNIX_LAUNCHER: &str = include_str!("../plugins/reposeiri/scripts/reposeiri-codex.sh");
const POWERSHELL_LAUNCHER: &str = include_str!("../plugins/reposeiri/scripts/reposeiri-codex.ps1");

const EXPECTED_QUERIES: [(&str, &str); 10] = [
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

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/holdout-appeal-unknown")
}

fn missing_readme_fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/missing-readme-repo")
}

fn quoted_assignment(source: &str, name: &str, quote: char) -> String {
    let prefix = format!("{name}=");
    source
        .lines()
        .map(str::trim)
        .find_map(|line| {
            line.strip_prefix(&prefix)
                .and_then(|value| value.strip_prefix(quote))
                .and_then(|value| value.strip_suffix(quote))
                .map(str::to_string)
        })
        .unwrap_or_else(|| panic!("missing launcher assignment {name}"))
}

fn powershell_assignment(source: &str, name: &str) -> String {
    let prefix = format!("{name} = \"");
    source
        .lines()
        .map(str::trim)
        .find_map(|line| {
            line.strip_prefix(&prefix)
                .and_then(|value| value.strip_suffix('"'))
                .map(str::to_string)
        })
        .unwrap_or_else(|| panic!("missing PowerShell launcher assignment {name}"))
}

fn expected_revisions() -> Vec<(String, String)> {
    ContractManifest::current("test")
        .semantic_revisions
        .entries()
        .into_iter()
        .map(|entry| {
            let key = serde_json::to_value(entry.key)
                .expect("serialize revision key")
                .as_str()
                .expect("revision key string")
                .to_string();
            (key, entry.revision.to_string())
        })
        .collect()
}

fn shell_revisions(prefix: &str) -> Vec<(String, String)> {
    let expected_keys = expected_revisions()
        .into_iter()
        .map(|(key, _)| key)
        .collect::<BTreeSet<_>>();
    UNIX_LAUNCHER
        .lines()
        .map(str::trim)
        .filter_map(|line| line.strip_prefix(prefix))
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let key = fields.next()?.to_string();
            let revision = fields.next()?.trim_matches(['\'', '"']).to_string();
            (fields.next().is_none() && expected_keys.contains(&key)).then_some((key, revision))
        })
        .collect()
}

fn powershell_revisions() -> Vec<(String, String)> {
    let block = POWERSHELL_LAUNCHER
        .split_once("$ExpectedRevisions = [ordered]@{")
        .expect("PowerShell revision block")
        .1
        .split_once('}')
        .expect("PowerShell revision block terminator")
        .0;
    block
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| {
            let (key, revision) = line.split_once('=').expect("PowerShell revision entry");
            (
                key.trim().to_string(),
                revision.trim().trim_matches('"').to_string(),
            )
        })
        .collect()
}

fn powershell_queries() -> Vec<String> {
    let line = POWERSHELL_LAUNCHER
        .lines()
        .find(|line| line.contains("ValidateSet") && line.contains("pr-body"))
        .expect("PowerShell query ValidateSet");
    line.split('"')
        .skip(1)
        .step_by(2)
        .map(str::to_string)
        .collect()
}

#[test]
fn unix_and_powershell_launchers_match_the_v6_contract_and_all_revisions() {
    let expected = expected_revisions();
    assert_eq!(SemanticRevisionKey::ALL.len(), 31);
    assert_eq!(expected.len(), 31);

    assert_eq!(
        quoted_assignment(UNIX_LAUNCHER, "expected_schema", '\''),
        CODEX_SCHEMA_VERSION
    );
    assert_eq!(
        quoted_assignment(UNIX_LAUNCHER, "expected_contract_schema", '\''),
        CONTRACT_SCHEMA_VERSION
    );
    assert_eq!(
        powershell_assignment(POWERSHELL_LAUNCHER, "$ExpectedSchema"),
        CODEX_SCHEMA_VERSION
    );
    assert_eq!(
        powershell_assignment(POWERSHELL_LAUNCHER, "$ExpectedContractSchema"),
        CONTRACT_SCHEMA_VERSION
    );
    assert!(UNIX_LAUNCHER.contains(&format!(
        "require_contract_value portable_audit_schema {PORTABLE_AUDIT_SCHEMA_VERSION}"
    )));
    assert!(UNIX_LAUNCHER.contains(&format!("{PORTABLE_AUDIT_SCHEMA_VERSION}.json")));
    assert!(POWERSHELL_LAUNCHER.contains(&format!(
        "$Contract.portable_audit_schema -ne \"{PORTABLE_AUDIT_SCHEMA_VERSION}\""
    )));

    assert_eq!(shell_revisions("require_contract_value "), expected);
    assert_eq!(shell_revisions("require_manifest_value "), expected);
    assert_eq!(powershell_revisions(), expected);
}

#[test]
fn launchers_and_core_expose_exactly_the_same_ten_query_slugs() {
    let expected = EXPECTED_QUERIES
        .iter()
        .map(|(slug, _)| (*slug).to_string())
        .collect::<Vec<_>>();
    let core = CodexQueryKind::ALL
        .into_iter()
        .map(|kind| kind.slug().to_string())
        .collect::<Vec<_>>();
    let unix = quoted_assignment(UNIX_LAUNCHER, "expected_queries", '\'')
        .split_whitespace()
        .map(str::to_string)
        .collect::<Vec<_>>();

    assert_eq!(CodexQueryKind::ALL.len(), 10);
    assert_eq!(core, expected);
    assert_eq!(unix, expected);
    assert_eq!(powershell_queries(), expected);
    assert!(UNIX_LAUNCHER.contains("validate_query_args \"$@\""));
}

#[test]
fn all_query_formats_keep_the_outer_wire_and_bounded_no_write_surface() {
    let analysis = seiri_report::audit_repository_with_scope(
        fixture(),
        ProfileKind::Cli,
        AnalysisScope::Subtree,
    )
    .expect("bounded fixture audit");
    let plan = seiri_planner::plan_patches(&analysis);
    let adapter = CodexView::new(&analysis, &plan, None);

    assert!(!plan.writes_files);
    for (kind, (slug, wire_kind)) in CodexQueryKind::ALL.into_iter().zip(EXPECTED_QUERIES) {
        assert_eq!(kind.slug(), slug);
        let view = adapter.query(kind);
        let json = serde_json::to_value(&view).expect("Codex query JSON");
        let markdown = seiri_codex::render_query_markdown(&view);

        assert_eq!(json["schema_version"], CODEX_SCHEMA_VERSION, "{slug}");
        assert_eq!(json["query"]["kind"], wire_kind, "{slug}");
        assert!(!json["query"]["data"].is_null(), "{slug}");
        assert!(json["boundary"]
            .as_str()
            .is_some_and(|boundary| boundary.contains("do not write files")));
        for marker in [
            "# RepoSeiri Codex Query",
            CODEX_SCHEMA_VERSION,
            &format!("Query: `{slug}`"),
            "- Boundary:",
        ] {
            assert!(markdown.contains(marker), "{slug}: missing {marker}");
        }

        let data = &json["query"]["data"];
        match &view.query {
            CodexQuery::Summary(summary) => {
                assert_eq!(data["contract_schema_version"], CONTRACT_SCHEMA_VERSION);
                assert_eq!(
                    data["portable_audit_schema_version"],
                    PORTABLE_AUDIT_SCHEMA_VERSION
                );
                assert_eq!(
                    data["patch_plan_schema_version"],
                    summary.patch_plan_schema_version
                );
                assert_eq!(
                    data["claim_draft_state"],
                    serde_json::to_value(summary.claim_draft_state).expect("draft state JSON")
                );
                assert_eq!(
                    data["maximum_claim_draft_ceiling"],
                    serde_json::to_value(summary.maximum_claim_draft_ceiling)
                        .expect("claim ceiling JSON")
                );
                assert_eq!(data["writes_files"], summary.writes_files);
                for marker in [
                    format!(
                        "- Contract schema: `{}`; portable audit schema: `{}`; patch-plan schema: `{}`",
                        summary.contract_schema_version,
                        summary.portable_audit_schema_version,
                        summary.patch_plan_schema_version,
                    ),
                    format!("- Claim draft state: `{:?}`", summary.claim_draft_state),
                    format!(
                        "Claim drafts: `{}`; baseline unknown: `{}`; maximum claim ceiling: `{:?}`",
                        summary.claim_drafts,
                        summary.claim_draft_baseline_unknown_count,
                        summary.maximum_claim_draft_ceiling,
                    ),
                    format!("- Writes files: `{}`", summary.writes_files),
                    format!(
                        "- Coverage: `{}` complete / `{}` partial / `{}` not requested; limit exceeded `{}`",
                        summary.coverage.complete_scopes,
                        summary.coverage.partial_scopes,
                        summary.coverage.not_requested_scopes,
                        summary.coverage.limit_exceeded_scopes,
                    ),
                    format!(
                        "`{}` unknown (`{}` unacknowledged; `{}` limit-exceeded)",
                        summary.observations.unknown,
                        summary.observations.unacknowledged_unknown,
                        summary.observations.limit_exceeded,
                    ),
                ] {
                    assert!(markdown.contains(&marker), "summary: missing {marker}");
                }
            }
            CodexQuery::Routes(routes) => {
                assert_eq!(
                    data["priorities"],
                    serde_json::to_value(routes.priorities).expect("route priorities JSON")
                );
                for assessment in routes.assessments {
                    let projection = assessment.summary_projection();
                    let state = format!("- `{:?}`: `{:?}`;", assessment.route(), projection.state);
                    let reason = format!("  Reason: {}", projection.reason);
                    assert!(markdown.contains(&state), "routes: missing {state}");
                    assert!(markdown.contains(&reason), "routes: missing {reason}");
                }
                for priority in &routes.priorities.priorities {
                    let marker = format!(
                        "state `{:?}`, gate `{:?}`, severity `{:?}`, priority `{:?}`",
                        priority.state, priority.gate, priority.severity, priority.priority,
                    );
                    assert!(markdown.contains(&marker), "routes: missing {marker}");
                    assert!(markdown.contains(&priority.reason));
                }
            }
            CodexQuery::Evidence(evidence) => {
                assert_eq!(
                    data["coverage"],
                    serde_json::to_value(evidence.coverage).expect("coverage JSON")
                );
                for record in evidence.coverage.records() {
                    let marker = format!("- Scope `{:?}`: `{:?}`", record.scope, record.status);
                    assert!(markdown.contains(&marker), "evidence: missing {marker}");
                }
            }
            CodexQuery::Documents(documents) => {
                assert_eq!(
                    data["index"],
                    serde_json::to_value(documents.index).expect("document index JSON")
                );
                for document in documents.index.entries() {
                    let marker = format!(
                        "`{}`: role `{:?}`, status `{:?}`",
                        document.path, document.role, document.status,
                    );
                    assert!(markdown.contains(&marker), "documents: missing {marker}");
                }
                for document in documents.github.documents() {
                    let marker = format!(
                        "`{}`: kind `{:?}`, status `{:?}`",
                        document.path, document.kind, document.status,
                    );
                    assert!(markdown.contains(&marker), "documents: missing {marker}");
                }
            }
            CodexQuery::Governance(governance) => {
                assert_eq!(
                    data["claim_capability_membrane"],
                    serde_json::to_value(governance.claim_capability_membrane)
                        .expect("membrane JSON")
                );
                assert!(
                    !governance
                        .repository_capabilities
                        .unknown_reasons
                        .is_empty(),
                    "fixture must retain a typed program-analysis Unknown"
                );
                assert!(
                    governance
                        .claim_capability_membrane
                        .alignments
                        .iter()
                        .any(|alignment| matches!(alignment.state, SupportState::Unknown(_))),
                    "fixture must retain Unknown at claim/capability alignment"
                );
                assert!(!governance.claim_capability_membrane.risks.is_empty());
                let unknown_marker = format!(
                    "Repository capability coverage: `{:?}`; nodes: `{}`; diagnostics: `{}`; Unknown reasons: `{:?}`",
                    governance.repository_capabilities.coverage,
                    governance.repository_capabilities.nodes.len(),
                    governance.repository_capabilities.diagnostics.len(),
                    governance.repository_capabilities.unknown_reasons,
                );
                assert!(markdown.contains(&unknown_marker));
                for relation in &governance.claim_capability_membrane.relations {
                    let marker = format!(
                        "- `{:?}`: state `{:?}`, evidence `{}`",
                        relation.dimension, relation.state, relation.evidence_count,
                    );
                    assert!(markdown.contains(&marker), "governance: missing {marker}");
                }
                for alignment in &governance.claim_capability_membrane.alignments {
                    let marker = format!(
                        "Claim `{:?}`: state `{:?}`, evidence `{}`, ceiling `{:?}`",
                        alignment.claim_atom,
                        alignment.state,
                        alignment.evidence_count,
                        alignment.claim_ceiling,
                    );
                    assert!(markdown.contains(&marker), "governance: missing {marker}");
                }
                for risk in &governance.claim_capability_membrane.risks {
                    let marker = format!("`{:?}` / `{:?}`", risk.dimension, risk.kind);
                    assert!(markdown.contains(&marker), "governance: missing {marker}");
                }
            }
            CodexQuery::Patches(patches) => {
                assert_eq!(
                    data["claim_draft_state"],
                    serde_json::to_value(patches.claim_draft_state).expect("draft state JSON")
                );
                assert_eq!(data["writes_files"], patches.writes_files);
                for marker in [
                    format!("- Claim draft state: `{:?}`", patches.claim_draft_state),
                    format!("- Held items: `{}`", patches.held.len()),
                    format!("- Writes files: `{}`", patches.writes_files),
                    format!("- Plan boundary: {}", patches.boundary),
                ] {
                    assert!(markdown.contains(&marker), "patches: missing {marker}");
                }
                assert!(
                    !patches.held.is_empty(),
                    "fixture must exercise patch holds"
                );
                for hold in &patches.held {
                    let marker = format!(
                        "`{:?}` at `{:?}`: reason `{:?}`, gate `{:?}`",
                        hold.route, hold.target_path, hold.reason, hold.decision_basis.gate,
                    );
                    assert!(markdown.contains(&marker), "patches: missing {marker}");
                }
                for suggestion in &patches.appeal_suggestions {
                    let marker = format!(
                        "gate `{:?}`, support `{:?}`, ceiling `{:?}`",
                        suggestion.gate, suggestion.support_state, suggestion.claim_ceiling,
                    );
                    assert!(markdown.contains(&marker), "patches: missing {marker}");
                }
            }
            CodexQuery::Linter(linter) => {
                assert_eq!(data["report"], serde_json::Value::Null);
                assert_eq!(data["boundary"], linter.boundary);
                assert!(markdown.contains("- Available: `false`"));
                assert!(markdown.contains(&format!("- Linter boundary: {}", linter.boundary)));
            }
            CodexQuery::Actions(actions) => {
                assert_eq!(data, &serde_json::to_value(actions).expect("actions JSON"));
                assert!(!actions.is_empty());
                for action in actions {
                    let marker = format!(
                        "mutates files `{}`; requires confirmation `{}`",
                        action.mutates_files, action.requires_confirmation,
                    );
                    assert!(!action.mutates_files);
                    assert!(markdown.contains(&marker), "actions: missing {marker}");
                }
            }
            CodexQuery::Remote(remote) => {
                assert_eq!(data, &serde_json::to_value(remote).expect("remote JSON"));
                for marker in [
                    format!("- Remote status: `{:?}`", remote.status),
                    format!("- Coverage: `{:?}`", remote.coverage),
                    format!("- Remote boundary: {}", remote.boundary),
                ] {
                    assert!(markdown.contains(&marker), "remote: missing {marker}");
                }
            }
            CodexQuery::PrBody(pr) => {
                assert_eq!(data["title"], pr.title);
                assert_eq!(data["draft"], pr.draft);
                assert!(markdown.contains(&format!("- PR title: {}", pr.title)));
                assert!(markdown.contains(&format!("- Draft: `{}`", pr.draft)));
            }
        }
    }
}

#[test]
fn claim_draft_hold_reason_is_visible_without_becoming_a_write() {
    let analysis = seiri_report::audit_repository_with_scope(
        missing_readme_fixture(),
        ProfileKind::Cli,
        AnalysisScope::Subtree,
    )
    .expect("missing README fixture audit");
    let plan = seiri_planner::plan_patches(&analysis);
    assert!(matches!(
        plan.claim_draft_state,
        ClaimDraftPlanState::Held(_)
    ));
    assert!(!plan.writes_files);

    let adapter = CodexView::new(&analysis, &plan, None);
    let view = adapter.query(CodexQueryKind::Patches);
    let json = serde_json::to_value(&view).expect("held patches JSON");
    let markdown = seiri_codex::render_query_markdown(&view);
    assert_eq!(
        json["query"]["data"]["claim_draft_state"],
        serde_json::to_value(plan.claim_draft_state).expect("held state JSON")
    );
    assert_eq!(json["query"]["data"]["writes_files"], false);
    assert!(markdown.contains(&format!(
        "- Claim draft state: `{:?}`",
        plan.claim_draft_state
    )));
    assert!(markdown.contains("- Writes files: `false`"));
}

use seiri_core::{DocumentLanguage, WordingLintSourceKind, WordingRuleKind};

#[test]
fn japanese_visible_prose_is_typed_and_source_bound() {
    let root = tempfile::tempdir().expect("temporary repository");
    let source = concat!(
        "# 主張\n\n",
        "RepoSeiri はセキュリティを保証します。\n\n",
        "RepoSeiri は本番利用可能です。\n",
    );
    std::fs::write(root.path().join("README.md"), source).expect("write README");

    let report = seiri_report::lint_wording_repository(root.path()).expect("wording lint report");
    let findings = report
        .findings
        .iter()
        .filter(|finding| finding.source == WordingLintSourceKind::RepositoryFile)
        .collect::<Vec<_>>();

    assert_eq!(report.schema_version, "seiri.wording-lint.v2");
    assert_eq!(findings.len(), 2, "{findings:#?}");
    assert_eq!(findings[0].language, DocumentLanguage::Japanese);
    assert_eq!(findings[0].rule, WordingRuleKind::SecurityGuarantee);
    assert_eq!(findings[0].matched, "セキュリティを保証");
    assert_eq!(
        &source[findings[0].byte_start..findings[0].byte_end],
        findings[0].matched
    );
    assert_eq!(findings[0].line, 3);
    assert_eq!(findings[0].column, 12);
    assert_eq!(findings[1].rule, WordingRuleKind::ProductionReadiness);
}

#[test]
fn japanese_negated_boundary_and_dead_zones_do_not_become_findings() {
    let root = tempfile::tempdir().expect("temporary repository");
    std::fs::write(
        root.path().join("README.md"),
        concat!(
            "# 境界\n\n",
            "RepoSeiri はセキュリティを保証しません。\n\n",
            "RepoSeiri は公開準備完了を保証しません。\n\n",
            "人気、信頼、安全性、品質、法的適合性、公開準備完了、一般性能を証明しません。\n\n",
            "公開準備完了を RepoSeiri の出力から保証しません。\n\n",
            "```text\nRepoSeiri は品質を保証します。\n```\n\n",
            "`RepoSeiri は人気を保証します。`\n",
        ),
    )
    .expect("write README");

    let report = seiri_report::lint_wording_repository(root.path()).expect("wording lint report");
    assert!(report
        .findings
        .iter()
        .all(|finding| finding.source != WordingLintSourceKind::RepositoryFile));
    assert!(report.summary.suppressed_boundary_exceptions >= 1);
}

#[test]
fn japanese_later_unrelated_negation_does_not_hide_a_positive_claim() {
    let root = tempfile::tempdir().expect("temporary repository");
    std::fs::write(
        root.path().join("README.md"),
        "RepoSeiri は公開準備完了を保証しますが、一般性能は保証しません。\n",
    )
    .expect("write README");

    let report = seiri_report::lint_wording_repository(root.path()).expect("wording lint report");
    assert!(report.findings.iter().any(|finding| {
        finding.source == WordingLintSourceKind::RepositoryFile && finding.matched == "公開準備完了"
    }));
}

#[test]
fn report_publishes_canonical_bilingual_inspection_coverage() {
    let root = tempfile::tempdir().expect("temporary repository");
    std::fs::write(
        root.path().join("README.md"),
        "# Repo\n\n安全な説明です。\n",
    )
    .expect("write README");

    let report = seiri_report::lint_wording_repository(root.path()).expect("wording lint report");
    let languages = report
        .summary
        .inspection_coverage
        .iter()
        .map(|coverage| coverage.language)
        .collect::<Vec<_>>();
    assert_eq!(
        languages,
        vec![DocumentLanguage::Japanese, DocumentLanguage::English]
    );
    assert!(report
        .summary
        .inspection_coverage
        .iter()
        .all(|coverage| coverage.visible_segments_inspected > 0));

    let json = seiri_report::wording_lint_to_json(&report).expect("wording JSON");
    let roundtrip = serde_json::from_str::<seiri_core::WordingLintReport>(&json)
        .expect("roundtrip wording report");
    assert_eq!(roundtrip, report);
    assert_eq!(
        json,
        seiri_report::wording_lint_to_json(&roundtrip).unwrap()
    );
}

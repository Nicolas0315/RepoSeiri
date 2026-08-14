use seiri_core::{
    CapabilityKind, CapabilityRelation, CoverageIncompleteReason, CoverageStatus, FileKind,
    FileRecord, RepositoryCapabilityIR, SourceDocument, SourceStore, UnknownReason,
};
use seiri_program_local::{
    analyze_rust_capabilities, scan_program_source_session, ProgramAnalysisOptions,
    ProgramSourceOptions, ProgramSourceReport,
};

fn analyze(sources: Vec<(&str, &str)>) -> RepositoryCapabilityIR {
    let mut documents = sources
        .into_iter()
        .map(|(path, source)| {
            SourceDocument::from_bytes(path.to_string(), source.as_bytes().to_vec())
        })
        .collect::<Vec<_>>();
    documents.sort_by(|left, right| left.path().cmp(right.path()));
    let store = SourceStore::try_new(documents).expect("canonical source store");
    let report = ProgramSourceReport {
        coverage: CoverageStatus::Complete,
        candidate_files: store.documents().len(),
        selected_files: store.documents().len(),
        selected_bytes: store.total_bytes(),
        skipped_existing: 0,
        issues: Vec::new(),
    };
    analyze_rust_capabilities(&store, &report, &ProgramAnalysisOptions::default())
}

#[test]
fn logical_items_preserve_multiline_reexports_cfg_and_visibility() {
    let source = r####"
const SAMPLE: &str = r###"
pub fn phantom() {}
"###;

#[cfg(feature = "audit")]
pub fn audit_repository(
    path: &str,
) -> Result<(), Error> {
    todo!()
}

pub use crate::internal::Runner;
pub(crate) fn internal_only() {}
"####;
    let ir = analyze(vec![("src/lib.rs", source)]);

    let audit = ir
        .nodes
        .iter()
        .find(|node| node.kind == CapabilityKind::Operation && node.symbol == "audit_repository")
        .expect("multiline public function");
    let feature = ir
        .nodes
        .iter()
        .find(|node| node.kind == CapabilityKind::Feature && node.symbol == "audit")
        .expect("cfg feature");
    assert!(ir.edges.iter().any(|edge| {
        edge.from == audit.id
            && edge.to == feature.id
            && edge.relation == CapabilityRelation::ConditionedBy
    }));
    assert!(ir
        .nodes
        .iter()
        .any(|node| node.kind == CapabilityKind::PublicApi && node.symbol == "Runner"));
    assert!(!ir
        .nodes
        .iter()
        .any(|node| matches!(node.symbol.as_str(), "internal_only" | "phantom")));
    assert!(ir
        .nodes
        .iter()
        .any(|node| node.kind == CapabilityKind::Input));
    assert!(ir
        .nodes
        .iter()
        .any(|node| node.kind == CapabilityKind::Output));
}

#[test]
fn unsupported_regions_are_local_and_do_not_erase_observed_neighbors() {
    let ir = analyze(vec![(
        "src/lib.rs",
        "macro_rules! generated { () => { pub fn generated_only() {} } }\n\
         include!(concat!(env!(\"OUT_DIR\"), \"/generated.rs\"));\n\
         pub fn stable() {}\n",
    )]);

    assert!(ir
        .nodes
        .iter()
        .any(|node| node.kind == CapabilityKind::Operation && node.symbol == "stable"));
    assert!(!ir.nodes.iter().any(|node| node.symbol == "generated_only"));
    assert_eq!(
        ir.coverage,
        CoverageStatus::Partial(CoverageIncompleteReason::UnsupportedSyntax)
    );
    assert!(ir
        .unknown_reasons
        .contains(&UnknownReason::UnsupportedSyntax));
    assert!(!ir.diagnostics.is_empty());
    assert!(ir.diagnostics.iter().all(|diagnostic| {
        diagnostic.path == "src/lib.rs"
            && diagnostic.reason == UnknownReason::UnsupportedSyntax
            && diagnostic.span.is_some()
    }));
}

#[test]
fn source_read_issue_reason_is_not_collapsed_to_limit_exceeded() {
    let root = tempfile::tempdir().expect("tempdir");
    let session = scan_program_source_session(
        root.path(),
        &[FileRecord {
            path: "src/missing.rs".to_string(),
            kind: FileKind::File,
            size_bytes: 10,
        }],
        true,
        SourceStore::default(),
        &ProgramSourceOptions::default(),
    )
    .expect("bounded scan");

    assert_eq!(
        session.report().issues[0].reason,
        UnknownReason::Unavailable
    );
    assert_eq!(
        session.report().coverage,
        CoverageStatus::Partial(CoverageIncompleteReason::Unavailable)
    );
    let ir = analyze_rust_capabilities(
        session.store(),
        session.report(),
        &ProgramAnalysisOptions::default(),
    );
    assert_eq!(ir.unknown_reasons, [UnknownReason::Unavailable]);
    assert_eq!(ir.diagnostics[0].path, "src/missing.rs");
}

#[test]
fn capability_frontend_is_deterministic_and_json_roundtrips() {
    let left = analyze(vec![
        ("src/z.rs", "pub fn zed() {}\n"),
        ("src/a.rs", "pub fn alpha(value: u8) -> u8 { value }\n"),
    ]);
    let right = analyze(vec![
        ("src/a.rs", "pub fn alpha(value: u8) -> u8 { value }\n"),
        ("src/z.rs", "pub fn zed() {}\n"),
    ]);
    assert_eq!(left, right);
    let left_json = serde_json::to_string(&left).expect("serialize left");
    let right_json = serde_json::to_string(&right).expect("serialize right");
    assert_eq!(left_json, right_json);
    let roundtrip: RepositoryCapabilityIR =
        serde_json::from_str(&left_json).expect("deserialize capability IR");
    assert_eq!(roundtrip, left);
}

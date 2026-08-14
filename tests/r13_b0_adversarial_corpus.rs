use serde_json::Value;
use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

const CORPUS_PATH: &str = "fixtures/r13-appeal-adversarial-v1.json";
const MAX_CORPUS_BYTES: usize = 64 * 1024;
const REQUIRED_CATEGORIES: [&str; 15] = [
    "bilingual_fragmentation",
    "clause_negation",
    "example_vs_first_result",
    "inline_formatting",
    "inverse_ja_en_polarity",
    "limit_exceeded",
    "read_issue",
    "rust_cfg",
    "rust_macro",
    "rust_multiline",
    "rust_pub_crate",
    "rust_pub_use",
    "rust_raw_string_false_positive",
    "unrelated_capability",
    "unsupported_syntax",
];
const REQUIRED_UNKNOWN_REASONS: [&str; 3] =
    ["limit_exceeded", "permission_denied", "unsupported_syntax"];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn corpus_bytes() -> Vec<u8> {
    let bytes = fs::read(root().join(CORPUS_PATH)).expect("read R13 adversarial corpus");
    assert!(
        bytes.len() <= MAX_CORPUS_BYTES,
        "R13 corpus exceeds its test-side byte ceiling"
    );
    bytes
}

fn parse_corpus() -> Value {
    serde_json::from_slice(&corpus_bytes()).expect("valid R13 adversarial corpus JSON")
}

fn required_string<'a>(value: &'a Value, field: &str) -> &'a str {
    value[field]
        .as_str()
        .unwrap_or_else(|| panic!("{field} must be a string"))
}

fn required_limit(limits: &Value, field: &str) -> usize {
    usize::try_from(
        limits[field]
            .as_u64()
            .unwrap_or_else(|| panic!("{field} must be a non-negative integer")),
    )
    .expect("limit fits usize")
}

fn string_set(value: &Value, field: &str) -> BTreeSet<String> {
    value[field]
        .as_array()
        .unwrap_or_else(|| panic!("{field} must be an array"))
        .iter()
        .map(|entry| {
            entry
                .as_str()
                .unwrap_or_else(|| panic!("{field} entries must be strings"))
                .to_owned()
        })
        .collect()
}

#[test]
fn r13_adversarial_corpus_is_bounded_unique_and_complete() {
    let corpus = parse_corpus();
    assert_eq!(
        corpus["schema_version"],
        "reposeiri.r13-appeal-adversarial.v1"
    );
    assert_eq!(
        corpus["purpose"], "future_gate_fixture_only",
        "the B0 corpus must not claim current semantic behavior"
    );
    assert!(required_string(&corpus, "claim_boundary")
        .contains("do not assert current parser behavior"));

    let declared_categories = string_set(&corpus, "required_categories");
    let expected_categories = REQUIRED_CATEGORIES
        .iter()
        .map(|category| (*category).to_owned())
        .collect::<BTreeSet<_>>();
    assert_eq!(declared_categories, expected_categories);
    assert_eq!(
        corpus["required_categories"]
            .as_array()
            .expect("required categories")
            .len(),
        declared_categories.len(),
        "required categories must not contain duplicates"
    );

    let declared_unknown_reasons = string_set(&corpus, "required_unknown_reasons");
    let expected_unknown_reasons = REQUIRED_UNKNOWN_REASONS
        .iter()
        .map(|reason| (*reason).to_owned())
        .collect::<BTreeSet<_>>();
    assert_eq!(declared_unknown_reasons, expected_unknown_reasons);

    let limits = &corpus["limits"];
    let max_cases = required_limit(limits, "max_cases");
    let max_artifacts = required_limit(limits, "max_artifacts_per_case");
    let max_categories = required_limit(limits, "max_categories_per_case");
    let max_source_bytes = required_limit(limits, "max_source_bytes_per_artifact");
    let max_total_source_bytes = required_limit(limits, "max_total_source_bytes");
    assert!((1..=256).contains(&max_cases));
    assert!((1..=16).contains(&max_artifacts));
    assert!((1..=16).contains(&max_categories));
    assert!((1..=64 * 1024).contains(&max_source_bytes));
    assert!((1..=1024 * 1024).contains(&max_total_source_bytes));

    let cases = corpus["cases"].as_array().expect("cases must be an array");
    assert!(!cases.is_empty());
    assert!(cases.len() <= max_cases);

    let mut ids = HashSet::new();
    let mut ordered_ids = Vec::with_capacity(cases.len());
    let mut observed_categories = BTreeSet::new();
    let mut observed_unknown_reasons = BTreeSet::new();
    let mut total_source_bytes = 0usize;

    for case in cases {
        let id = required_string(case, "id");
        assert!((1..=96).contains(&id.len()));
        assert!(id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-'));
        assert!(ids.insert(id), "duplicate case id: {id}");
        ordered_ids.push(id);

        let gate = required_string(case, "future_gate");
        let gate_number = gate
            .strip_prefix('B')
            .and_then(|number| number.parse::<u8>().ok())
            .expect("future_gate must be B<number>");
        assert!((1..=11).contains(&gate_number));

        let categories = case["categories"]
            .as_array()
            .expect("case categories must be an array");
        assert!(!categories.is_empty());
        assert!(categories.len() <= max_categories);
        let mut case_categories = HashSet::new();
        for category in categories {
            let category = category.as_str().expect("category must be a string");
            assert!(
                declared_categories.contains(category),
                "undeclared category: {category}"
            );
            assert!(
                case_categories.insert(category),
                "duplicate category in {id}: {category}"
            );
            observed_categories.insert(category.to_owned());
        }

        let artifacts = case["artifacts"]
            .as_array()
            .expect("case artifacts must be an array");
        assert!(!artifacts.is_empty());
        assert!(artifacts.len() <= max_artifacts);
        let mut paths = HashSet::new();
        for artifact in artifacts {
            let path = required_string(artifact, "path");
            assert!((1..=240).contains(&path.len()));
            assert!(!path.contains('\\'));
            assert!(Path::new(path)
                .components()
                .all(|component| matches!(component, Component::Normal(_))));
            assert!(
                paths.insert(path),
                "duplicate artifact path in {id}: {path}"
            );

            let kind = required_string(artifact, "kind");
            assert!(matches!(kind, "markdown" | "rust" | "synthetic_read_issue"));
            let content = required_string(artifact, "content");
            assert!(content.len() <= max_source_bytes);
            total_source_bytes = total_source_bytes
                .checked_add(content.len())
                .expect("source byte total does not overflow");

            if kind == "synthetic_read_issue" {
                assert!(content.is_empty());
                assert_eq!(artifact["simulated_issue"], "permission_denied");
            }
            if let Some(configured_limit) = artifact["configured_limit_bytes"].as_u64() {
                let configured_limit = usize::try_from(configured_limit).expect("limit fits usize");
                assert!(configured_limit > 0);
                assert!(
                    content.len() > configured_limit,
                    "limit fixture must exceed its synthetic configured limit"
                );
            }
        }

        let expectation = &case["future_expectation"];
        assert_eq!(expectation["mode"], "future_gate_only");
        let invariant = required_string(expectation, "target_invariant");
        assert!((1..=512).contains(&invariant.len()));
        let preserves_unknown = expectation["must_preserve_unknown"]
            .as_bool()
            .expect("must_preserve_unknown must be a boolean");
        match expectation["unknown_reason"].as_str() {
            Some(reason) => {
                assert!(preserves_unknown);
                assert!(declared_unknown_reasons.contains(reason));
                observed_unknown_reasons.insert(reason.to_owned());
            }
            None => assert!(!preserves_unknown),
        }
    }

    let mut sorted_ids = ordered_ids.clone();
    sorted_ids.sort_unstable();
    assert_eq!(ordered_ids, sorted_ids, "case order must be deterministic");
    assert_eq!(observed_categories, declared_categories);
    assert_eq!(observed_unknown_reasons, declared_unknown_reasons);
    assert!(total_source_bytes <= max_total_source_bytes);
}

#[test]
fn r13_adversarial_corpus_parse_and_wire_are_deterministic() {
    let bytes = corpus_bytes();
    let first: Value = serde_json::from_slice(&bytes).expect("first parse");
    let second: Value = serde_json::from_slice(&bytes).expect("second parse");
    assert_eq!(first, second);

    let first_wire = serde_json::to_vec(&first).expect("first deterministic wire");
    let second_wire = serde_json::to_vec(&second).expect("second deterministic wire");
    assert_eq!(first_wire, second_wire);

    let roundtrip: Value = serde_json::from_slice(&first_wire).expect("roundtrip parse");
    assert_eq!(first, roundtrip);
}

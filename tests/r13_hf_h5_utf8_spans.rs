use seiri_core::{DocumentScan, DocumentScanInvariantError};
use seiri_markdown::{parse_readme, scan_document, MarkdownError};

#[test]
fn empty_parse_path_returns_error_without_panicking() {
    let call = std::panic::catch_unwind(|| parse_readme("", ""));
    assert!(call.is_ok(), "parse_readme must not panic");
    assert!(matches!(
        call.expect("no panic"),
        Err(MarkdownError::Invariant(
            DocumentScanInvariantError::EmptyPath
        ))
    ));
}

#[test]
fn scan_is_bound_to_exact_path_length_and_digest() {
    let source = "# 見出し\n\n日本語の本文です。\n";
    let scan = scan_document("README.md", source).expect("scan source");
    scan.validate_against_source("README.md", source.as_bytes())
        .expect("exact source binding");
    assert_eq!(
        scan.validate_against_source("docs/README.md", source.as_bytes()),
        Err(DocumentScanInvariantError::SourcePathMismatch)
    );
    let changed = source.replace("本文", "説明");
    assert_eq!(changed.len(), source.len());
    assert_eq!(
        scan.validate_against_source("README.md", changed.as_bytes()),
        Err(DocumentScanInvariantError::SourceDigestMismatch)
    );
    assert_eq!(
        scan.validate_against_source("README.md", b"short"),
        Err(DocumentScanInvariantError::SourceLengthMismatch)
    );
}

#[test]
fn deserialized_span_cannot_split_a_utf8_code_point() {
    let source = "# 見出し\n\n日本語の本文です。\n";
    let scan = scan_document("README.md", source).expect("scan source");
    let mut wire = serde_json::to_value(&scan).expect("scan wire");
    let events = wire["events"].as_array_mut().expect("events array");
    let (event_index, prose) = events
        .iter_mut()
        .enumerate()
        .find(|(_, event)| {
            event["kind"] == "visible_prose" && event["data"]["text"] == "日本語の本文です。"
        })
        .expect("Japanese prose event");
    let byte_start = prose["data"]["span"]["byte_start"]
        .as_u64()
        .expect("byte start");
    prose["data"]["span"]["byte_start"] = serde_json::Value::from(byte_start + 1);
    let tampered: DocumentScan =
        serde_json::from_value(wire).expect("structural wire remains bounded");
    assert_eq!(
        tampered.validate_against_source("README.md", source.as_bytes()),
        Err(DocumentScanInvariantError::EventSpanNotCharBoundary { event_index })
    );
}

#[test]
fn line_and_column_are_recomputed_from_the_bound_source() {
    let source = "# 見出し\n\n日本語の本文です。\n";
    let scan = scan_document("README.md", source).expect("scan source");
    let mut wire = serde_json::to_value(&scan).expect("scan wire");
    let events = wire["events"].as_array_mut().expect("events array");
    let (event_index, prose) = events
        .iter_mut()
        .enumerate()
        .find(|(_, event)| {
            event["kind"] == "visible_prose" && event["data"]["text"] == "日本語の本文です。"
        })
        .expect("Japanese prose event");
    prose["data"]["span"]["column"] = serde_json::Value::from(2);
    let tampered: DocumentScan =
        serde_json::from_value(wire).expect("structural wire remains bounded");
    assert_eq!(
        tampered.validate_against_source("README.md", source.as_bytes()),
        Err(DocumentScanInvariantError::EventLineColumnMismatch { event_index })
    );
}

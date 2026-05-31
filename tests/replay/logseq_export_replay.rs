//! Replay verification tests for logseq_export.
//!
//! Loads JSONL event fixtures and verifies that logseq_export events conform to
//! the approved event schema (events/logseq_export_schema.md): required fields,
//! valid event types, correct payload shapes, and valid event sequences.

use serde_json::Value;

fn load_fixture(name: &str) -> Vec<Value> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/replay/fixtures")
        .join(name);
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("Could not read fixture: {}", path.display()));
    content
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

fn le_events(all: &[Value]) -> Vec<&Value> {
    all.iter()
        .filter(|e| e["source_module"].as_str() == Some("logseq_export"))
        .collect()
}

const VALID_EVENT_TYPES: &[&str] = &[
    "ExportRequested",
    "ExportCompleted",
    "ExportFailedEmptyRecord",
    "ExportFailedOutputUnavailable",
    "ExportFailedRecordUnreadable",
];

// ── Schema conformance ────────────────────────────────────────────────────────

#[test]
fn test_happy_path_all_le_events_have_required_base_fields() {
    let all = load_fixture("logseq_export_happy_path.jsonl");
    let events = le_events(&all);
    assert!(!events.is_empty(), "Fixture must contain logseq_export events");

    for event in &events {
        let t = event["event_type"].as_str().unwrap_or("unknown");
        assert!(event["event_id"].as_str().is_some(),        "{}: event_id must be a string", t);
        assert!(event["event_type"].as_str().is_some(),      "{}: event_type must be a string", t);
        assert!(event["timestamp"].as_u64().is_some(),       "{}: timestamp must be a u64", t);
        assert!(event["correlation_id"].as_str().is_some(),  "{}: correlation_id must be a string", t);
        assert!(event["source_module"].as_str().is_some(),   "{}: source_module must be a string", t);
        assert!(event["payload"].is_object(),                "{}: payload must be an object", t);
        assert_eq!(
            event["source_module"].as_str().unwrap(),
            "logseq_export",
            "{}: source_module must be 'logseq_export'",
            t
        );
        assert!(
            event["timestamp"].as_u64().unwrap() > 0,
            "{}: timestamp must be positive",
            t
        );
    }
}

#[test]
fn test_happy_path_event_types_are_schema_members() {
    let all = load_fixture("logseq_export_happy_path.jsonl");
    let events = le_events(&all);
    for event in &events {
        let t = event["event_type"].as_str().unwrap();
        assert!(
            VALID_EVENT_TYPES.contains(&t),
            "Event type '{}' is not in the approved logseq_export schema",
            t
        );
    }
}

#[test]
fn test_happy_path_no_failure_events() {
    let all = load_fixture("logseq_export_happy_path.jsonl");
    let events = le_events(&all);
    for event in &events {
        let t = event["event_type"].as_str().unwrap();
        assert!(
            !t.contains("Failed"),
            "Happy path must not contain failure event '{}'",
            t
        );
    }
}

// ── Sequence conformance ──────────────────────────────────────────────────────

#[test]
fn test_happy_path_export_requested_before_completed() {
    let all = load_fixture("logseq_export_happy_path.jsonl");
    let events = le_events(&all);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"ExportRequested"), "Fixture must contain ExportRequested");
    assert!(types.contains(&"ExportCompleted"), "Fixture must contain ExportCompleted");

    let req_pos = types.iter().position(|&t| t == "ExportRequested").unwrap();
    let cmp_pos = types.iter().position(|&t| t == "ExportCompleted").unwrap();
    assert!(req_pos < cmp_pos, "ExportRequested must precede ExportCompleted");
}

// ── Correlation ID conformance ────────────────────────────────────────────────

#[test]
fn test_happy_path_all_events_share_correlation_id() {
    let all = load_fixture("logseq_export_happy_path.jsonl");
    let events = le_events(&all);
    assert!(events.len() >= 2, "Fixture must contain at least 2 logseq_export events");

    let first_cid = events[0]["correlation_id"].as_str().unwrap();
    for event in &events {
        assert_eq!(
            event["correlation_id"].as_str().unwrap(),
            first_cid,
            "All events in a single export invocation must share the same correlation_id"
        );
    }
}

#[test]
fn test_happy_path_correlation_id_is_non_empty() {
    let all = load_fixture("logseq_export_happy_path.jsonl");
    let events = le_events(&all);
    for event in &events {
        let cid = event["correlation_id"].as_str().unwrap();
        assert!(!cid.is_empty(), "correlation_id must not be empty");
        assert!(cid.len() > 8, "correlation_id must be a substantial identifier");
    }
}

// ── Payload shape conformance ─────────────────────────────────────────────────

#[test]
fn test_happy_path_export_requested_payload() {
    let all = load_fixture("logseq_export_happy_path.jsonl");
    let events = le_events(&all);
    let event = events
        .iter()
        .find(|e| e["event_type"] == "ExportRequested")
        .expect("ExportRequested must be present");

    assert!(
        event["payload"]["output_dir"].as_str().is_some(),
        "ExportRequested payload must contain output_dir string"
    );
}

#[test]
fn test_happy_path_export_completed_payload() {
    let all = load_fixture("logseq_export_happy_path.jsonl");
    let events = le_events(&all);
    let event = events
        .iter()
        .find(|e| e["event_type"] == "ExportCompleted")
        .expect("ExportCompleted must be present");

    let p = &event["payload"];
    assert!(p["output_dir"].as_str().is_some(),         "output_dir must be a string");
    assert!(p["item_count"].as_u64().is_some(),         "item_count must be a u64");
    assert!(p["item_count"].as_u64().unwrap() > 0,      "item_count must be greater than zero");
    assert!(p["pages_written"].as_array().is_some(),    "pages_written must be an array");
    assert!(
        !p["pages_written"].as_array().unwrap().is_empty(),
        "pages_written must not be empty on a successful export"
    );
}

#[test]
fn test_happy_path_item_count_matches_pages_written_length() {
    let all = load_fixture("logseq_export_happy_path.jsonl");
    let events = le_events(&all);
    let completed = events
        .iter()
        .find(|e| e["event_type"] == "ExportCompleted")
        .unwrap();

    let item_count = completed["payload"]["item_count"].as_u64().unwrap() as usize;
    let pages_written = completed["payload"]["pages_written"].as_array().unwrap().len();
    assert_eq!(
        item_count, pages_written,
        "item_count must equal the number of entries in pages_written"
    );
}

#[test]
fn test_happy_path_export_requested_output_dir_matches_completed() {
    let all = load_fixture("logseq_export_happy_path.jsonl");
    let events = le_events(&all);

    let requested = events
        .iter()
        .find(|e| e["event_type"] == "ExportRequested")
        .unwrap();
    let completed = events
        .iter()
        .find(|e| e["event_type"] == "ExportCompleted")
        .unwrap();

    assert_eq!(
        requested["payload"]["output_dir"].as_str().unwrap(),
        completed["payload"]["output_dir"].as_str().unwrap(),
        "output_dir in ExportRequested and ExportCompleted must match"
    );
}

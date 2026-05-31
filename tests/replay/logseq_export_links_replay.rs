//! Replay verification tests for logseq_export_links (F8).
//!
//! Loads the JSONL fixture and verifies that the logseq_export event stream
//! conforms to the approved schema: required base fields, valid event types,
//! correct sequence, and no failure events in the happy path.
//! Also verifies that the fixture contains ItemLinked events from item_links
//! to confirm the link-rendering scenario is represented.

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
    let all = load_fixture("logseq_export_links_happy_path.jsonl");
    let events = le_events(&all);
    assert!(!events.is_empty(), "Fixture must contain logseq_export events");

    for event in &events {
        let t = event["event_type"].as_str().unwrap_or("unknown");
        assert!(event["event_id"].as_str().is_some(),       "{}: event_id must be a string", t);
        assert!(event["event_type"].as_str().is_some(),     "{}: event_type must be a string", t);
        assert!(event["timestamp"].as_u64().is_some(),      "{}: timestamp must be a u64", t);
        assert!(event["correlation_id"].as_str().is_some(), "{}: correlation_id must be a string", t);
        assert!(event["source_module"].as_str().is_some(),  "{}: source_module must be a string", t);
        assert!(event["payload"].is_object(),               "{}: payload must be an object", t);
        assert_eq!(event["source_module"].as_str().unwrap(), "logseq_export",
            "{}: source_module must be 'logseq_export'", t);
        assert!(event["timestamp"].as_u64().unwrap() > 0,
            "{}: timestamp must be positive", t);
    }
}

#[test]
fn test_happy_path_event_types_are_schema_members() {
    let all = load_fixture("logseq_export_links_happy_path.jsonl");
    let events = le_events(&all);
    for event in &events {
        let t = event["event_type"].as_str().unwrap();
        assert!(VALID_EVENT_TYPES.contains(&t),
            "Event type '{}' is not in the approved logseq_export schema", t);
    }
}

#[test]
fn test_happy_path_no_failure_events() {
    let all = load_fixture("logseq_export_links_happy_path.jsonl");
    let events = le_events(&all);
    for event in &events {
        let t = event["event_type"].as_str().unwrap();
        assert!(!t.starts_with("ExportFailed"),
            "Happy path fixture must not contain failure event '{}'", t);
    }
}

// ── Sequence conformance ──────────────────────────────────────────────────────

#[test]
fn test_happy_path_requested_before_completed() {
    let all = load_fixture("logseq_export_links_happy_path.jsonl");
    let events = le_events(&all);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"ExportRequested"), "Fixture must contain ExportRequested");
    assert!(types.contains(&"ExportCompleted"), "Fixture must contain ExportCompleted");

    let req_pos = types.iter().position(|&t| t == "ExportRequested").unwrap();
    let cmp_pos = types.iter().position(|&t| t == "ExportCompleted").unwrap();
    assert!(req_pos < cmp_pos, "ExportRequested must precede ExportCompleted");
}

#[test]
fn test_happy_path_requested_and_completed_share_correlation_id() {
    let all = load_fixture("logseq_export_links_happy_path.jsonl");
    let events = le_events(&all);

    let req = events.iter().find(|e| e["event_type"] == "ExportRequested").unwrap();
    let cmp = events.iter().find(|e| e["event_type"] == "ExportCompleted").unwrap();

    assert_eq!(
        req["correlation_id"].as_str().unwrap(),
        cmp["correlation_id"].as_str().unwrap(),
        "ExportRequested and ExportCompleted must share correlation_id"
    );
}

// ── Payload shape conformance ─────────────────────────────────────────────────

#[test]
fn test_happy_path_export_completed_payload_shape() {
    let all = load_fixture("logseq_export_links_happy_path.jsonl");
    let events = le_events(&all);
    let completed = events.iter().find(|e| e["event_type"] == "ExportCompleted").unwrap();
    let p = &completed["payload"];

    assert!(p["output_dir"].as_str().is_some(),    "output_dir must be a string");
    assert!(p["item_count"].as_u64().is_some(),    "item_count must be a u64");
    assert!(p["pages_written"].is_array(),         "pages_written must be an array");
    assert_eq!(
        p["item_count"].as_u64().unwrap() as usize,
        p["pages_written"].as_array().unwrap().len(),
        "item_count must equal pages_written array length"
    );
}

// ── Link context: fixture must represent a link-rendering scenario ───────────

#[test]
fn test_happy_path_fixture_contains_item_linked_event() {
    let all = load_fixture("logseq_export_links_happy_path.jsonl");
    let linked: Vec<&Value> = all.iter()
        .filter(|e| {
            e["source_module"].as_str() == Some("item_links")
                && e["event_type"].as_str() == Some("ItemLinked")
        })
        .collect();

    assert!(!linked.is_empty(),
        "Fixture must contain at least one ItemLinked event to represent the link-rendering scenario");
}

#[test]
fn test_happy_path_item_linked_precedes_export_requested() {
    let all = load_fixture("logseq_export_links_happy_path.jsonl");

    let linked_pos = all.iter().position(|e| {
        e["source_module"].as_str() == Some("item_links")
            && e["event_type"].as_str() == Some("ItemLinked")
    }).expect("ItemLinked must be in fixture");

    let export_pos = all.iter().position(|e| {
        e["event_type"].as_str() == Some("ExportRequested")
    }).expect("ExportRequested must be in fixture");

    assert!(linked_pos < export_pos,
        "ItemLinked must precede ExportRequested — links exist before export runs");
}

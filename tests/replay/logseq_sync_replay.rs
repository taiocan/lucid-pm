//! Replay verification tests for logseq_sync.
//!
//! Loads JSONL event fixtures and verifies that logseq_sync events conform to
//! the approved event schema (events/logseq_sync_schema.md): required fields,
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

fn ls_events(all: &[Value]) -> Vec<&Value> {
    all.iter()
        .filter(|e| e["source_module"].as_str() == Some("logseq_sync"))
        .collect()
}

const VALID_EVENT_TYPES: &[&str] = &[
    "SyncRequested",
    "ItemStatusUpdated",
    "ItemPriorityUpdated",
    "ItemSyncSkippedInvalidStatus",
    "SyncCompleted",
    "SyncCompletedNoChanges",
    "SyncFailedGraphNotAccessible",
    "SyncFailedEmptyRecord",
];

// ── Schema conformance ────────────────────────────────────────────────────────

#[test]
fn test_happy_path_all_ls_events_have_required_base_fields() {
    let all = load_fixture("logseq_sync_happy_path.jsonl");
    let events = ls_events(&all);
    assert!(!events.is_empty(), "Fixture must contain logseq_sync events");

    for event in &events {
        let t = event["event_type"].as_str().unwrap_or("unknown");
        assert!(event["event_id"].as_str().is_some(),       "{}: event_id must be a string", t);
        assert!(event["event_type"].as_str().is_some(),     "{}: event_type must be a string", t);
        assert!(event["timestamp"].as_u64().is_some(),      "{}: timestamp must be a u64", t);
        assert!(event["correlation_id"].as_str().is_some(), "{}: correlation_id must be a string", t);
        assert!(event["source_module"].as_str().is_some(),  "{}: source_module must be a string", t);
        assert!(event["payload"].is_object(),               "{}: payload must be an object", t);
        assert_eq!(
            event["source_module"].as_str().unwrap(),
            "logseq_sync",
            "{}: source_module must be 'logseq_sync'",
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
    let all = load_fixture("logseq_sync_happy_path.jsonl");
    let events = ls_events(&all);
    for event in &events {
        let t = event["event_type"].as_str().unwrap();
        assert!(
            VALID_EVENT_TYPES.contains(&t),
            "Event type '{}' is not in the approved logseq_sync schema",
            t
        );
    }
}

#[test]
fn test_happy_path_no_failure_events() {
    let all = load_fixture("logseq_sync_happy_path.jsonl");
    let events = ls_events(&all);
    for event in &events {
        let t = event["event_type"].as_str().unwrap();
        assert!(
            !t.starts_with("SyncFailed") && !t.contains("Skipped"),
            "Happy path must not contain failure event '{}'",
            t
        );
    }
}

// ── Sequence conformance ──────────────────────────────────────────────────────

#[test]
fn test_happy_path_sync_requested_before_completed() {
    let all = load_fixture("logseq_sync_happy_path.jsonl");
    let events = ls_events(&all);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"SyncRequested"), "Fixture must contain SyncRequested");
    assert!(types.contains(&"SyncCompleted"), "Fixture must contain SyncCompleted");

    let req_pos = types.iter().position(|&t| t == "SyncRequested").unwrap();
    let cmp_pos = types.iter().position(|&t| t == "SyncCompleted").unwrap();
    assert!(req_pos < cmp_pos, "SyncRequested must precede SyncCompleted");
}

#[test]
fn test_happy_path_item_updates_between_requested_and_completed() {
    let all = load_fixture("logseq_sync_happy_path.jsonl");
    let events = ls_events(&all);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    let req_pos = types.iter().position(|&t| t == "SyncRequested").unwrap();
    let cmp_pos = types.iter().position(|&t| t == "SyncCompleted").unwrap();

    let update_events = &types[req_pos + 1..cmp_pos];
    assert!(
        update_events.iter().any(|&t| t == "ItemStatusUpdated" || t == "ItemPriorityUpdated"),
        "Happy path must have at least one item update between SyncRequested and SyncCompleted"
    );
}

// ── Correlation ID conformance ────────────────────────────────────────────────

#[test]
fn test_happy_path_all_events_share_correlation_id() {
    let all = load_fixture("logseq_sync_happy_path.jsonl");
    let events = ls_events(&all);
    assert!(events.len() >= 2, "Fixture must contain at least 2 logseq_sync events");

    let first_cid = events[0]["correlation_id"].as_str().unwrap();
    for event in &events {
        assert_eq!(
            event["correlation_id"].as_str().unwrap(),
            first_cid,
            "All events in a single sync invocation must share the same correlation_id"
        );
    }
}

#[test]
fn test_happy_path_correlation_id_is_non_empty() {
    let all = load_fixture("logseq_sync_happy_path.jsonl");
    let events = ls_events(&all);
    for event in &events {
        let cid = event["correlation_id"].as_str().unwrap();
        assert!(!cid.is_empty(), "correlation_id must not be empty");
        assert!(cid.len() > 8, "correlation_id must be a substantial identifier");
    }
}

// ── Payload shape conformance ─────────────────────────────────────────────────

#[test]
fn test_happy_path_sync_requested_payload() {
    let all = load_fixture("logseq_sync_happy_path.jsonl");
    let events = ls_events(&all);
    let event = events
        .iter()
        .find(|e| e["event_type"] == "SyncRequested")
        .expect("SyncRequested must be present");

    assert!(
        event["payload"]["graph_dir"].as_str().is_some(),
        "SyncRequested payload must contain graph_dir string"
    );
}

#[test]
fn test_happy_path_item_status_updated_payload() {
    let all = load_fixture("logseq_sync_happy_path.jsonl");
    let events = ls_events(&all);
    let event = events
        .iter()
        .find(|e| e["event_type"] == "ItemStatusUpdated")
        .expect("ItemStatusUpdated must be present in happy path fixture");

    let p = &event["payload"];
    assert!(p["item_id"].as_str().is_some(),       "item_id must be a string");
    assert!(p["item_type"].as_str().is_some(),     "item_type must be a string");
    assert!(p["new_status"].as_str().is_some(),    "new_status must be a string");
    assert!(!p["new_status"].as_str().unwrap().is_empty(), "new_status must not be empty");
    assert!(p.get("previous_status").is_some(),   "previous_status key must be present");
}

#[test]
fn test_happy_path_item_priority_updated_payload() {
    let all = load_fixture("logseq_sync_happy_path.jsonl");
    let events = ls_events(&all);
    let event = events
        .iter()
        .find(|e| e["event_type"] == "ItemPriorityUpdated")
        .expect("ItemPriorityUpdated must be present in happy path fixture");

    let p = &event["payload"];
    assert!(p["item_id"].as_str().is_some(),        "item_id must be a string");
    assert!(p["item_type"].as_str().is_some(),      "item_type must be a string");
    assert!(p["new_priority"].as_str().is_some(),   "new_priority must be a string");
    assert!(!p["new_priority"].as_str().unwrap().is_empty(), "new_priority must not be empty");
    assert!(p.get("previous_priority").is_some(),  "previous_priority key must be present");
}

#[test]
fn test_happy_path_sync_completed_payload() {
    let all = load_fixture("logseq_sync_happy_path.jsonl");
    let events = ls_events(&all);
    let event = events
        .iter()
        .find(|e| e["event_type"] == "SyncCompleted")
        .expect("SyncCompleted must be present");

    let p = &event["payload"];
    assert!(p["graph_dir"].as_str().is_some(),          "graph_dir must be a string");
    assert!(p["changes_applied"].as_u64().is_some(),    "changes_applied must be a u64");
    assert!(p["changes_applied"].as_u64().unwrap() > 0, "changes_applied must be > 0 in happy path");
    assert!(p["items_skipped"].as_u64().is_some(),      "items_skipped must be a u64");
}

#[test]
fn test_happy_path_graph_dir_consistent_in_requested_and_completed() {
    let all = load_fixture("logseq_sync_happy_path.jsonl");
    let events = ls_events(&all);

    let requested = events
        .iter()
        .find(|e| e["event_type"] == "SyncRequested")
        .unwrap();
    let completed = events
        .iter()
        .find(|e| e["event_type"] == "SyncCompleted")
        .unwrap();

    assert_eq!(
        requested["payload"]["graph_dir"].as_str().unwrap(),
        completed["payload"]["graph_dir"].as_str().unwrap(),
        "graph_dir in SyncRequested and SyncCompleted must match"
    );
}

#[test]
fn test_happy_path_changes_applied_matches_update_event_count() {
    let all = load_fixture("logseq_sync_happy_path.jsonl");
    let events = ls_events(&all);

    let update_count = events
        .iter()
        .filter(|e| {
            e["event_type"] == "ItemStatusUpdated" || e["event_type"] == "ItemPriorityUpdated"
        })
        .count() as u64;

    let completed = events
        .iter()
        .find(|e| e["event_type"] == "SyncCompleted")
        .unwrap();
    let changes_applied = completed["payload"]["changes_applied"].as_u64().unwrap();

    assert_eq!(
        changes_applied, update_count,
        "changes_applied must equal the number of ItemStatusUpdated + ItemPriorityUpdated events"
    );
}

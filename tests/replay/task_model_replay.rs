//! Replay verification tests for task_model.
//!
//! Loads JSONL event fixtures and verifies that task_model events conform to the
//! approved event schema (events/task_model_schema.md): required fields,
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

/// Filter to only task_model events.
fn tm_events(all: &[Value]) -> Vec<&Value> {
    all.iter()
        .filter(|e| e["source_module"].as_str() == Some("task_model"))
        .collect()
}

const VALID_EVENT_TYPES: &[&str] = &[
    "TaskAddRequested",
    "TaskAdded",
    "TaskMarkerUpdated",
    "TaskAddFailedParentNotFound",
    "TaskAddFailedSchemaInvalid",
    "TaskAddFailedTaskTypeNotDefined",
];

// ── Schema conformance ────────────────────────────────────────────────────────

#[test]
fn test_all_events_have_required_base_fields() {
    let all = load_fixture("task_model_happy_path.jsonl");
    let events = tm_events(&all);
    assert!(!events.is_empty(), "Fixture must contain task_model events");

    for event in &events {
        let t = event["event_type"].as_str().unwrap_or("unknown");
        assert!(event["event_id"].as_str().is_some(),       "{}: event_id must be a string", t);
        assert!(event["event_type"].as_str().is_some(),     "{}: event_type must be a string", t);
        assert!(event["timestamp"].as_u64().is_some(),      "{}: timestamp must be u64", t);
        assert!(event["correlation_id"].as_str().is_some(), "{}: correlation_id must be a string", t);
        assert!(event["source_module"].as_str().is_some(),  "{}: source_module must be a string", t);
        assert!(event["payload"].is_object(),               "{}: payload must be an object", t);
        assert_eq!(
            event["source_module"].as_str().unwrap(), "task_model",
            "{}: source_module must be 'task_model'", t
        );
        assert!(event["timestamp"].as_u64().unwrap() > 0, "{}: timestamp must be positive", t);
        let cid = event["correlation_id"].as_str().unwrap();
        assert!(!cid.is_empty(), "{}: correlation_id must not be empty", t);
    }
}

#[test]
fn test_event_types_are_schema_members() {
    let all = load_fixture("task_model_happy_path.jsonl");
    let events = tm_events(&all);

    for event in &events {
        let t = event["event_type"].as_str().unwrap();
        assert!(
            VALID_EVENT_TYPES.contains(&t),
            "Event type '{}' is not in the approved task_model schema", t
        );
    }
}

// ── TaskAddRequested payload shape ────────────────────────────────────────────

#[test]
fn test_task_add_requested_payload_shape() {
    let all = load_fixture("task_model_happy_path.jsonl");
    let events = tm_events(&all);

    for event in events.iter().filter(|e| e["event_type"] == "TaskAddRequested") {
        let p = &event["payload"];
        assert!(p["description"].as_str().is_some(),   "description must be a string");
        assert!(p["parent_item_id"].as_str().is_some(), "parent_item_id must be a string");
        // requested_marker may be null or a string
        assert!(p.get("requested_marker").is_some(),   "requested_marker must be present (may be null)");
    }
}

// ── TaskAdded payload shape ───────────────────────────────────────────────────

#[test]
fn test_task_added_payload_shape() {
    let all = load_fixture("task_model_happy_path.jsonl");
    let events = tm_events(&all);

    for event in events.iter().filter(|e| e["event_type"] == "TaskAdded") {
        let p = &event["payload"];
        assert!(p["task_id"].as_str().is_some(),        "task_id must be a string");
        assert!(p["item_type"].as_str().is_some(),       "item_type must be a string");
        assert!(p["description"].as_str().is_some(),     "description must be a string");
        assert!(p["parent_item_id"].as_str().is_some(),  "parent_item_id must be a string");
        assert!(p["initial_marker"].as_str().is_some(),  "initial_marker must be a string");

        // task_id must look like a UUID (36 chars with hyphens)
        let task_id = p["task_id"].as_str().unwrap();
        assert_eq!(task_id.len(), 36, "task_id must be UUID format");
        assert!(task_id.contains('-'), "task_id must contain hyphens");

        // initial_marker must be non-empty
        assert!(!p["initial_marker"].as_str().unwrap().is_empty(),
            "initial_marker must not be empty");
    }
}

#[test]
fn test_task_added_has_task_add_requested_fixture_contains_both() {
    let all = load_fixture("task_model_happy_path.jsonl");
    let events = tm_events(&all);

    let has_requested = events.iter().any(|e| e["event_type"] == "TaskAddRequested");
    let has_added     = events.iter().any(|e| e["event_type"] == "TaskAdded");

    assert!(has_requested, "Fixture must contain at least one TaskAddRequested");
    assert!(has_added,     "Fixture must contain at least one TaskAdded");
}

// ── TaskAddRequested always precedes TaskAdded in the same correlation chain ──

#[test]
fn test_task_add_requested_before_task_added_per_correlation_id() {
    let all = load_fixture("task_model_happy_path.jsonl");
    let events = tm_events(&all);

    // For each correlation_id that has both a TaskAddRequested and a TaskAdded,
    // the request must come before the add.
    let correlation_ids: std::collections::HashSet<&str> = events.iter()
        .filter(|e| e["event_type"] == "TaskAdded")
        .map(|e| e["correlation_id"].as_str().unwrap())
        .collect();

    for cid in correlation_ids {
        let req_pos = events.iter().position(|e|
            e["event_type"] == "TaskAddRequested"
            && e["correlation_id"].as_str() == Some(cid));
        let add_pos = events.iter().position(|e|
            e["event_type"] == "TaskAdded"
            && e["correlation_id"].as_str() == Some(cid));

        // TaskAdded events from sync discovery don't have a prior TaskAddRequested,
        // so req_pos may be None for those. Only check when both are present.
        if let (Some(rp), Some(ap)) = (req_pos, add_pos) {
            assert!(rp < ap,
                "TaskAddRequested must precede TaskAdded for correlation_id {}", cid);
        }
    }
}

// ── TaskMarkerUpdated payload shape ───────────────────────────────────────────

#[test]
fn test_task_marker_updated_payload_shape() {
    let all = load_fixture("task_model_happy_path.jsonl");
    let events = tm_events(&all);

    for event in events.iter().filter(|e| e["event_type"] == "TaskMarkerUpdated") {
        let p = &event["payload"];
        assert!(p["task_id"].as_str().is_some(),        "task_id must be a string");
        assert!(p["previous_marker"].as_str().is_some(), "previous_marker must be a string");
        assert!(p["new_marker"].as_str().is_some(),      "new_marker must be a string");

        // previous_marker and new_marker must differ
        assert_ne!(
            p["previous_marker"].as_str().unwrap(),
            p["new_marker"].as_str().unwrap(),
            "previous_marker and new_marker must differ in a TaskMarkerUpdated event"
        );
    }
}

// ── Failure event payload shapes ──────────────────────────────────────────────

#[test]
fn test_task_add_failed_parent_not_found_payload() {
    let all = load_fixture("task_model_happy_path.jsonl");
    let events = tm_events(&all);
    let event = events.iter()
        .find(|e| e["event_type"] == "TaskAddFailedParentNotFound")
        .expect("Fixture must contain TaskAddFailedParentNotFound");

    let p = &event["payload"];
    assert_eq!(p["failure_reason"].as_str().unwrap(), "parent_not_found",
        "failure_reason must be 'parent_not_found'");
    assert!(p["parent_item_id"].as_str().is_some(),
        "parent_item_id must be present in TaskAddFailedParentNotFound");
}

#[test]
fn test_task_add_failed_schema_invalid_payload() {
    let all = load_fixture("task_model_happy_path.jsonl");
    let events = tm_events(&all);
    let event = events.iter()
        .find(|e| e["event_type"] == "TaskAddFailedSchemaInvalid")
        .expect("Fixture must contain TaskAddFailedSchemaInvalid");

    let p = &event["payload"];
    assert_eq!(p["failure_reason"].as_str().unwrap(), "schema_invalid",
        "failure_reason must be 'schema_invalid'");
}

#[test]
fn test_task_add_failed_task_type_not_defined_payload() {
    let all = load_fixture("task_model_happy_path.jsonl");
    let events = tm_events(&all);
    let event = events.iter()
        .find(|e| e["event_type"] == "TaskAddFailedTaskTypeNotDefined")
        .expect("Fixture must contain TaskAddFailedTaskTypeNotDefined");

    let p = &event["payload"];
    assert_eq!(p["failure_reason"].as_str().unwrap(), "task_type_not_defined",
        "failure_reason must be 'task_type_not_defined'");
}

// ── No failure events on happy path ──────────────────────────────────────────

#[test]
fn test_happy_path_task_added_events_have_no_failure_type() {
    let all = load_fixture("task_model_happy_path.jsonl");
    let events = tm_events(&all);

    // Failure events are present in the fixture (test them separately).
    // TaskAdded events must not carry failure payload fields.
    for event in events.iter().filter(|e| e["event_type"] == "TaskAdded") {
        let p = &event["payload"];
        assert!(p.get("failure_reason").is_none(),
            "TaskAdded must not contain a failure_reason field");
    }
}

// ── Correlation ID consistency ────────────────────────────────────────────────

#[test]
fn test_correlation_ids_are_non_empty_uuids() {
    let all = load_fixture("task_model_happy_path.jsonl");
    let events = tm_events(&all);

    for event in &events {
        let cid = event["correlation_id"].as_str()
            .expect("correlation_id must be a string");
        assert_eq!(cid.len(), 36, "correlation_id must be UUID length (36)");
        assert!(cid.contains('-'), "correlation_id must contain hyphens");
    }
}

#[test]
fn test_task_added_task_id_differs_from_correlation_id() {
    let all = load_fixture("task_model_happy_path.jsonl");
    let events = tm_events(&all);

    for event in events.iter().filter(|e| e["event_type"] == "TaskAdded") {
        let task_id = event["payload"]["task_id"].as_str().unwrap();
        let cid     = event["correlation_id"].as_str().unwrap();
        assert_ne!(task_id, cid,
            "task_id and correlation_id must be distinct identifiers");
    }
}

// ── All fixture events are task_model source ──────────────────────────────────

#[test]
fn test_all_fixture_events_have_correct_source_module() {
    let all = load_fixture("task_model_happy_path.jsonl");
    for event in &all {
        assert_eq!(
            event["source_module"].as_str().unwrap_or(""),
            "task_model",
            "All fixture events must have source_module 'task_model'; \
             got '{}'", event["source_module"]
        );
    }
}

// ── Timestamps are monotonically non-decreasing ───────────────────────────────

#[test]
fn test_timestamps_are_monotonically_nondecreasing() {
    let all = load_fixture("task_model_happy_path.jsonl");
    let events = tm_events(&all);
    let timestamps: Vec<u64> = events.iter()
        .map(|e| e["timestamp"].as_u64().unwrap())
        .collect();

    for window in timestamps.windows(2) {
        assert!(window[0] <= window[1],
            "Timestamps must be non-decreasing: {} > {}", window[0], window[1]);
    }
}

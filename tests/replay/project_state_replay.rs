//! Replay verification tests for project_state.
//!
//! Loads JSONL event fixtures and verifies that project_state events conform to
//! the approved event schema (events/project_state_schema.md): required fields,
//! valid event types, correct payload shapes, and valid event sequences.
//! All event type names must match project_state_schema.md exactly.

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

/// Filter the fixture to only project_state events.
fn ps_events(all: &[Value]) -> Vec<&Value> {
    all.iter()
        .filter(|e| e["source_module"].as_str() == Some("project_state"))
        .collect()
}

const VALID_EVENT_TYPES: &[&str] = &[
    "IncorporationRequested",
    "ItemsIncorporated",
    "IncorporationFailedDuplicate",
    "RecordQueried",
    "RecordReturned",
    "RecordQueryFailedEmpty",
    "RecordQueryFailedSchemaInvalid",  // R10: schema load failure
];

// Canonical names (R10 and later) + legacy lowercase aliases (pre-R10 fixtures).
const VALID_ITEM_TYPES: &[&str] = &[
    "Task", "Milestone", "Risk", "Issue", "Stakeholder", "WorkPackage",
    "task", "milestone", "risk", "issue", "stakeholder", "workpackage",
];

// ── Schema conformance ────────────────────────────────────────────────────────

#[test]
fn test_happy_path_fixture_all_ps_events_have_required_base_fields() {
    let all = load_fixture("project_state_happy_path.jsonl");
    let events = ps_events(&all);
    assert!(!events.is_empty(), "Fixture must contain project_state events");

    for event in &events {
        let t = event["event_type"].as_str().unwrap_or("unknown");
        assert!(event["event_id"].as_str().is_some(),       "{}: event_id must be a string", t);
        assert!(event["event_type"].as_str().is_some(),     "{}: event_type must be a string", t);
        assert!(event["timestamp"].as_u64().is_some(),      "{}: timestamp must be a u64", t);
        assert!(event["correlation_id"].as_str().is_some(), "{}: correlation_id must be a string", t);
        assert!(event["source_module"].as_str().is_some(),  "{}: source_module must be a string", t);
        assert!(event["payload"].is_object(),               "{}: payload must be an object", t);
        assert_eq!(
            event["source_module"].as_str().unwrap(), "project_state",
            "{}: source_module must be 'project_state'", t
        );
        assert!(event["timestamp"].as_u64().unwrap() > 0, "{}: timestamp must be positive", t);
    }
}

#[test]
fn test_happy_path_fixture_event_types_are_schema_members() {
    let all = load_fixture("project_state_happy_path.jsonl");
    let events = ps_events(&all);
    for event in &events {
        let t = event["event_type"].as_str().unwrap();
        assert!(VALID_EVENT_TYPES.contains(&t),
            "Event type '{}' is not in the approved project_state schema", t);
    }
}

#[test]
fn test_happy_path_fixture_no_failure_events() {
    let all = load_fixture("project_state_happy_path.jsonl");
    let events = ps_events(&all);
    let types: Vec<&str> = events.iter()
        .map(|e| e["event_type"].as_str().unwrap())
        .collect();

    assert!(!types.contains(&"IncorporationFailedDuplicate"),
        "Happy path must not contain IncorporationFailedDuplicate");
    assert!(!types.contains(&"RecordQueryFailedEmpty"),
        "Happy path must not contain RecordQueryFailedEmpty");
}

// ── Sequence conformance ──────────────────────────────────────────────────────

#[test]
fn test_happy_path_fixture_incorporation_requested_before_incorporated() {
    let all = load_fixture("project_state_happy_path.jsonl");
    let events = ps_events(&all);
    let types: Vec<&str> = events.iter()
        .map(|e| e["event_type"].as_str().unwrap())
        .collect();

    assert!(types.contains(&"IncorporationRequested"), "Fixture must contain IncorporationRequested");
    assert!(types.contains(&"ItemsIncorporated"),      "Fixture must contain ItemsIncorporated");

    let req_pos = types.iter().position(|&t| t == "IncorporationRequested").unwrap();
    let inc_pos = types.iter().position(|&t| t == "ItemsIncorporated").unwrap();
    assert!(req_pos < inc_pos, "IncorporationRequested must precede ItemsIncorporated");
}

#[test]
fn test_happy_path_fixture_record_queried_before_returned() {
    let all = load_fixture("project_state_happy_path.jsonl");
    let events = ps_events(&all);
    let types: Vec<&str> = events.iter()
        .map(|e| e["event_type"].as_str().unwrap())
        .collect();

    assert!(types.contains(&"RecordQueried"),  "Fixture must contain RecordQueried");
    assert!(types.contains(&"RecordReturned"), "Fixture must contain RecordReturned");

    let queried_pos  = types.iter().position(|&t| t == "RecordQueried").unwrap();
    let returned_pos = types.iter().position(|&t| t == "RecordReturned").unwrap();
    assert!(queried_pos < returned_pos, "RecordQueried must precede RecordReturned");
}

#[test]
fn test_happy_path_fixture_incorporate_and_view_have_different_correlation_ids() {
    let all = load_fixture("project_state_happy_path.jsonl");
    let events = ps_events(&all);

    let inc_req = events.iter().find(|e| e["event_type"] == "IncorporationRequested").unwrap();
    let rec_queried = events.iter().find(|e| e["event_type"] == "RecordQueried").unwrap();

    assert_ne!(
        inc_req["correlation_id"].as_str().unwrap(),
        rec_queried["correlation_id"].as_str().unwrap(),
        "Incorporate and view invocations must have different correlation_ids"
    );
}

// ── Correlation ID consistency within each invocation ─────────────────────────

#[test]
fn test_happy_path_fixture_incorporate_invocation_shares_correlation_id() {
    let all = load_fixture("project_state_happy_path.jsonl");
    let events = ps_events(&all);

    let inc_req = events.iter()
        .find(|e| e["event_type"] == "IncorporationRequested")
        .expect("IncorporationRequested must be present");
    let inc_done = events.iter()
        .find(|e| e["event_type"] == "ItemsIncorporated")
        .expect("ItemsIncorporated must be present");

    assert_eq!(
        inc_req["correlation_id"].as_str().unwrap(),
        inc_done["correlation_id"].as_str().unwrap(),
        "IncorporationRequested and ItemsIncorporated must share the same correlation_id"
    );
}

#[test]
fn test_happy_path_fixture_view_invocation_shares_correlation_id() {
    let all = load_fixture("project_state_happy_path.jsonl");
    let events = ps_events(&all);

    let queried = events.iter()
        .find(|e| e["event_type"] == "RecordQueried")
        .expect("RecordQueried must be present");
    let returned = events.iter()
        .find(|e| e["event_type"] == "RecordReturned")
        .expect("RecordReturned must be present");

    assert_eq!(
        queried["correlation_id"].as_str().unwrap(),
        returned["correlation_id"].as_str().unwrap(),
        "RecordQueried and RecordReturned must share the same correlation_id"
    );
}

// ── Payload shape conformance ─────────────────────────────────────────────────

#[test]
fn test_happy_path_fixture_incorporation_requested_payload() {
    let all = load_fixture("project_state_happy_path.jsonl");
    let events = ps_events(&all);
    let event = events.iter()
        .find(|e| e["event_type"] == "IncorporationRequested")
        .unwrap();

    assert!(event["payload"]["session_id"].as_str().is_some(),
        "IncorporationRequested.session_id must be a string");
}

#[test]
fn test_happy_path_fixture_items_incorporated_payload() {
    let all = load_fixture("project_state_happy_path.jsonl");
    let events = ps_events(&all);
    let event = events.iter()
        .find(|e| e["event_type"] == "ItemsIncorporated")
        .unwrap();

    assert!(event["payload"]["session_id"].as_str().is_some(),
        "ItemsIncorporated.session_id must be a string");
    assert!(event["payload"]["incorporated_count"].as_u64().is_some(),
        "ItemsIncorporated.incorporated_count must be a u64");
    assert!(event["payload"]["total_record_size"].as_u64().is_some(),
        "ItemsIncorporated.total_record_size must be a u64");

    let inc_count   = event["payload"]["incorporated_count"].as_u64().unwrap();
    let total_size  = event["payload"]["total_record_size"].as_u64().unwrap();
    assert!(inc_count > 0,             "incorporated_count must be positive");
    assert!(total_size >= inc_count,   "total_record_size must be >= incorporated_count");
}

#[test]
fn test_happy_path_fixture_record_returned_payload() {
    let all = load_fixture("project_state_happy_path.jsonl");
    let events = ps_events(&all);
    let event = events.iter()
        .find(|e| e["event_type"] == "RecordReturned")
        .unwrap();

    let items = event["payload"]["items"].as_array()
        .expect("RecordReturned.items must be an array");
    let total_count   = event["payload"]["total_count"].as_u64()
        .expect("RecordReturned.total_count must be a u64");
    let session_count = event["payload"]["session_count"].as_u64()
        .expect("RecordReturned.session_count must be a u64");

    assert_eq!(items.len() as u64, total_count,
        "total_count must equal actual items array length");
    assert!(session_count > 0, "session_count must be positive");
    assert!(session_count <= total_count, "session_count cannot exceed total_count");
}

#[test]
fn test_happy_path_fixture_record_returned_item_shapes() {
    let all = load_fixture("project_state_happy_path.jsonl");
    let events = ps_events(&all);
    let event = events.iter()
        .find(|e| e["event_type"] == "RecordReturned")
        .unwrap();

    let items = event["payload"]["items"].as_array().unwrap();
    assert!(!items.is_empty(), "RecordReturned items must not be empty in happy path");

    for item in items {
        assert!(item["item_id"].as_str().is_some(),    "item must have item_id string");
        assert!(item["description"].as_str().is_some(),"item must have description string");
        assert!(item["uncertain"].as_bool().is_some(), "item must have uncertain bool");
        assert!(item["session_id"].as_str().is_some(), "item must have session_id string");

        let item_type = item["item_type"].as_str().expect("item must have item_type string");
        assert!(VALID_ITEM_TYPES.contains(&item_type),
            "item_type '{}' not in schema", item_type);
    }
}

#[test]
fn test_happy_path_fixture_items_incorporated_session_matches_extraction() {
    let all = load_fixture("project_state_happy_path.jsonl");
    let events = ps_events(&all);

    let inc_done = events.iter()
        .find(|e| e["event_type"] == "ItemsIncorporated")
        .unwrap();
    let inc_req = events.iter()
        .find(|e| e["event_type"] == "IncorporationRequested")
        .unwrap();

    assert_eq!(
        inc_req["payload"]["session_id"].as_str().unwrap(),
        inc_done["payload"]["session_id"].as_str().unwrap(),
        "ItemsIncorporated.session_id must match IncorporationRequested.session_id"
    );

    // session_id in project_state events must match a pm_structuring ExtractionConfirmed correlation_id
    let session_id = inc_done["payload"]["session_id"].as_str().unwrap();
    let pm_confirmed = all.iter()
        .find(|e| e["event_type"] == "ExtractionConfirmed" && e["correlation_id"] == session_id);
    assert!(pm_confirmed.is_some(),
        "session_id '{}' must match a pm_structuring ExtractionConfirmed correlation_id", session_id);
}

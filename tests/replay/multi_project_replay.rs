//! Replay verification tests for multi_project.
//!
//! Loads JSONL event fixtures and verifies that multi_project events conform to
//! the approved event schema (events/multi_project_schema.md): required fields,
//! valid event types, correct payload shapes, and valid event sequences.

use project_schema::test_support::load_fixture;
use serde_json::Value;

fn mp_events(all: &[Value]) -> Vec<&Value> {
    all.iter()
        .filter(|e| e["source_module"].as_str() == Some("multi_project"))
        .collect()
}

const VALID_EVENT_TYPES: &[&str] = &[
    "ProjectInitRequested",
    "ProjectInitialized",
    "ProjectInitFailedDuplicate",
    "ProjectInitFailedDirectoryNotAccessible",
    "ProjectListRequested",
    "ProjectListReturned",
    "ProjectOpenRequested",
    "ProjectPathReturned",
    "ProjectOpenFailedNotFound",
];

// ── Schema conformance ────────────────────────────────────────────────────────

#[test]
fn test_happy_path_all_events_have_required_base_fields() {
    let all = load_fixture("multi_project_happy_path.jsonl");
    let events = mp_events(&all);
    assert!(!events.is_empty(), "Fixture must contain multi_project events");

    for event in &events {
        let t = event["event_type"].as_str().unwrap_or("unknown");
        assert!(event["event_id"].as_str().is_some(),       "{t}: event_id must be a string");
        assert!(event["event_type"].as_str().is_some(),     "{t}: event_type must be a string");
        assert!(event["timestamp"].as_u64().is_some(),      "{t}: timestamp must be a u64");
        assert!(event["correlation_id"].as_str().is_some(), "{t}: correlation_id must be a string");
        assert!(event["source_module"].as_str().is_some(),  "{t}: source_module must be a string");
        assert!(event["payload"].is_object(),               "{t}: payload must be an object");
        assert_eq!(
            event["source_module"].as_str().unwrap(), "multi_project",
            "{t}: source_module must be 'multi_project'"
        );
        assert!(event["timestamp"].as_u64().unwrap() > 0,  "{t}: timestamp must be positive");
    }
}

#[test]
fn test_happy_path_event_types_are_schema_members() {
    let all = load_fixture("multi_project_happy_path.jsonl");
    let events = mp_events(&all);
    for event in &events {
        let t = event["event_type"].as_str().unwrap();
        assert!(
            VALID_EVENT_TYPES.contains(&t),
            "Event type '{t}' is not in the approved multi_project schema"
        );
    }
}

#[test]
fn test_happy_path_no_failure_events() {
    let all = load_fixture("multi_project_happy_path.jsonl");
    let events = mp_events(&all);
    for event in &events {
        let t = event["event_type"].as_str().unwrap();
        assert!(
            !t.contains("Failed"),
            "Happy path must not contain failure event '{t}'"
        );
    }
}

// ── Sequence conformance ──────────────────────────────────────────────────────

#[test]
fn test_happy_path_init_requested_before_initialized() {
    let all = load_fixture("multi_project_happy_path.jsonl");
    let events = mp_events(&all);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"ProjectInitRequested"), "Fixture must contain ProjectInitRequested");
    assert!(types.contains(&"ProjectInitialized"),   "Fixture must contain ProjectInitialized");

    let req_pos = types.iter().position(|&t| t == "ProjectInitRequested").unwrap();
    let ini_pos = types.iter().position(|&t| t == "ProjectInitialized").unwrap();
    assert!(req_pos < ini_pos, "ProjectInitRequested must precede ProjectInitialized");
}

#[test]
fn test_happy_path_list_requested_before_returned() {
    let all = load_fixture("multi_project_happy_path.jsonl");
    let events = mp_events(&all);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    let req_pos = types.iter().position(|&t| t == "ProjectListRequested").unwrap();
    let ret_pos = types.iter().position(|&t| t == "ProjectListReturned").unwrap();
    assert!(req_pos < ret_pos, "ProjectListRequested must precede ProjectListReturned");
}

#[test]
fn test_happy_path_open_requested_before_path_returned() {
    let all = load_fixture("multi_project_happy_path.jsonl");
    let events = mp_events(&all);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    let req_pos = types.iter().position(|&t| t == "ProjectOpenRequested").unwrap();
    let ret_pos = types.iter().position(|&t| t == "ProjectPathReturned").unwrap();
    assert!(req_pos < ret_pos, "ProjectOpenRequested must precede ProjectPathReturned");
}

// ── Correlation ID conformance ────────────────────────────────────────────────

#[test]
fn test_happy_path_init_events_share_correlation_id() {
    let all = load_fixture("multi_project_happy_path.jsonl");
    let events = mp_events(&all);

    let init_events: Vec<&&Value> = events.iter()
        .filter(|e| {
            let t = e["event_type"].as_str().unwrap_or("");
            t == "ProjectInitRequested" || t == "ProjectInitialized"
        })
        .collect();

    assert!(init_events.len() >= 2);
    let cid = init_events[0]["correlation_id"].as_str().unwrap();
    for e in &init_events {
        assert_eq!(e["correlation_id"].as_str().unwrap(), cid,
            "Init events must share the same correlation_id");
    }
}

#[test]
fn test_happy_path_list_events_share_correlation_id() {
    let all = load_fixture("multi_project_happy_path.jsonl");
    let events = mp_events(&all);

    let list_events: Vec<&&Value> = events.iter()
        .filter(|e| {
            let t = e["event_type"].as_str().unwrap_or("");
            t == "ProjectListRequested" || t == "ProjectListReturned"
        })
        .collect();

    assert!(list_events.len() >= 2);
    let cid = list_events[0]["correlation_id"].as_str().unwrap();
    for e in &list_events {
        assert_eq!(e["correlation_id"].as_str().unwrap(), cid,
            "List events must share the same correlation_id");
    }
}

#[test]
fn test_happy_path_different_commands_have_different_correlation_ids() {
    let all = load_fixture("multi_project_happy_path.jsonl");
    let events = mp_events(&all);

    let init_cid = events.iter()
        .find(|e| e["event_type"] == "ProjectInitRequested")
        .map(|e| e["correlation_id"].as_str().unwrap())
        .unwrap();
    let list_cid = events.iter()
        .find(|e| e["event_type"] == "ProjectListRequested")
        .map(|e| e["correlation_id"].as_str().unwrap())
        .unwrap();
    let open_cid = events.iter()
        .find(|e| e["event_type"] == "ProjectOpenRequested")
        .map(|e| e["correlation_id"].as_str().unwrap())
        .unwrap();

    assert_ne!(init_cid, list_cid, "init and list must have different correlation_ids");
    assert_ne!(list_cid, open_cid, "list and open must have different correlation_ids");
    assert_ne!(init_cid, open_cid, "init and open must have different correlation_ids");
}

// ── Payload shape conformance ─────────────────────────────────────────────────

#[test]
fn test_happy_path_project_init_requested_payload() {
    let all = load_fixture("multi_project_happy_path.jsonl");
    let events = mp_events(&all);
    let event = events.iter().find(|e| e["event_type"] == "ProjectInitRequested").unwrap();
    let p = &event["payload"];
    assert!(p["project_name"].as_str().is_some(), "project_name must be a string");
    assert!(p["project_dir"].as_str().is_some(),  "project_dir must be a string");
}

#[test]
fn test_happy_path_project_initialized_payload() {
    let all = load_fixture("multi_project_happy_path.jsonl");
    let events = mp_events(&all);
    let event = events.iter().find(|e| e["event_type"] == "ProjectInitialized").unwrap();
    let p = &event["payload"];
    assert!(p["project_name"].as_str().is_some(), "project_name must be a string");
    assert!(p["project_dir"].as_str().is_some(),  "project_dir must be a string");
    assert!(!p["project_name"].as_str().unwrap().is_empty(), "project_name must not be empty");
    assert!(!p["project_dir"].as_str().unwrap().is_empty(),  "project_dir must not be empty");
}

#[test]
fn test_happy_path_project_list_returned_payload() {
    let all = load_fixture("multi_project_happy_path.jsonl");
    let events = mp_events(&all);
    let event = events.iter().find(|e| e["event_type"] == "ProjectListReturned").unwrap();
    let p = &event["payload"];
    assert!(p["project_count"].as_u64().is_some(), "project_count must be a u64");
    assert!(p["projects"].as_array().is_some(),     "projects must be an array");
    assert!(p["project_count"].as_u64().unwrap() > 0, "project_count must be > 0 in happy path");
    let arr_len = p["projects"].as_array().unwrap().len() as u64;
    assert_eq!(p["project_count"].as_u64().unwrap(), arr_len,
        "project_count must equal the projects array length");
}

#[test]
fn test_happy_path_project_path_returned_payload() {
    let all = load_fixture("multi_project_happy_path.jsonl");
    let events = mp_events(&all);
    let event = events.iter().find(|e| e["event_type"] == "ProjectPathReturned").unwrap();
    let p = &event["payload"];
    assert!(p["project_name"].as_str().is_some(), "project_name must be a string");
    assert!(p["project_dir"].as_str().is_some(),  "project_dir must be a string");
}

#[test]
fn test_happy_path_init_and_open_project_name_consistent() {
    let all = load_fixture("multi_project_happy_path.jsonl");
    let events = mp_events(&all);

    let init_name = events.iter()
        .find(|e| e["event_type"] == "ProjectInitialized")
        .map(|e| e["payload"]["project_name"].as_str().unwrap())
        .unwrap();
    let open_name = events.iter()
        .find(|e| e["event_type"] == "ProjectPathReturned")
        .map(|e| e["payload"]["project_name"].as_str().unwrap())
        .unwrap();

    assert_eq!(init_name, open_name,
        "project_name in ProjectInitialized and ProjectPathReturned must match");
}

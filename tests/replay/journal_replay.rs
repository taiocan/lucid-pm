//! Replay verification tests for journal.
//!
//! Loads JSONL event fixtures and verifies that journal events conform to
//! the approved event schema (events/journal_schema.md): required fields,
//! valid event types, correct payload shapes, and valid event sequences.

use project_schema::test_support::load_fixture;
use serde_json::Value;

fn jn_events(all: &[Value]) -> Vec<&Value> {
    all.iter()
        .filter(|e| e["source_module"].as_str() == Some("journal"))
        .collect()
}

const VALID_EVENT_TYPES: &[&str] = &[
    "JournalEntryCreated",
    "JournalListRequested",
    "JournalListReturned",
    "JournalOpenRequested",
    "JournalEntryOpened",
    "JournalOpenFailedEntryNotFound",
];

// ── Schema conformance ────────────────────────────────────────────────────────

#[test]
fn test_happy_path_all_events_have_required_base_fields() {
    let all = load_fixture("journal_happy_path.jsonl");
    let events = jn_events(&all);
    assert!(!events.is_empty(), "Fixture must contain journal events");

    for event in &events {
        let t = event["event_type"].as_str().unwrap_or("unknown");
        assert!(event["event_id"].as_str().is_some(),       "{}: event_id must be a string", t);
        assert!(event["event_type"].as_str().is_some(),     "{}: event_type must be a string", t);
        assert!(event["timestamp"].as_u64().is_some(),      "{}: timestamp must be a u64", t);
        assert!(event["correlation_id"].as_str().is_some(), "{}: correlation_id must be present", t);
        assert!(event["source_module"].as_str().is_some(),  "{}: source_module must be a string", t);
        assert!(event["payload"].is_object(),               "{}: payload must be an object", t);
        assert_eq!(event["source_module"].as_str().unwrap(), "journal",
            "{}: source_module must be 'journal'", t);
        assert!(event["timestamp"].as_u64().unwrap() > 0,
            "{}: timestamp must be positive", t);
    }
}

#[test]
fn test_happy_path_event_types_are_schema_members() {
    let all = load_fixture("journal_happy_path.jsonl");
    let events = jn_events(&all);
    for event in &events {
        let t = event["event_type"].as_str().unwrap();
        assert!(
            VALID_EVENT_TYPES.contains(&t),
            "Event type '{}' is not in the approved journal schema", t
        );
    }
}

#[test]
fn test_happy_path_no_failure_events() {
    let all = load_fixture("journal_happy_path.jsonl");
    let events = jn_events(&all);
    for event in &events {
        let t = event["event_type"].as_str().unwrap();
        assert!(
            !t.contains("Failed"),
            "Happy path fixture must not contain failure event '{}'", t
        );
    }
}

// ── Sequence conformance ──────────────────────────────────────────────────────

#[test]
fn test_happy_path_list_requested_before_returned() {
    let all = load_fixture("journal_happy_path.jsonl");
    let events = jn_events(&all);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"JournalListRequested"), "Fixture must contain JournalListRequested");
    assert!(types.contains(&"JournalListReturned"),  "Fixture must contain JournalListReturned");

    let req_pos = types.iter().position(|&t| t == "JournalListRequested").unwrap();
    let ret_pos = types.iter().position(|&t| t == "JournalListReturned").unwrap();
    assert!(req_pos < ret_pos, "JournalListRequested must precede JournalListReturned");
}

#[test]
fn test_happy_path_open_requested_before_opened() {
    let all = load_fixture("journal_happy_path.jsonl");
    let events = jn_events(&all);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"JournalOpenRequested"), "Fixture must contain JournalOpenRequested");
    assert!(types.contains(&"JournalEntryOpened"),   "Fixture must contain JournalEntryOpened");

    let req_pos = types.iter().position(|&t| t == "JournalOpenRequested").unwrap();
    let opn_pos = types.iter().position(|&t| t == "JournalEntryOpened").unwrap();
    assert!(req_pos < opn_pos, "JournalOpenRequested must precede JournalEntryOpened");
}

#[test]
fn test_happy_path_list_events_share_correlation_id() {
    let all = load_fixture("journal_happy_path.jsonl");
    let events = jn_events(&all);

    let req = events.iter().find(|e| e["event_type"] == "JournalListRequested").unwrap();
    let ret = events.iter().find(|e| e["event_type"] == "JournalListReturned").unwrap();

    assert_eq!(
        req["correlation_id"].as_str().unwrap(),
        ret["correlation_id"].as_str().unwrap(),
        "JournalListRequested and JournalListReturned must share correlation_id"
    );
}

#[test]
fn test_happy_path_open_events_share_correlation_id() {
    let all = load_fixture("journal_happy_path.jsonl");
    let events = jn_events(&all);

    let req = events.iter().find(|e| e["event_type"] == "JournalOpenRequested").unwrap();
    let opn = events.iter().find(|e| e["event_type"] == "JournalEntryOpened").unwrap();

    assert_eq!(
        req["correlation_id"].as_str().unwrap(),
        opn["correlation_id"].as_str().unwrap(),
        "JournalOpenRequested and JournalEntryOpened must share correlation_id"
    );
}

// ── Payload shape conformance ─────────────────────────────────────────────────

#[test]
fn test_happy_path_entry_created_payload_shape() {
    let all = load_fixture("journal_happy_path.jsonl");
    let events = jn_events(&all);
    let created = events.iter().find(|e| e["event_type"] == "JournalEntryCreated").unwrap();
    let p = &created["payload"];

    assert!(p["filename"].as_str().is_some(),   "filename must be a string");
    assert!(p["title"].as_str().is_some(),       "title must be a string");
    assert!(p["created_at"].as_str().is_some(),  "created_at must be a string");

    let created_at = p["created_at"].as_str().unwrap();
    assert_eq!(created_at.len(), 10, "created_at must be YYYY-MM-DD (10 chars)");
    assert!(p["filename"].as_str().unwrap().starts_with(created_at),
        "filename must start with created_at date");
}

#[test]
fn test_happy_path_list_returned_payload_shape() {
    let all = load_fixture("journal_happy_path.jsonl");
    let events = jn_events(&all);
    let returned = events.iter().find(|e| e["event_type"] == "JournalListReturned").unwrap();
    let p = &returned["payload"];

    assert!(p["entry_count"].as_u64().is_some(), "entry_count must be a u64");
    assert!(p["entries"].is_array(),             "entries must be an array");
    assert_eq!(
        p["entry_count"].as_u64().unwrap() as usize,
        p["entries"].as_array().unwrap().len(),
        "entry_count must equal entries array length"
    );
    assert!(p["entry_count"].as_u64().unwrap() > 0,
        "happy path fixture must have at least one entry");
}

#[test]
fn test_happy_path_list_entry_objects_have_required_fields() {
    let all = load_fixture("journal_happy_path.jsonl");
    let events = jn_events(&all);
    let returned = events.iter().find(|e| e["event_type"] == "JournalListReturned").unwrap();
    let entries = returned["payload"]["entries"].as_array().unwrap();

    for entry in entries {
        assert!(entry["filename"].as_str().is_some(),   "entry must have filename");
        assert!(entry["title"].as_str().is_some(),      "entry must have title");
        assert!(entry["created_at"].as_str().is_some(), "entry must have created_at");
    }
}

#[test]
fn test_happy_path_entry_opened_payload_shape() {
    let all = load_fixture("journal_happy_path.jsonl");
    let events = jn_events(&all);
    let opened = events.iter().find(|e| e["event_type"] == "JournalEntryOpened").unwrap();
    let p = &opened["payload"];

    assert!(p["filename"].as_str().is_some(), "filename must be a string");
    assert!(p["path"].as_str().is_some(),     "path must be a string");
    assert!(!p["path"].as_str().unwrap().is_empty(), "path must not be empty");
}

#[test]
fn test_happy_path_open_requested_filename_matches_opened_filename() {
    let all = load_fixture("journal_happy_path.jsonl");
    let events = jn_events(&all);

    let req = events.iter().find(|e| e["event_type"] == "JournalOpenRequested").unwrap();
    let opn = events.iter().find(|e| e["event_type"] == "JournalEntryOpened").unwrap();

    assert_eq!(
        req["payload"]["filename"].as_str().unwrap(),
        opn["payload"]["filename"].as_str().unwrap(),
        "JournalOpenRequested.filename must match JournalEntryOpened.filename"
    );
}

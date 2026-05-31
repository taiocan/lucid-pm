//! Behavioral tests for project_state.
//!
//! Tests verify observable outcomes: events emitted, payload shapes, ordering,
//! and state changes. No internal logic is tested.
//! All assertions reference event names from events/project_state_schema.md exactly.

use serde_json::{json, Value};
use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};
use tempfile::TempDir;

fn binary_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_project_state"))
}

fn setup_temp_dir() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("events")).unwrap();
    dir
}

/// Write pm_structuring events for a session into the shared events file.
/// This simulates a prior pm_structuring run that produced confirmed items.
fn seed_pm_events(dir: &TempDir, session_id: &str, items: &[(&str, &str, &str)]) {
    // items: (item_id, item_type, description)
    let items_json: Vec<Value> = items.iter().map(|(id, typ, desc)| json!({
        "item_id": id,
        "item_type": typ,
        "description": desc,
        "uncertain": false,
        "uncertainty_reason": null
    })).collect();

    let accepted_ids: Vec<&str> = items.iter().map(|(id, _, _)| *id).collect();

    let items_extracted = json!({
        "event_id": format!("seed-extracted-{}", &session_id[..8]),
        "event_type": "ItemsExtracted",
        "timestamp": 1748000001000u64,
        "correlation_id": session_id,
        "source_module": "pm_structuring",
        "payload": {
            "items": items_json,
            "item_count": items.len(),
            "uncertain_count": 0
        }
    });

    let extraction_confirmed = json!({
        "event_id": format!("seed-confirmed-{}", &session_id[..8]),
        "event_type": "ExtractionConfirmed",
        "timestamp": 1748000002000u64,
        "correlation_id": session_id,
        "source_module": "pm_structuring",
        "payload": {
            "accepted_item_ids": accepted_ids,
            "accepted_count": items.len()
        }
    });

    let path = dir.path().join("events/runtime_events.jsonl");
    let mut file = fs::OpenOptions::new().create(true).append(true).open(&path).unwrap();
    writeln!(file, "{}", items_extracted).unwrap();
    writeln!(file, "{}", extraction_confirmed).unwrap();
}

fn run_binary(dir: &TempDir, args: &[&str]) -> std::process::Output {
    Command::new(binary_path())
        .current_dir(dir.path())
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("Failed to run binary")
}

/// Read only project_state events from the shared events file.
fn read_ps_events(dir: &TempDir) -> Vec<Value> {
    let path = dir.path().join("events/runtime_events.jsonl");
    if !path.exists() { return vec![]; }
    fs::read_to_string(path).unwrap()
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .filter(|e| e["source_module"].as_str() == Some("project_state"))
        .collect()
}

const SESSION_A: &str = "a4ca3a7e-61eb-4f36-b59e-f3abd166e351";
const SESSION_B: &str = "b5db4b8f-72fc-4g47-c60f-g4bce277f462";

// ── Happy Path 1: Incorporate ─────────────────────────────────────────────────

#[test]
fn test_incorporate_emits_incorporation_requested_then_items_incorporated() {
    let dir = setup_temp_dir();
    seed_pm_events(&dir, SESSION_A, &[
        ("item-001", "task", "Deploy the release by end of week"),
        ("item-002", "stakeholder", "Sarah is the release manager"),
    ]);

    run_binary(&dir, &["incorporate", SESSION_A]);

    let events = read_ps_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"IncorporationRequested"), "IncorporationRequested must be emitted");
    assert!(types.contains(&"ItemsIncorporated"),      "ItemsIncorporated must be emitted");
    assert!(!types.contains(&"IncorporationFailedDuplicate"), "IncorporationFailedDuplicate must NOT be emitted");

    let req_pos = types.iter().position(|&t| t == "IncorporationRequested").unwrap();
    let inc_pos = types.iter().position(|&t| t == "ItemsIncorporated").unwrap();
    assert!(req_pos < inc_pos, "IncorporationRequested must precede ItemsIncorporated");
}

#[test]
fn test_incorporate_payload_reflects_session_and_counts() {
    let dir = setup_temp_dir();
    seed_pm_events(&dir, SESSION_A, &[
        ("item-001", "task", "Deploy the release"),
        ("item-002", "risk", "Vendor delay risk"),
        ("item-003", "stakeholder", "Alice"),
    ]);

    run_binary(&dir, &["incorporate", SESSION_A]);

    let events = read_ps_events(&dir);
    let incorporated = events.iter()
        .find(|e| e["event_type"] == "ItemsIncorporated")
        .expect("ItemsIncorporated event not found");

    assert_eq!(incorporated["payload"]["session_id"].as_str().unwrap(), SESSION_A);
    assert_eq!(incorporated["payload"]["incorporated_count"].as_u64().unwrap(), 3);
    assert_eq!(incorporated["payload"]["total_record_size"].as_u64().unwrap(), 3);
}

#[test]
fn test_second_incorporate_grows_total_record_size() {
    let dir = setup_temp_dir();
    seed_pm_events(&dir, SESSION_A, &[("item-001", "task", "First task")]);
    seed_pm_events(&dir, SESSION_B, &[("item-002", "milestone", "Q2 delivery"), ("item-003", "risk", "Budget cut")]);

    run_binary(&dir, &["incorporate", SESSION_A]);
    run_binary(&dir, &["incorporate", SESSION_B]);

    let events = read_ps_events(&dir);
    let incorporated_events: Vec<&Value> = events.iter()
        .filter(|e| e["event_type"] == "ItemsIncorporated")
        .collect();

    assert_eq!(incorporated_events.len(), 2);
    let second = incorporated_events[1];
    assert_eq!(second["payload"]["incorporated_count"].as_u64().unwrap(), 2);
    assert_eq!(second["payload"]["total_record_size"].as_u64().unwrap(), 3);
}

// ── Happy Path 2: View ────────────────────────────────────────────────────────

#[test]
fn test_view_after_incorporate_emits_record_returned() {
    let dir = setup_temp_dir();
    seed_pm_events(&dir, SESSION_A, &[
        ("item-001", "task", "Deploy the release"),
        ("item-002", "stakeholder", "Sarah"),
    ]);
    run_binary(&dir, &["incorporate", SESSION_A]);
    run_binary(&dir, &["view"]);

    let events = read_ps_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"RecordQueried"),  "RecordQueried must be emitted");
    assert!(types.contains(&"RecordReturned"), "RecordReturned must be emitted");
    assert!(!types.contains(&"RecordQueryFailedEmpty"), "RecordQueryFailedEmpty must NOT be emitted");
}

#[test]
fn test_view_record_returned_payload_shape() {
    let dir = setup_temp_dir();
    seed_pm_events(&dir, SESSION_A, &[
        ("item-001", "task", "Deploy the release"),
        ("item-002", "stakeholder", "Sarah"),
    ]);
    run_binary(&dir, &["incorporate", SESSION_A]);
    run_binary(&dir, &["view"]);

    let events = read_ps_events(&dir);
    let returned = events.iter()
        .find(|e| e["event_type"] == "RecordReturned")
        .expect("RecordReturned event not found");

    let items = returned["payload"]["items"].as_array().expect("items must be an array");
    assert_eq!(items.len() as u64, returned["payload"]["total_count"].as_u64().unwrap());
    assert_eq!(returned["payload"]["session_count"].as_u64().unwrap(), 1);

    for item in items {
        assert!(item.get("item_id").is_some(),    "item must have item_id");
        assert!(item.get("item_type").is_some(),  "item must have item_type");
        assert!(item.get("description").is_some(),"item must have description");
        assert!(item.get("uncertain").is_some(),  "item must have uncertain");
        assert!(item.get("session_id").is_some(), "item must have session_id");
        assert_eq!(item["session_id"].as_str().unwrap(), SESSION_A);
    }
}

#[test]
fn test_view_is_read_only_no_record_mutation() {
    let dir = setup_temp_dir();
    seed_pm_events(&dir, SESSION_A, &[("item-001", "task", "Deploy the release")]);
    run_binary(&dir, &["incorporate", SESSION_A]);

    // View twice — total_count must remain the same
    run_binary(&dir, &["view"]);
    run_binary(&dir, &["view"]);

    let events = read_ps_events(&dir);
    let returned_events: Vec<&Value> = events.iter()
        .filter(|e| e["event_type"] == "RecordReturned")
        .collect();

    assert_eq!(returned_events.len(), 2);
    assert_eq!(
        returned_events[0]["payload"]["total_count"],
        returned_events[1]["payload"]["total_count"],
        "total_count must be identical across repeated views"
    );
}

// ── Failure Path 1: EmptyRecord ───────────────────────────────────────────────

#[test]
fn test_view_empty_record_emits_record_query_failed_empty() {
    let dir = setup_temp_dir();
    run_binary(&dir, &["view"]);

    let events = read_ps_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"RecordQueried"),           "RecordQueried must be emitted");
    assert!(types.contains(&"RecordQueryFailedEmpty"),  "RecordQueryFailedEmpty must be emitted");
    assert!(!types.contains(&"RecordReturned"),         "RecordReturned must NOT be emitted on empty record");
}

#[test]
fn test_empty_record_failure_reason() {
    let dir = setup_temp_dir();
    run_binary(&dir, &["view"]);

    let events = read_ps_events(&dir);
    let failure = events.iter()
        .find(|e| e["event_type"] == "RecordQueryFailedEmpty")
        .expect("RecordQueryFailedEmpty not found");

    assert_eq!(failure["payload"]["failure_reason"].as_str().unwrap(), "record_empty");
}

#[test]
fn test_empty_record_record_queried_precedes_failure() {
    let dir = setup_temp_dir();
    run_binary(&dir, &["view"]);

    let events = read_ps_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    let queried_pos = types.iter().position(|&t| t == "RecordQueried").unwrap();
    let failed_pos  = types.iter().position(|&t| t == "RecordQueryFailedEmpty").unwrap();
    assert!(queried_pos < failed_pos, "RecordQueried must precede RecordQueryFailedEmpty");
}

// ── Failure Path 2: SessionAlreadyIncorporated ────────────────────────────────

#[test]
fn test_duplicate_incorporate_emits_incorporation_failed_duplicate() {
    let dir = setup_temp_dir();
    seed_pm_events(&dir, SESSION_A, &[("item-001", "task", "Deploy the release")]);

    run_binary(&dir, &["incorporate", SESSION_A]);
    run_binary(&dir, &["incorporate", SESSION_A]);

    let events = read_ps_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"IncorporationFailedDuplicate"),
        "IncorporationFailedDuplicate must be emitted on second incorporate");
    assert_eq!(
        types.iter().filter(|&&t| t == "ItemsIncorporated").count(), 1,
        "ItemsIncorporated must appear only once"
    );
}

#[test]
fn test_duplicate_incorporate_failure_reason() {
    let dir = setup_temp_dir();
    seed_pm_events(&dir, SESSION_A, &[("item-001", "task", "Deploy the release")]);

    run_binary(&dir, &["incorporate", SESSION_A]);
    run_binary(&dir, &["incorporate", SESSION_A]);

    let events = read_ps_events(&dir);
    let failure = events.iter()
        .find(|e| e["event_type"] == "IncorporationFailedDuplicate")
        .expect("IncorporationFailedDuplicate not found");

    assert_eq!(failure["payload"]["failure_reason"].as_str().unwrap(), "session_already_incorporated");
    assert_eq!(failure["payload"]["session_id"].as_str().unwrap(), SESSION_A);
}

#[test]
fn test_duplicate_incorporate_record_unchanged() {
    let dir = setup_temp_dir();
    seed_pm_events(&dir, SESSION_A, &[("item-001", "task", "Deploy the release")]);

    run_binary(&dir, &["incorporate", SESSION_A]);
    run_binary(&dir, &["incorporate", SESSION_A]); // duplicate — must be rejected
    run_binary(&dir, &["view"]);

    let events = read_ps_events(&dir);
    let returned = events.iter()
        .find(|e| e["event_type"] == "RecordReturned")
        .expect("RecordReturned not found");

    assert_eq!(returned["payload"]["total_count"].as_u64().unwrap(), 1,
        "Record must contain exactly 1 item — duplicate must not add more");
    assert_eq!(returned["payload"]["session_count"].as_u64().unwrap(), 1,
        "Record must contain exactly 1 session after duplicate rejection");
}

// ── Telemetry: required base fields ──────────────────────────────────────────

#[test]
fn test_all_events_have_required_base_fields() {
    let dir = setup_temp_dir();
    run_binary(&dir, &["view"]); // EmptyRecord path — no seeding needed

    let events = read_ps_events(&dir);
    assert!(!events.is_empty());

    for event in &events {
        let t = event["event_type"].as_str().unwrap_or("unknown");
        assert!(!event["event_id"].is_null(),       "{}: event_id must be present", t);
        assert!(!event["event_type"].is_null(),     "{}: event_type must be present", t);
        assert!(!event["timestamp"].is_null(),      "{}: timestamp must be present", t);
        assert!(!event["correlation_id"].is_null(), "{}: correlation_id must be present", t);
        assert!(!event["source_module"].is_null(),  "{}: source_module must be present", t);
        assert!(!event["payload"].is_null(),        "{}: payload must be present", t);
        assert_eq!(event["source_module"].as_str().unwrap(), "project_state",
            "{}: source_module must be 'project_state'", t);
        assert!(event["timestamp"].as_u64().unwrap() > 0, "{}: timestamp must be positive", t);
    }
}

#[test]
fn test_correlation_id_consistent_within_one_invocation() {
    let dir = setup_temp_dir();
    run_binary(&dir, &["view"]);

    let events = read_ps_events(&dir);
    assert!(events.len() >= 2);

    let first_id = events[0]["correlation_id"].as_str().unwrap();
    for event in &events {
        assert_eq!(event["correlation_id"].as_str().unwrap(), first_id,
            "All events from one invocation must share the same correlation_id");
    }
}

#[test]
fn test_separate_invocations_have_different_correlation_ids() {
    let dir = setup_temp_dir();
    run_binary(&dir, &["view"]);
    run_binary(&dir, &["view"]);

    let events = read_ps_events(&dir);
    let ids: Vec<&str> = events.iter()
        .filter(|e| e["event_type"] == "RecordQueried")
        .map(|e| e["correlation_id"].as_str().unwrap())
        .collect();

    assert_eq!(ids.len(), 2);
    assert_ne!(ids[0], ids[1], "Different invocations must produce different correlation_ids");
}

//! Behavioral tests for item_status.
//!
//! Tests verify observable outcomes: events emitted, payload shapes, ordering,
//! and state changes. No internal logic is tested.
//! All assertions reference event names from events/item_status_schema.md exactly.

use serde_json::{json, Value};
use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};
use tempfile::TempDir;

fn binary_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_item_status"))
}

// Minimal default vocabulary matching the installed ~/.lucidpm/default-schema.yaml.
// Makes tests portable without requiring the installed schema to be present.
const DEFAULT_SCHEMA: &str = r#"schemaVersion: 1
statuses:
  todo:
  doing:
  done:
  waiting:
  cancelled:
  pending:
  achieved:
  missed:
  open:
  mitigated:
  accepted:
  closed:
  in_progress:
  resolved:
  active:
  inactive:
pageTypes:
  Task:
    allowedStatuses: [todo, doing, done, waiting, cancelled]
    aliases: [task]
  Milestone:
    allowedStatuses: [pending, achieved, missed]
    aliases: [milestone]
  Risk:
    allowedStatuses: [open, mitigated, accepted, closed]
    aliases: [risk]
  Issue:
    allowedStatuses: [open, in_progress, resolved, closed]
    aliases: [issue]
  Stakeholder:
    allowedStatuses: [active, inactive]
    aliases: [stakeholder]
"#;

fn write_project_schema(dir: &TempDir, yaml: &str) {
    fs::write(dir.path().join("project-schema.yaml"), yaml).unwrap();
}

fn setup_temp_dir() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("events")).unwrap();
    write_project_schema(&dir, DEFAULT_SCHEMA);
    dir
}

/// Read all events (all source_modules) from the events file.
fn read_all_events(dir: &TempDir) -> Vec<Value> {
    let path = dir.path().join("events/runtime_events.jsonl");
    if !path.exists() { return vec![]; }
    fs::read_to_string(path).unwrap()
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .collect()
}

/// Seed pm_structuring + project_state events so item_status can find items.
/// items: (item_id, item_type, description)
fn seed_incorporated_items(dir: &TempDir, session_id: &str, items: &[(&str, &str, &str)]) {
    let items_json: Vec<Value> = items.iter().map(|(id, typ, desc)| json!({
        "item_id": id,
        "item_type": typ,
        "description": desc,
        "uncertain": false,
        "uncertainty_reason": null
    })).collect();

    let accepted_ids: Vec<&str> = items.iter().map(|(id, _, _)| *id).collect();

    let items_extracted = json!({
        "event_id": format!("seed-ext-{}", &session_id[..8]),
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
        "event_id": format!("seed-conf-{}", &session_id[..8]),
        "event_type": "ExtractionConfirmed",
        "timestamp": 1748000002000u64,
        "correlation_id": session_id,
        "source_module": "pm_structuring",
        "payload": {
            "accepted_item_ids": accepted_ids,
            "accepted_count": items.len()
        }
    });

    let items_incorporated = json!({
        "event_id": format!("seed-inc-{}", &session_id[..8]),
        "event_type": "ItemsIncorporated",
        "timestamp": 1748000003000u64,
        "correlation_id": "00000000-0000-0000-0000-000000000001",
        "source_module": "project_state",
        "payload": {
            "session_id": session_id,
            "incorporated_count": items.len(),
            "total_record_size": items.len()
        }
    });

    let path = dir.path().join("events/runtime_events.jsonl");
    let mut file = fs::OpenOptions::new().create(true).append(true).open(&path).unwrap();
    writeln!(file, "{}", items_extracted).unwrap();
    writeln!(file, "{}", extraction_confirmed).unwrap();
    writeln!(file, "{}", items_incorporated).unwrap();
}

/// Seed items with proposed_status and proposed_priority included in ItemsExtracted.
/// items: (item_id, item_type, description, proposed_status, proposed_priority)
fn seed_with_proposed(
    dir: &TempDir,
    session_id: &str,
    items: &[(&str, &str, &str, Option<&str>, Option<&str>)],
) {
    let items_json: Vec<Value> = items.iter().map(|(id, typ, desc, ps, pp)| json!({
        "item_id": id,
        "item_type": typ,
        "description": desc,
        "uncertain": false,
        "uncertainty_reason": null,
        "proposed_status": ps,
        "proposed_priority": pp,
    })).collect();

    let accepted_ids: Vec<&str> = items.iter().map(|(id, _, _, _, _)| *id).collect();

    let items_extracted = json!({
        "event_id": format!("seed-ext-{}", &session_id[..8]),
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
        "event_id": format!("seed-conf-{}", &session_id[..8]),
        "event_type": "ExtractionConfirmed",
        "timestamp": 1748000002000u64,
        "correlation_id": session_id,
        "source_module": "pm_structuring",
        "payload": {
            "accepted_item_ids": accepted_ids,
            "accepted_count": items.len()
        }
    });

    let items_incorporated = json!({
        "event_id": format!("seed-inc-{}", &session_id[..8]),
        "event_type": "ItemsIncorporated",
        "timestamp": 1748000003000u64,
        "correlation_id": "00000000-0000-0000-0000-000000000001",
        "source_module": "project_state",
        "payload": {
            "session_id": session_id,
            "incorporated_count": items.len(),
            "total_record_size": items.len()
        }
    });

    let path = dir.path().join("events/runtime_events.jsonl");
    let mut file = fs::OpenOptions::new().create(true).append(true).open(&path).unwrap();
    writeln!(file, "{}", items_extracted).unwrap();
    writeln!(file, "{}", extraction_confirmed).unwrap();
    writeln!(file, "{}", items_incorporated).unwrap();
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

/// Read only item_status events from the shared events file.
fn read_is_events(dir: &TempDir) -> Vec<Value> {
    let path = dir.path().join("events/runtime_events.jsonl");
    if !path.exists() { return vec![]; }
    fs::read_to_string(path).unwrap()
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .filter(|e| e["source_module"].as_str() == Some("item_status"))
        .collect()
}

const SESSION_A:    &str = "a4ca3a7e-61eb-4f36-b59e-f3abd166e351";
const ITEM_TASK:    &str = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeee01";
const ITEM_RISK:    &str = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeee02";
const ITEM_MILESTONE: &str = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeee03";
const UNKNOWN_ITEM: &str = "ffffffff-ffff-ffff-ffff-ffffffffffff";

// ── Happy Path 1: Set Status ──────────────────────────────────────────────────

#[test]
fn test_set_status_emits_requested_then_updated() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_TASK, "task", "Deploy by Friday")]);

    run_binary(&dir, &["set-status", ITEM_TASK, "doing"]);

    let events = read_is_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"StatusUpdateRequested"), "StatusUpdateRequested must be emitted");
    assert!(types.contains(&"ItemStatusUpdated"),     "ItemStatusUpdated must be emitted");
    assert!(!types.contains(&"StatusUpdateFailedItemNotFound"),  "must NOT emit ItemNotFound failure");
    assert!(!types.contains(&"StatusUpdateFailedInvalidStatus"), "must NOT emit InvalidStatus failure");

    let req_pos = types.iter().position(|&t| t == "StatusUpdateRequested").unwrap();
    let upd_pos = types.iter().position(|&t| t == "ItemStatusUpdated").unwrap();
    assert!(req_pos < upd_pos, "StatusUpdateRequested must precede ItemStatusUpdated");
}

#[test]
fn test_set_status_payload_shape() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_TASK, "task", "Deploy by Friday")]);

    run_binary(&dir, &["set-status", ITEM_TASK, "doing"]);

    let events = read_is_events(&dir);
    let updated = events.iter()
        .find(|e| e["event_type"] == "ItemStatusUpdated")
        .expect("ItemStatusUpdated not found");

    assert_eq!(updated["payload"]["item_id"].as_str().unwrap(), ITEM_TASK);
    assert_eq!(updated["payload"]["item_type"].as_str().unwrap(), "task");
    assert_eq!(updated["payload"]["new_status"].as_str().unwrap(), "doing");
    assert!(updated["payload"].get("previous_status").is_some(), "previous_status field must be present");
}

#[test]
fn test_set_status_previous_status_null_on_first_set() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_TASK, "task", "Deploy by Friday")]);

    run_binary(&dir, &["set-status", ITEM_TASK, "todo"]);

    let events = read_is_events(&dir);
    let updated = events.iter()
        .find(|e| e["event_type"] == "ItemStatusUpdated")
        .expect("ItemStatusUpdated not found");

    assert!(updated["payload"]["previous_status"].is_null(),
        "previous_status must be null on first status set");
}

#[test]
fn test_set_status_previous_status_reflects_prior_value() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_TASK, "task", "Deploy by Friday")]);

    run_binary(&dir, &["set-status", ITEM_TASK, "todo"]);
    run_binary(&dir, &["set-status", ITEM_TASK, "doing"]);

    let events = read_is_events(&dir);
    let updated_events: Vec<&Value> = events.iter()
        .filter(|e| e["event_type"] == "ItemStatusUpdated")
        .collect();

    assert_eq!(updated_events.len(), 2);
    assert_eq!(updated_events[1]["payload"]["previous_status"].as_str().unwrap(), "todo",
        "previous_status on second update must equal first update's new_status");
}

#[test]
fn test_set_status_does_not_affect_other_items() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[
        (ITEM_TASK, "task", "Deploy by Friday"),
        (ITEM_RISK, "risk", "Vendor delay"),
    ]);

    run_binary(&dir, &["set-status", ITEM_TASK, "doing"]);

    let events = read_is_events(&dir);
    let updated_events: Vec<&Value> = events.iter()
        .filter(|e| e["event_type"] == "ItemStatusUpdated")
        .collect();

    assert_eq!(updated_events.len(), 1, "Only one ItemStatusUpdated must be emitted");
    assert_eq!(updated_events[0]["payload"]["item_id"].as_str().unwrap(), ITEM_TASK);
}

// ── Happy Path 2: Set Priority ────────────────────────────────────────────────

#[test]
fn test_set_priority_emits_requested_then_updated() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_RISK, "risk", "Vendor delay")]);

    run_binary(&dir, &["set-priority", ITEM_RISK, "high"]);

    let events = read_is_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"PriorityUpdateRequested"), "PriorityUpdateRequested must be emitted");
    assert!(types.contains(&"ItemPriorityUpdated"),     "ItemPriorityUpdated must be emitted");
    assert!(!types.contains(&"PriorityUpdateFailedItemNotFound"), "must NOT emit ItemNotFound failure");
    assert!(!types.contains(&"PriorityUpdateFailedInvalidValue"), "must NOT emit InvalidValue failure");

    let req_pos = types.iter().position(|&t| t == "PriorityUpdateRequested").unwrap();
    let upd_pos = types.iter().position(|&t| t == "ItemPriorityUpdated").unwrap();
    assert!(req_pos < upd_pos, "PriorityUpdateRequested must precede ItemPriorityUpdated");
}

#[test]
fn test_set_priority_payload_shape() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_RISK, "risk", "Vendor delay")]);

    run_binary(&dir, &["set-priority", ITEM_RISK, "high"]);

    let events = read_is_events(&dir);
    let updated = events.iter()
        .find(|e| e["event_type"] == "ItemPriorityUpdated")
        .expect("ItemPriorityUpdated not found");

    assert_eq!(updated["payload"]["item_id"].as_str().unwrap(), ITEM_RISK);
    assert_eq!(updated["payload"]["new_priority"].as_str().unwrap(), "high");
    assert!(updated["payload"].get("previous_priority").is_some(), "previous_priority field must be present");
    assert!(updated["payload"]["previous_priority"].is_null(),
        "previous_priority must be null on first priority set");
}

#[test]
fn test_set_priority_does_not_emit_status_events() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_RISK, "risk", "Vendor delay")]);

    run_binary(&dir, &["set-priority", ITEM_RISK, "medium"]);

    let events = read_is_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(!types.contains(&"ItemStatusUpdated"),     "set-priority must not emit ItemStatusUpdated");
    assert!(!types.contains(&"StatusUpdateRequested"), "set-priority must not emit StatusUpdateRequested");
}

// ── Happy Path 3: Get ─────────────────────────────────────────────────────────

#[test]
fn test_get_emits_queried_then_returned() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_TASK, "task", "Deploy by Friday")]);

    run_binary(&dir, &["get", ITEM_TASK]);

    let events = read_is_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"ItemStatusQueried"),  "ItemStatusQueried must be emitted");
    assert!(types.contains(&"ItemStatusReturned"), "ItemStatusReturned must be emitted");
    assert!(!types.contains(&"ItemStatusQueryFailedItemNotFound"), "must NOT emit failure");

    let q_pos = types.iter().position(|&t| t == "ItemStatusQueried").unwrap();
    let r_pos = types.iter().position(|&t| t == "ItemStatusReturned").unwrap();
    assert!(q_pos < r_pos, "ItemStatusQueried must precede ItemStatusReturned");
}

#[test]
fn test_get_returns_null_status_and_priority_when_never_set() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_TASK, "task", "Deploy by Friday")]);

    run_binary(&dir, &["get", ITEM_TASK]);

    let events = read_is_events(&dir);
    let returned = events.iter()
        .find(|e| e["event_type"] == "ItemStatusReturned")
        .expect("ItemStatusReturned not found");

    assert_eq!(returned["payload"]["item_id"].as_str().unwrap(), ITEM_TASK);
    assert_eq!(returned["payload"]["item_type"].as_str().unwrap(), "task");
    assert!(returned["payload"]["current_status"].is_null(),
        "current_status must be null when never set");
    assert!(returned["payload"]["current_priority"].is_null(),
        "current_priority must be null when never set");
}

#[test]
fn test_get_returns_current_status_and_priority_after_set() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_TASK, "task", "Deploy by Friday")]);

    run_binary(&dir, &["set-status",   ITEM_TASK, "doing"]);
    run_binary(&dir, &["set-priority", ITEM_TASK, "high"]);
    run_binary(&dir, &["get",          ITEM_TASK]);

    let events = read_is_events(&dir);
    let returned = events.iter()
        .find(|e| e["event_type"] == "ItemStatusReturned")
        .expect("ItemStatusReturned not found");

    assert_eq!(returned["payload"]["current_status"].as_str().unwrap(), "doing");
    assert_eq!(returned["payload"]["current_priority"].as_str().unwrap(), "high");
}

#[test]
fn test_get_reflects_most_recent_status() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_TASK, "task", "Deploy by Friday")]);

    run_binary(&dir, &["set-status", ITEM_TASK, "todo"]);
    run_binary(&dir, &["set-status", ITEM_TASK, "doing"]);
    run_binary(&dir, &["set-status", ITEM_TASK, "done"]);
    run_binary(&dir, &["get",        ITEM_TASK]);

    let events = read_is_events(&dir);
    let returned = events.iter()
        .find(|e| e["event_type"] == "ItemStatusReturned")
        .expect("ItemStatusReturned not found");

    assert_eq!(returned["payload"]["current_status"].as_str().unwrap(), "done",
        "get must return the most recently set status");
}

#[test]
fn test_get_is_read_only() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_TASK, "task", "Deploy by Friday")]);
    run_binary(&dir, &["set-status", ITEM_TASK, "todo"]);

    run_binary(&dir, &["get", ITEM_TASK]);
    run_binary(&dir, &["get", ITEM_TASK]);

    let events = read_is_events(&dir);
    let update_count = events.iter()
        .filter(|e| matches!(
            e["event_type"].as_str(),
            Some("ItemStatusUpdated") | Some("ItemPriorityUpdated")
        ))
        .count();

    assert_eq!(update_count, 1,
        "get must produce no update events; only the original set-status counts");
}

// ── Failure Path 1: ItemNotFound ──────────────────────────────────────────────

#[test]
fn test_set_status_item_not_found_emits_failure() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_TASK, "task", "Deploy by Friday")]);

    run_binary(&dir, &["set-status", UNKNOWN_ITEM, "todo"]);

    let events = read_is_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"StatusUpdateRequested"),          "StatusUpdateRequested must be emitted");
    assert!(types.contains(&"StatusUpdateFailedItemNotFound"), "StatusUpdateFailedItemNotFound must be emitted");
    assert!(!types.contains(&"ItemStatusUpdated"),              "ItemStatusUpdated must NOT be emitted");
}

#[test]
fn test_set_priority_item_not_found_emits_failure() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_TASK, "task", "Deploy by Friday")]);

    run_binary(&dir, &["set-priority", UNKNOWN_ITEM, "high"]);

    let events = read_is_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"PriorityUpdateRequested"),           "PriorityUpdateRequested must be emitted");
    assert!(types.contains(&"PriorityUpdateFailedItemNotFound"),  "PriorityUpdateFailedItemNotFound must be emitted");
    assert!(!types.contains(&"ItemPriorityUpdated"),               "ItemPriorityUpdated must NOT be emitted");
}

#[test]
fn test_get_item_not_found_emits_failure() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_TASK, "task", "Deploy by Friday")]);

    run_binary(&dir, &["get", UNKNOWN_ITEM]);

    let events = read_is_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"ItemStatusQueried"),                 "ItemStatusQueried must be emitted");
    assert!(types.contains(&"ItemStatusQueryFailedItemNotFound"), "ItemStatusQueryFailedItemNotFound must be emitted");
    assert!(!types.contains(&"ItemStatusReturned"),                "ItemStatusReturned must NOT be emitted");
}

#[test]
fn test_item_not_found_failure_reason_and_item_id() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_TASK, "task", "Deploy by Friday")]);

    run_binary(&dir, &["set-status", UNKNOWN_ITEM, "todo"]);

    let events = read_is_events(&dir);
    let failure = events.iter()
        .find(|e| e["event_type"] == "StatusUpdateFailedItemNotFound")
        .expect("StatusUpdateFailedItemNotFound not found");

    assert_eq!(failure["payload"]["failure_reason"].as_str().unwrap(), "item_not_found");
    assert_eq!(failure["payload"]["item_id"].as_str().unwrap(), UNKNOWN_ITEM);
}

// ── Failure Path 2: InvalidStatusForType ──────────────────────────────────────

#[test]
fn test_invalid_status_for_type_emits_failure() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_MILESTONE, "milestone", "Q2 launch")]);

    // "doing" is valid for task but NOT for milestone
    run_binary(&dir, &["set-status", ITEM_MILESTONE, "doing"]);

    let events = read_is_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"StatusUpdateRequested"),           "StatusUpdateRequested must be emitted");
    assert!(types.contains(&"StatusUpdateFailedInvalidStatus"), "StatusUpdateFailedInvalidStatus must be emitted");
    assert!(!types.contains(&"ItemStatusUpdated"),               "ItemStatusUpdated must NOT be emitted");
}

#[test]
fn test_invalid_status_failure_payload() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_MILESTONE, "milestone", "Q2 launch")]);

    run_binary(&dir, &["set-status", ITEM_MILESTONE, "doing"]);

    let events = read_is_events(&dir);
    let failure = events.iter()
        .find(|e| e["event_type"] == "StatusUpdateFailedInvalidStatus")
        .expect("StatusUpdateFailedInvalidStatus not found");

    assert_eq!(failure["payload"]["failure_reason"].as_str().unwrap(), "invalid_status_for_type");
    assert_eq!(failure["payload"]["item_id"].as_str().unwrap(), ITEM_MILESTONE);
    assert_eq!(failure["payload"]["item_type"].as_str().unwrap(), "milestone");
    assert_eq!(failure["payload"]["requested_status"].as_str().unwrap(), "doing");
}

#[test]
fn test_invalid_status_does_not_overwrite_existing_status() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_MILESTONE, "milestone", "Q2 launch")]);

    run_binary(&dir, &["set-status", ITEM_MILESTONE, "pending"]);  // valid
    run_binary(&dir, &["set-status", ITEM_MILESTONE, "doing"]);    // invalid — must be rejected

    let events = read_is_events(&dir);
    let updated_events: Vec<&Value> = events.iter()
        .filter(|e| e["event_type"] == "ItemStatusUpdated")
        .collect();

    assert_eq!(updated_events.len(), 1, "Only the valid set must produce ItemStatusUpdated");
    assert_eq!(updated_events[0]["payload"]["new_status"].as_str().unwrap(), "pending");
}

// ── Failure Path 3: InvalidPriorityValue ─────────────────────────────────────

#[test]
fn test_invalid_priority_emits_failure() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_TASK, "task", "Deploy by Friday")]);

    run_binary(&dir, &["set-priority", ITEM_TASK, "critical"]);

    let events = read_is_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"PriorityUpdateRequested"),          "PriorityUpdateRequested must be emitted");
    assert!(types.contains(&"PriorityUpdateFailedInvalidValue"), "PriorityUpdateFailedInvalidValue must be emitted");
    assert!(!types.contains(&"ItemPriorityUpdated"),              "ItemPriorityUpdated must NOT be emitted");
}

#[test]
fn test_invalid_priority_failure_payload() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_TASK, "task", "Deploy by Friday")]);

    run_binary(&dir, &["set-priority", ITEM_TASK, "critical"]);

    let events = read_is_events(&dir);
    let failure = events.iter()
        .find(|e| e["event_type"] == "PriorityUpdateFailedInvalidValue")
        .expect("PriorityUpdateFailedInvalidValue not found");

    assert_eq!(failure["payload"]["failure_reason"].as_str().unwrap(), "invalid_priority_value");
    assert_eq!(failure["payload"]["item_id"].as_str().unwrap(), ITEM_TASK);
    assert_eq!(failure["payload"]["requested_priority"].as_str().unwrap(), "critical");
}

// ── Happy Path 4 & 5: Proposed value fallback (R1) ───────────────────────────

#[test]
fn test_get_returns_proposed_status_as_fallback() {
    let dir = setup_temp_dir();
    seed_with_proposed(&dir, SESSION_A, &[
        (ITEM_TASK, "task", "Deploy by Friday", Some("todo"), None),
    ]);

    run_binary(&dir, &["get", ITEM_TASK]);

    let events = read_is_events(&dir);
    let returned = events.iter()
        .find(|e| e["event_type"] == "ItemStatusReturned")
        .expect("ItemStatusReturned not found");

    assert_eq!(returned["payload"]["current_status"].as_str().unwrap(), "todo",
        "proposed_status must be returned as effective status when no explicit update exists");
}

#[test]
fn test_get_returns_proposed_priority_as_fallback() {
    let dir = setup_temp_dir();
    seed_with_proposed(&dir, SESSION_A, &[
        (ITEM_RISK, "risk", "Vendor delay", None, Some("high")),
    ]);

    run_binary(&dir, &["get", ITEM_RISK]);

    let events = read_is_events(&dir);
    let returned = events.iter()
        .find(|e| e["event_type"] == "ItemStatusReturned")
        .expect("ItemStatusReturned not found");

    assert_eq!(returned["payload"]["current_priority"].as_str().unwrap(), "high",
        "proposed_priority must be returned as effective priority when no explicit update exists");
}

#[test]
fn test_explicit_status_overrides_proposed() {
    let dir = setup_temp_dir();
    seed_with_proposed(&dir, SESSION_A, &[
        (ITEM_TASK, "task", "Deploy by Friday", Some("todo"), None),
    ]);

    run_binary(&dir, &["set-status", ITEM_TASK, "doing"]);
    run_binary(&dir, &["get", ITEM_TASK]);

    let events = read_is_events(&dir);
    let returned = events.iter()
        .find(|e| e["event_type"] == "ItemStatusReturned")
        .expect("ItemStatusReturned not found");

    assert_eq!(returned["payload"]["current_status"].as_str().unwrap(), "doing",
        "explicit set-status must override proposed_status");
}

#[test]
fn test_explicit_priority_overrides_proposed() {
    let dir = setup_temp_dir();
    seed_with_proposed(&dir, SESSION_A, &[
        (ITEM_RISK, "risk", "Vendor delay", None, Some("high")),
    ]);

    run_binary(&dir, &["set-priority", ITEM_RISK, "low"]);
    run_binary(&dir, &["get", ITEM_RISK]);

    let events = read_is_events(&dir);
    let returned = events.iter()
        .find(|e| e["event_type"] == "ItemStatusReturned")
        .expect("ItemStatusReturned not found");

    assert_eq!(returned["payload"]["current_priority"].as_str().unwrap(), "low",
        "explicit set-priority must override proposed_priority");
}

#[test]
fn test_null_proposed_returns_null_when_no_explicit() {
    let dir = setup_temp_dir();
    seed_with_proposed(&dir, SESSION_A, &[
        (ITEM_MILESTONE, "milestone", "Q2 launch", None, None),
    ]);

    run_binary(&dir, &["get", ITEM_MILESTONE]);

    let events = read_is_events(&dir);
    let returned = events.iter()
        .find(|e| e["event_type"] == "ItemStatusReturned")
        .expect("ItemStatusReturned not found");

    assert!(returned["payload"]["current_status"].is_null(),
        "current_status must be null when both proposed and explicit are absent");
    assert!(returned["payload"]["current_priority"].is_null(),
        "current_priority must be null when both proposed and explicit are absent");
}

#[test]
fn test_previous_status_null_on_first_explicit_set_regardless_of_proposed() {
    let dir = setup_temp_dir();
    // proposed_status="todo" is present, but previous_status in ItemStatusUpdated
    // must still be null — proposed values don't count as prior explicit state
    seed_with_proposed(&dir, SESSION_A, &[
        (ITEM_TASK, "task", "Deploy by Friday", Some("todo"), None),
    ]);

    run_binary(&dir, &["set-status", ITEM_TASK, "doing"]);

    let events = read_is_events(&dir);
    let updated = events.iter()
        .find(|e| e["event_type"] == "ItemStatusUpdated")
        .expect("ItemStatusUpdated not found");

    assert!(updated["payload"]["previous_status"].is_null(),
        "previous_status must be null on first explicit set — proposed value is not prior state");
}

// ── Telemetry ─────────────────────────────────────────────────────────────────

#[test]
fn test_all_events_have_required_base_fields() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_TASK, "task", "Deploy by Friday")]);
    run_binary(&dir, &["set-status", ITEM_TASK, "todo"]);

    let events = read_is_events(&dir);
    assert!(!events.is_empty());

    for event in &events {
        let t = event["event_type"].as_str().unwrap_or("unknown");
        assert!(!event["event_id"].is_null(),       "{}: event_id must be present", t);
        assert!(!event["event_type"].is_null(),     "{}: event_type must be present", t);
        assert!(!event["timestamp"].is_null(),      "{}: timestamp must be present", t);
        assert!(!event["correlation_id"].is_null(), "{}: correlation_id must be present", t);
        assert!(!event["source_module"].is_null(),  "{}: source_module must be present", t);
        assert!(!event["payload"].is_null(),        "{}: payload must be present", t);
        assert_eq!(event["source_module"].as_str().unwrap(), "item_status",
            "{}: source_module must be 'item_status'", t);
        assert!(event["timestamp"].as_u64().unwrap() > 0,
            "{}: timestamp must be positive", t);
    }
}

#[test]
fn test_correlation_id_consistent_within_one_invocation() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_TASK, "task", "Deploy by Friday")]);
    run_binary(&dir, &["set-status", ITEM_TASK, "todo"]);

    let events = read_is_events(&dir);
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
    seed_incorporated_items(&dir, SESSION_A, &[
        (ITEM_TASK, "task", "Deploy by Friday"),
        (ITEM_RISK, "risk", "Vendor delay"),
    ]);
    run_binary(&dir, &["set-status", ITEM_TASK, "todo"]);
    run_binary(&dir, &["set-status", ITEM_RISK, "open"]);

    let events = read_is_events(&dir);
    let ids: Vec<&str> = events.iter()
        .filter(|e| e["event_type"] == "StatusUpdateRequested")
        .map(|e| e["correlation_id"].as_str().unwrap())
        .collect();

    assert_eq!(ids.len(), 2);
    assert_ne!(ids[0], ids[1],
        "Different invocations must produce different correlation_ids");
}

// ── R5: Schema-driven vocabulary (HP1) ───────────────────────────────────────

#[test]
fn test_schema_vocabulary_governs_set_status_custom_type() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, r#"schemaVersion: 1
statuses:
  draft:
  active:
  delivered:
  todo:
  doing:
  done:
  waiting:
  cancelled:
  pending:
  achieved:
  missed:
  open:
  mitigated:
  accepted:
  closed:
  in_progress:
  resolved:
  inactive:
pageTypes:
  Epic:
    allowedStatuses: [draft, active, delivered]
    aliases: [epic]
"#);
    seed_incorporated_items(&dir, SESSION_A, &[("epic-0001-0000-0000-000000000001", "epic", "Q3 roadmap")]);

    run_binary(&dir, &["set-status", "epic-0001-0000-0000-000000000001", "active"]);

    let events = read_is_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"ItemStatusUpdated"),
        "set-status with a vocabulary-defined status for a custom type must succeed");
    assert!(!types.contains(&"StatusUpdateFailedInvalidStatus"),
        "must NOT emit InvalidStatus failure for a valid vocabulary status");
}

#[test]
fn test_custom_type_with_no_status_vocabulary_rejects_all_statuses() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, r#"schemaVersion: 1
statuses:
  todo:
  doing:
  done:
  waiting:
  cancelled:
  pending:
  achieved:
  missed:
  open:
  mitigated:
  accepted:
  closed:
  in_progress:
  resolved:
  active:
  inactive:
pageTypes:
  Note:
    aliases: [note]
"#);
    // Note has no allowedStatuses — empty status vocabulary is the condition under test.
    seed_incorporated_items(&dir, SESSION_A, &[("note-0001-0000-0000-000000000001", "note", "Architecture notes")]);

    run_binary(&dir, &["set-status", "note-0001-0000-0000-000000000001", "open"]);

    let events = read_is_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"StatusUpdateFailedInvalidStatus"),
        "type with empty allowedStatuses must reject all status values");
    assert!(!types.contains(&"ItemStatusUpdated"),
        "ItemStatusUpdated must NOT be emitted for a type with no status vocabulary");
}

#[test]
fn test_schema_vocabulary_invalid_status_uses_vocabulary_not_hardcoded_table() {
    // "doing" is valid for task per legacy table but we override the schema
    // to exclude it — validation must use the schema, not the hardcoded table.
    let dir = setup_temp_dir();
    write_project_schema(&dir, r#"schemaVersion: 1
statuses:
  open:
  closed:
  todo:
  doing:
  done:
  waiting:
  cancelled:
  pending:
  achieved:
  missed:
  mitigated:
  accepted:
  in_progress:
  resolved:
  active:
  inactive:
pageTypes:
  Task:
    allowedStatuses: [open, closed]
    aliases: [task]
"#);
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_TASK, "task", "Deploy by Friday")]);

    run_binary(&dir, &["set-status", ITEM_TASK, "doing"]);

    let events = read_is_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"StatusUpdateFailedInvalidStatus"),
        "'doing' must be rejected when the active vocabulary does not include it");
    assert!(!types.contains(&"ItemStatusUpdated"),
        "ItemStatusUpdated must NOT be emitted");
}

// ── R5: Schema failure (FP1) ─────────────────────────────────────────────────

#[test]
fn test_schema_invalid_aborts_set_status_before_observational_event() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, "this: is: not: valid: yaml: [[[");
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_TASK, "task", "Deploy by Friday")]);

    run_binary(&dir, &["set-status", ITEM_TASK, "doing"]);

    let events = read_is_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(!types.contains(&"StatusUpdateRequested"),
        "StatusUpdateRequested must NOT be emitted when schema is invalid");
    assert!(!types.contains(&"ItemStatusUpdated"),
        "ItemStatusUpdated must NOT be emitted when schema is invalid");
}

#[test]
fn test_schema_invalid_aborts_set_priority_before_observational_event() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, "this: is: not: valid: yaml: [[[");
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_TASK, "task", "Deploy by Friday")]);

    run_binary(&dir, &["set-priority", ITEM_TASK, "high"]);

    let events = read_is_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(!types.contains(&"PriorityUpdateRequested"),
        "PriorityUpdateRequested must NOT be emitted when schema is invalid");
    assert!(!types.contains(&"ItemPriorityUpdated"),
        "ItemPriorityUpdated must NOT be emitted when schema is invalid");
}

#[test]
fn test_schema_invalid_aborts_get_before_observational_event() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, "this: is: not: valid: yaml: [[[");
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_TASK, "task", "Deploy by Friday")]);

    run_binary(&dir, &["get", ITEM_TASK]);

    let events = read_is_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(!types.contains(&"ItemStatusQueried"),
        "ItemStatusQueried must NOT be emitted when schema is invalid");
    assert!(!types.contains(&"ItemStatusReturned"),
        "ItemStatusReturned must NOT be emitted when schema is invalid");
}

#[test]
fn test_schema_invalid_emits_cross_module_failure_event() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, "this: is: not: valid: yaml: [[[");
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_TASK, "task", "Deploy by Friday")]);

    run_binary(&dir, &["set-status", ITEM_TASK, "doing"]);

    let all = read_all_events(&dir);
    let schema_failures: Vec<&Value> = all.iter()
        .filter(|e| e["source_module"].as_str() == Some("project_schema"))
        .filter(|e| {
            let t = e["event_type"].as_str().unwrap_or("");
            t == "SchemaParseError" || t == "SchemaValidationFailed" || t == "SchemaNotFound"
        })
        .collect();

    assert!(!schema_failures.is_empty(),
        "project_schema module must emit a schema failure event when schema is invalid");
}

#[test]
fn test_schema_invalid_project_record_unchanged() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_TASK, "task", "Deploy by Friday")]);
    run_binary(&dir, &["set-status", ITEM_TASK, "todo"]); // valid — record first status

    write_project_schema(&dir, "this: is: not: valid: yaml: [[[");
    run_binary(&dir, &["set-status", ITEM_TASK, "doing"]); // fails — schema invalid

    let events = read_is_events(&dir);
    let updated: Vec<&Value> = events.iter()
        .filter(|e| e["event_type"] == "ItemStatusUpdated")
        .collect();

    assert_eq!(updated.len(), 1, "Only the first set-status (valid schema) must produce ItemStatusUpdated");
    assert_eq!(updated[0]["payload"]["new_status"].as_str().unwrap(), "todo",
        "Project record must be unchanged after schema failure");
}

// ── R5: Marker-derived effective status (HP2) ─────────────────────────────────

const MARKER_SCHEMA: &str = r#"schemaVersion: 1
statuses:
  todo:
  doing:
  done:
  waiting:
  cancelled:
  pending:
  achieved:
  missed:
  open:
  mitigated:
  accepted:
  closed:
  in_progress:
  resolved:
  active:
  inactive:
pageTypes:
  Task:
    allowedStatuses: [todo, doing, done, waiting, cancelled]
    aliases: [task]
blockTypes:
  taskBlock:
    markers:
      TODO: todo
      DOING: doing
      DONE: done
"#;

#[test]
fn test_marker_derived_effective_status_at_query_time() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, MARKER_SCHEMA);
    // Description starts with "TODO" — the vocabulary maps TODO → todo
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_TASK, "task", "TODO Deploy by Friday")]);

    run_binary(&dir, &["get", ITEM_TASK]);

    let events = read_is_events(&dir);
    let returned = events.iter()
        .find(|e| e["event_type"] == "ItemStatusReturned")
        .expect("ItemStatusReturned must be emitted");

    assert_eq!(returned["payload"]["current_status"].as_str().unwrap(), "todo",
        "effective status must be the vocabulary mapping for the TODO marker");
    assert_eq!(returned["payload"]["status_source"].as_str().unwrap(), "marker_derived",
        "status_source must be 'marker_derived' when effective status comes from a task marker");
}

#[test]
fn test_marker_derived_emits_no_failure_signal() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, MARKER_SCHEMA);
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_TASK, "task", "TODO Deploy by Friday")]);

    run_binary(&dir, &["get", ITEM_TASK]);

    let events = read_is_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(!types.contains(&"ItemStatusUnrecognized"),
        "marker-derived status must NOT emit ItemStatusUnrecognized");
    assert!(!types.iter().any(|t| t.contains("Failed")),
        "marker-derived resolution must not emit any failure event");
}

// ── R5: Explicit status takes precedence over marker (HP3) ───────────────────

#[test]
fn test_explicit_status_overrides_marker_derived() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, MARKER_SCHEMA);
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_TASK, "task", "TODO Deploy by Friday")]);

    run_binary(&dir, &["set-status", ITEM_TASK, "doing"]);
    run_binary(&dir, &["get", ITEM_TASK]);

    let events = read_is_events(&dir);
    let returned = events.iter()
        .find(|e| e["event_type"] == "ItemStatusReturned")
        .expect("ItemStatusReturned must be emitted");

    assert_eq!(returned["payload"]["current_status"].as_str().unwrap(), "doing",
        "explicit set-status must override marker-derived status");
    assert_eq!(returned["payload"]["status_source"].as_str().unwrap(), "explicit",
        "status_source must be 'explicit' when an explicit ItemStatusUpdated event exists");
}

// ── R5: Unmapped marker → proposed-value fallback (HP4) ──────────────────────

#[test]
fn test_unmapped_marker_falls_through_to_proposed_value() {
    let dir = setup_temp_dir();
    // Schema with no marker mappings for "NOW"
    write_project_schema(&dir, MARKER_SCHEMA);
    seed_with_proposed(&dir, SESSION_A, &[
        (ITEM_TASK, "task", "NOW Deploy by Friday", Some("todo"), None),
    ]);

    run_binary(&dir, &["get", ITEM_TASK]);

    let events = read_is_events(&dir);
    let returned = events.iter()
        .find(|e| e["event_type"] == "ItemStatusReturned")
        .expect("ItemStatusReturned must be emitted");

    assert_eq!(returned["payload"]["current_status"].as_str().unwrap(), "todo",
        "unmapped marker must fall through to proposed status");
    assert_eq!(returned["payload"]["status_source"].as_str().unwrap(), "proposed",
        "status_source must be 'proposed' when marker is unmapped and proposed value exists");
}

#[test]
fn test_unmapped_marker_emits_no_failure_signal() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, MARKER_SCHEMA);
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_TASK, "task", "NOW Deploy by Friday")]);

    run_binary(&dir, &["get", ITEM_TASK]);

    let events = read_is_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(!types.iter().any(|t| t.contains("Failed")),
        "unmapped marker must produce no failure signal");
    assert!(!types.contains(&"ItemStatusUnrecognized"),
        "unmapped marker must not emit ItemStatusUnrecognized");
}

// ── R5: Stale recorded status (HP5) ──────────────────────────────────────────

#[test]
fn test_stale_status_emits_item_status_unrecognized() {
    let dir = setup_temp_dir();
    // First: use a schema where "legacy_value" is valid, record the status
    write_project_schema(&dir, r#"schemaVersion: 1
statuses:
  todo:
  doing:
  done:
  waiting:
  cancelled:
  pending:
  achieved:
  missed:
  open:
  mitigated:
  accepted:
  closed:
  in_progress:
  resolved:
  active:
  inactive:
  legacy_value:
pageTypes:
  Task:
    allowedStatuses: [todo, doing, done, legacy_value]
    aliases: [task]
"#);
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_TASK, "task", "Deploy by Friday")]);
    run_binary(&dir, &["set-status", ITEM_TASK, "legacy_value"]);

    // Now: change schema so "legacy_value" is no longer recognized
    write_project_schema(&dir, DEFAULT_SCHEMA);
    run_binary(&dir, &["get", ITEM_TASK]);

    let events = read_is_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"ItemStatusUnrecognized"),
        "ItemStatusUnrecognized must be emitted when the recorded status is no longer in the vocabulary");
}

#[test]
fn test_stale_status_unrecognized_emitted_before_returned() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, r#"schemaVersion: 1
statuses:
  todo:
  doing:
  done:
  waiting:
  cancelled:
  pending:
  achieved:
  missed:
  open:
  mitigated:
  accepted:
  closed:
  in_progress:
  resolved:
  active:
  inactive:
  legacy_value:
pageTypes:
  Task:
    allowedStatuses: [todo, doing, done, legacy_value]
    aliases: [task]
"#);
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_TASK, "task", "Deploy by Friday")]);
    run_binary(&dir, &["set-status", ITEM_TASK, "legacy_value"]);

    write_project_schema(&dir, DEFAULT_SCHEMA);
    run_binary(&dir, &["get", ITEM_TASK]);

    let events = read_is_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    let unrecognized_pos = types.iter().position(|&t| t == "ItemStatusUnrecognized")
        .expect("ItemStatusUnrecognized must be present");
    let returned_pos = types.iter().position(|&t| t == "ItemStatusReturned")
        .expect("ItemStatusReturned must be present");

    assert!(unrecognized_pos < returned_pos,
        "ItemStatusUnrecognized must be emitted before ItemStatusReturned");
}

#[test]
fn test_stale_status_still_returned_as_effective_status() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, r#"schemaVersion: 1
statuses:
  todo:
  doing:
  done:
  waiting:
  cancelled:
  pending:
  achieved:
  missed:
  open:
  mitigated:
  accepted:
  closed:
  in_progress:
  resolved:
  active:
  inactive:
  legacy_value:
pageTypes:
  Task:
    allowedStatuses: [todo, doing, done, legacy_value]
    aliases: [task]
"#);
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_TASK, "task", "Deploy by Friday")]);
    run_binary(&dir, &["set-status", ITEM_TASK, "legacy_value"]);

    write_project_schema(&dir, DEFAULT_SCHEMA);
    run_binary(&dir, &["get", ITEM_TASK]);

    let events = read_is_events(&dir);
    let returned = events.iter()
        .find(|e| e["event_type"] == "ItemStatusReturned")
        .expect("ItemStatusReturned must be emitted");

    assert_eq!(returned["payload"]["current_status"].as_str().unwrap(), "legacy_value",
        "stale recorded status must still be returned as the effective status");
    assert_eq!(returned["payload"]["status_source"].as_str().unwrap(), "explicit",
        "status_source must be 'explicit' — the value came from an ItemStatusUpdated event");
}

#[test]
fn test_stale_status_unrecognized_is_not_a_failure_event() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, r#"schemaVersion: 1
statuses:
  todo:
  doing:
  done:
  waiting:
  cancelled:
  pending:
  achieved:
  missed:
  open:
  mitigated:
  accepted:
  closed:
  in_progress:
  resolved:
  active:
  inactive:
  legacy_value:
pageTypes:
  Task:
    allowedStatuses: [todo, doing, done, legacy_value]
    aliases: [task]
"#);
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_TASK, "task", "Deploy by Friday")]);
    run_binary(&dir, &["set-status", ITEM_TASK, "legacy_value"]);

    write_project_schema(&dir, DEFAULT_SCHEMA);
    let output = run_binary(&dir, &["get", ITEM_TASK]);

    assert!(output.status.success(),
        "get must exit successfully even when ItemStatusUnrecognized is emitted");

    let events = read_is_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();
    assert!(types.contains(&"ItemStatusReturned"),
        "ItemStatusReturned must still be emitted — ItemStatusUnrecognized is not a failure");
}

#[test]
fn test_stale_status_unrecognized_payload() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, r#"schemaVersion: 1
statuses:
  todo:
  doing:
  done:
  waiting:
  cancelled:
  pending:
  achieved:
  missed:
  open:
  mitigated:
  accepted:
  closed:
  in_progress:
  resolved:
  active:
  inactive:
  legacy_value:
pageTypes:
  Task:
    allowedStatuses: [todo, doing, done, legacy_value]
    aliases: [task]
"#);
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_TASK, "task", "Deploy by Friday")]);
    run_binary(&dir, &["set-status", ITEM_TASK, "legacy_value"]);

    write_project_schema(&dir, DEFAULT_SCHEMA);
    run_binary(&dir, &["get", ITEM_TASK]);

    let events = read_is_events(&dir);
    let unrecognized = events.iter()
        .find(|e| e["event_type"] == "ItemStatusUnrecognized")
        .expect("ItemStatusUnrecognized must be present");

    assert_eq!(unrecognized["payload"]["item_id"].as_str().unwrap(), ITEM_TASK);
    assert_eq!(unrecognized["payload"]["item_type"].as_str().unwrap(), "task");
    assert_eq!(unrecognized["payload"]["recorded_status"].as_str().unwrap(), "legacy_value");
}

#[test]
fn test_stale_status_unrecognized_emitted_exactly_once_per_get() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, r#"schemaVersion: 1
statuses:
  todo:
  doing:
  done:
  waiting:
  cancelled:
  pending:
  achieved:
  missed:
  open:
  mitigated:
  accepted:
  closed:
  in_progress:
  resolved:
  active:
  inactive:
  legacy_value:
pageTypes:
  Task:
    allowedStatuses: [todo, doing, done, legacy_value]
    aliases: [task]
"#);
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_TASK, "task", "Deploy by Friday")]);
    run_binary(&dir, &["set-status", ITEM_TASK, "legacy_value"]);

    write_project_schema(&dir, DEFAULT_SCHEMA);
    run_binary(&dir, &["get", ITEM_TASK]);

    let events = read_is_events(&dir);
    let count = events.iter()
        .filter(|e| e["event_type"] == "ItemStatusUnrecognized")
        .count();

    assert_eq!(count, 1,
        "ItemStatusUnrecognized must be emitted exactly once per get invocation");
}

// ── R5: status_source field (ItemStatusReturned payload amendment) ────────────

#[test]
fn test_get_status_source_explicit_when_explicitly_set() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_TASK, "task", "Deploy by Friday")]);
    run_binary(&dir, &["set-status", ITEM_TASK, "doing"]);
    run_binary(&dir, &["get", ITEM_TASK]);

    let events = read_is_events(&dir);
    let returned = events.iter()
        .find(|e| e["event_type"] == "ItemStatusReturned")
        .expect("ItemStatusReturned must be present");

    assert_eq!(returned["payload"]["status_source"].as_str().unwrap(), "explicit",
        "status_source must be 'explicit' when an ItemStatusUpdated event exists");
}

#[test]
fn test_get_status_source_proposed_when_from_proposed_value() {
    let dir = setup_temp_dir();
    seed_with_proposed(&dir, SESSION_A, &[
        (ITEM_TASK, "task", "Deploy by Friday", Some("todo"), None),
    ]);
    run_binary(&dir, &["get", ITEM_TASK]);

    let events = read_is_events(&dir);
    let returned = events.iter()
        .find(|e| e["event_type"] == "ItemStatusReturned")
        .expect("ItemStatusReturned must be present");

    assert_eq!(returned["payload"]["status_source"].as_str().unwrap(), "proposed",
        "status_source must be 'proposed' when effective status comes from proposed_status");
}

#[test]
fn test_get_status_source_null_when_no_status_available() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_TASK, "task", "Deploy by Friday")]);
    run_binary(&dir, &["get", ITEM_TASK]);

    let events = read_is_events(&dir);
    let returned = events.iter()
        .find(|e| e["event_type"] == "ItemStatusReturned")
        .expect("ItemStatusReturned must be present");

    assert!(returned["payload"]["status_source"].is_null(),
        "status_source must be null when effective status is null");
    assert!(returned["payload"]["current_status"].is_null(),
        "current_status must also be null");
}

//! Behavioral tests for priority_view.
//!
//! Tests verify observable outcomes: events emitted, payload shapes, sort ordering,
//! filter behavior, and failure modes. No internal logic is tested.
//! All assertions reference event names from events/priority_view_schema.md exactly.

use serde_json::{json, Value};
use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};
use tempfile::TempDir;

fn binary_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_priority_view"))
}

fn setup_temp_dir() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("events")).unwrap();
    dir
}

/// Seed pm_structuring + project_state events so priority_view can find items.
/// items: (item_id, item_type, description)
fn seed_incorporated_items(dir: &TempDir, session_id: &str, items: &[(&str, &str, &str)]) {
    let items_json: Vec<Value> = items.iter().map(|(id, typ, desc)| json!({
        "item_id": id,
        "item_type": typ,
        "description": desc,
        "uncertain": false,
        "uncertainty_reason": null,
        "proposed_status": null,
        "proposed_priority": null,
    })).collect();

    let accepted_ids: Vec<&str> = items.iter().map(|(id, _, _)| *id).collect();

    let path = dir.path().join("events/runtime_events.jsonl");
    let mut file = fs::OpenOptions::new().create(true).append(true).open(&path).unwrap();

    writeln!(file, "{}", json!({
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
    })).unwrap();

    writeln!(file, "{}", json!({
        "event_id": format!("seed-conf-{}", &session_id[..8]),
        "event_type": "ExtractionConfirmed",
        "timestamp": 1748000002000u64,
        "correlation_id": session_id,
        "source_module": "pm_structuring",
        "payload": {
            "accepted_item_ids": accepted_ids,
            "accepted_count": items.len()
        }
    })).unwrap();

    writeln!(file, "{}", json!({
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
    })).unwrap();
}

/// Append an ItemStatusUpdated event directly to the event log.
fn seed_status(dir: &TempDir, item_id: &str, item_type: &str, status: &str) {
    let path = dir.path().join("events/runtime_events.jsonl");
    let mut file = fs::OpenOptions::new().create(true).append(true).open(&path).unwrap();
    writeln!(file, "{}", json!({
        "event_id": format!("seed-sta-{}", &item_id[..8]),
        "event_type": "ItemStatusUpdated",
        "timestamp": 1748000010000u64,
        "correlation_id": format!("sta-corr-{}", &item_id[..8]),
        "source_module": "item_status",
        "payload": {
            "item_id": item_id,
            "item_type": item_type,
            "new_status": status,
            "previous_status": null
        }
    })).unwrap();
}

/// Append an ItemPriorityUpdated event directly to the event log.
fn seed_priority(dir: &TempDir, item_id: &str, priority: &str) {
    let path = dir.path().join("events/runtime_events.jsonl");
    let mut file = fs::OpenOptions::new().create(true).append(true).open(&path).unwrap();
    writeln!(file, "{}", json!({
        "event_id": format!("seed-pri-{}", &item_id[..8]),
        "event_type": "ItemPriorityUpdated",
        "timestamp": 1748000011000u64,
        "correlation_id": format!("pri-corr-{}", &item_id[..8]),
        "source_module": "item_status",
        "payload": {
            "item_id": item_id,
            "new_priority": priority,
            "previous_priority": null
        }
    })).unwrap();
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

/// Read only priority_view events from the events file.
fn read_pv_events(dir: &TempDir) -> Vec<Value> {
    let path = dir.path().join("events/runtime_events.jsonl");
    if !path.exists() { return vec![]; }
    fs::read_to_string(path).unwrap()
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .filter(|e| e["source_module"].as_str() == Some("priority_view"))
        .collect()
}

fn write_project_schema(dir: &TempDir, yaml: &str) {
    fs::write(dir.path().join("project-schema.yaml"), yaml).unwrap();
}

/// Read ALL events from the log (all source_modules).
fn read_all_events(dir: &TempDir) -> Vec<Value> {
    let path = dir.path().join("events/runtime_events.jsonl");
    if !path.exists() { return vec![]; }
    fs::read_to_string(path).unwrap()
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .collect()
}

/// Like run_binary but removes HOME to prevent default schema merge.
fn run_binary_isolated(dir: &TempDir, args: &[&str]) -> std::process::Output {
    Command::new(binary_path())
        .current_dir(dir.path())
        .args(args)
        .env_remove("HOME")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("Failed to run binary")
}

const SESSION_A: &str = "a4ca3a7e-61eb-4f36-b59e-f3abd166e351";

const ITEM_TASK_HIGH_DOING:   &str = "b1000000-0000-0000-0000-000000000001";
const ITEM_TASK_LOW_TODO:     &str = "b1000000-0000-0000-0000-000000000002";
const ITEM_RISK_HIGH_OPEN:    &str = "b1000000-0000-0000-0000-000000000003";
const ITEM_MILESTONE_MED_PND: &str = "b1000000-0000-0000-0000-000000000004";
const ITEM_ISSUE_NO_PRI:      &str = "b1000000-0000-0000-0000-000000000005";

/// Seed a representative multi-item record for ordering tests.
fn seed_multi_item_record(dir: &TempDir) {
    seed_incorporated_items(dir, SESSION_A, &[
        (ITEM_TASK_HIGH_DOING,   "task",      "Fix critical data loss bug"),
        (ITEM_TASK_LOW_TODO,     "task",      "Write release notes"),
        (ITEM_RISK_HIGH_OPEN,    "risk",      "Vendor lock-in risk"),
        (ITEM_MILESTONE_MED_PND, "milestone", "Q3 release"),
        (ITEM_ISSUE_NO_PRI,      "issue",     "Login page is slow"),
    ]);
    seed_status(dir, ITEM_TASK_HIGH_DOING,   "task",      "doing");
    seed_priority(dir, ITEM_TASK_HIGH_DOING,               "high");
    seed_status(dir, ITEM_TASK_LOW_TODO,     "task",      "todo");
    seed_priority(dir, ITEM_TASK_LOW_TODO,                 "low");
    seed_status(dir, ITEM_RISK_HIGH_OPEN,    "risk",      "open");
    seed_priority(dir, ITEM_RISK_HIGH_OPEN,                "high");
    seed_status(dir, ITEM_MILESTONE_MED_PND, "milestone", "pending");
    seed_priority(dir, ITEM_MILESTONE_MED_PND,             "medium");
    seed_status(dir, ITEM_ISSUE_NO_PRI,      "issue",     "in_progress");
    // ITEM_ISSUE_NO_PRI intentionally has no priority set
}

// ── Happy Path 1: Unfiltered view ────────────────────────────────────────────

#[test]
fn test_unfiltered_view_emits_requested_then_returned() {
    let dir = setup_temp_dir();
    seed_multi_item_record(&dir);

    run_binary(&dir, &[]);

    let events = read_pv_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"PriorityViewRequested"), "PriorityViewRequested must be emitted");
    assert!(types.contains(&"PriorityViewReturned"),  "PriorityViewReturned must be emitted");
    assert!(!types.contains(&"PriorityViewFailedEmptyRecord"),   "must NOT emit EmptyRecord failure");
    assert!(!types.contains(&"PriorityViewFailedInvalidFilter"), "must NOT emit InvalidFilter failure");

    let req_pos = types.iter().position(|&t| t == "PriorityViewRequested").unwrap();
    let ret_pos = types.iter().position(|&t| t == "PriorityViewReturned").unwrap();
    assert!(req_pos < ret_pos, "PriorityViewRequested must precede PriorityViewReturned");
}

#[test]
fn test_unfiltered_view_returned_payload_shape() {
    let dir = setup_temp_dir();
    seed_multi_item_record(&dir);

    run_binary(&dir, &[]);

    let events = read_pv_events(&dir);
    let returned = events.iter()
        .find(|e| e["event_type"] == "PriorityViewReturned")
        .expect("PriorityViewReturned not found");

    let p = &returned["payload"];
    assert!(p["item_count"].as_u64().is_some(), "item_count must be a number");
    assert_eq!(p["item_count"].as_u64().unwrap(), 5, "all 5 items must be returned unfiltered");
    assert!(p["filters_applied"].is_object(), "filters_applied must be an object");
    assert!(p["items"].is_array(), "items must be an array");
    assert_eq!(p["items"].as_array().unwrap().len(), 5);
}

#[test]
fn test_each_returned_item_has_required_fields() {
    let dir = setup_temp_dir();
    seed_multi_item_record(&dir);

    run_binary(&dir, &[]);

    let events = read_pv_events(&dir);
    let returned = events.iter()
        .find(|e| e["event_type"] == "PriorityViewReturned")
        .expect("PriorityViewReturned not found");

    for item in returned["payload"]["items"].as_array().unwrap() {
        assert!(item["item_id"].as_str().is_some(),    "item must have item_id");
        assert!(item["item_type"].as_str().is_some(),  "item must have item_type");
        assert!(item["description"].as_str().is_some(),"item must have description");
        assert!(item["session_id"].as_str().is_some(), "item must have session_id");
        assert!(item.get("priority").is_some(),        "item must have priority field (may be null)");
        assert!(item.get("status").is_some(),          "item must have status field (may be null)");
    }
}

#[test]
fn test_items_with_explicit_priority_ranked_before_items_without() {
    let dir = setup_temp_dir();
    seed_multi_item_record(&dir);

    run_binary(&dir, &[]);

    let events = read_pv_events(&dir);
    let returned = events.iter()
        .find(|e| e["event_type"] == "PriorityViewReturned")
        .expect("PriorityViewReturned not found");

    let items = returned["payload"]["items"].as_array().unwrap();
    let no_pri_pos = items.iter()
        .position(|i| i["item_id"].as_str() == Some(ITEM_ISSUE_NO_PRI))
        .expect("issue item must be in results");

    // every item with an explicit priority must appear before the no-priority item
    for (pos, item) in items.iter().enumerate() {
        if item["priority"].is_string() {
            assert!(pos < no_pri_pos,
                "item with priority '{}' must rank before item with no priority",
                item["priority"].as_str().unwrap());
        }
    }
}

#[test]
fn test_priority_ordering_high_before_medium_before_low() {
    let dir = setup_temp_dir();
    seed_multi_item_record(&dir);

    run_binary(&dir, &[]);

    let events = read_pv_events(&dir);
    let returned = events.iter()
        .find(|e| e["event_type"] == "PriorityViewReturned")
        .expect("PriorityViewReturned not found");

    let items = returned["payload"]["items"].as_array().unwrap();

    let high_positions: Vec<usize> = items.iter().enumerate()
        .filter(|(_, i)| i["priority"].as_str() == Some("high"))
        .map(|(pos, _)| pos)
        .collect();
    let med_positions: Vec<usize> = items.iter().enumerate()
        .filter(|(_, i)| i["priority"].as_str() == Some("medium"))
        .map(|(pos, _)| pos)
        .collect();
    let low_positions: Vec<usize> = items.iter().enumerate()
        .filter(|(_, i)| i["priority"].as_str() == Some("low"))
        .map(|(pos, _)| pos)
        .collect();

    let max_high = high_positions.iter().max().expect("must have high priority items");
    let min_med  = med_positions.iter().min().expect("must have medium priority items");
    let max_med  = med_positions.iter().max().unwrap();
    let min_low  = low_positions.iter().min().expect("must have low priority items");

    assert!(max_high < min_med, "all high-priority items must precede medium-priority items");
    assert!(max_med  < min_low, "all medium-priority items must precede low-priority items");
}

#[test]
fn test_equal_priority_doing_ranked_before_open_and_todo() {
    let dir = setup_temp_dir();
    seed_multi_item_record(&dir);

    run_binary(&dir, &[]);

    let events = read_pv_events(&dir);
    let returned = events.iter()
        .find(|e| e["event_type"] == "PriorityViewReturned")
        .expect("PriorityViewReturned not found");

    let items = returned["payload"]["items"].as_array().unwrap();

    let doing_pos = items.iter()
        .position(|i| i["item_id"].as_str() == Some(ITEM_TASK_HIGH_DOING))
        .expect("high-priority doing task must be in results");
    let open_pos = items.iter()
        .position(|i| i["item_id"].as_str() == Some(ITEM_RISK_HIGH_OPEN))
        .expect("high-priority open risk must be in results");

    assert!(doing_pos < open_pos,
        "doing (rank 1) must appear before open (rank 2) at equal priority");
}

#[test]
fn test_unfiltered_requested_payload_has_null_filters() {
    let dir = setup_temp_dir();
    seed_multi_item_record(&dir);

    run_binary(&dir, &[]);

    let events = read_pv_events(&dir);
    let requested = events.iter()
        .find(|e| e["event_type"] == "PriorityViewRequested")
        .expect("PriorityViewRequested not found");

    assert!(requested["payload"]["filter_type"].is_null(),     "filter_type must be null when not specified");
    assert!(requested["payload"]["filter_status"].is_null(),   "filter_status must be null when not specified");
    assert!(requested["payload"]["filter_priority"].is_null(), "filter_priority must be null when not specified");
}

// ── Happy Path 2: Filtered view ───────────────────────────────────────────────

#[test]
fn test_filter_by_type_returns_only_matching_items() {
    let dir = setup_temp_dir();
    seed_multi_item_record(&dir);

    run_binary(&dir, &["--type", "task"]);

    let events = read_pv_events(&dir);
    let returned = events.iter()
        .find(|e| e["event_type"] == "PriorityViewReturned")
        .expect("PriorityViewReturned not found");

    let items = returned["payload"]["items"].as_array().unwrap();
    assert!(!items.is_empty(), "must return at least one task");
    for item in items {
        assert_eq!(item["item_type"].as_str().unwrap(), "task",
            "filter --type task must return only task items");
    }
    assert_eq!(returned["payload"]["item_count"].as_u64().unwrap(), 2,
        "exactly 2 tasks expected");
}

#[test]
fn test_filter_by_status_returns_only_matching_items() {
    let dir = setup_temp_dir();
    seed_multi_item_record(&dir);

    run_binary(&dir, &["--status", "open"]);

    let events = read_pv_events(&dir);
    let returned = events.iter()
        .find(|e| e["event_type"] == "PriorityViewReturned")
        .expect("PriorityViewReturned not found");

    let items = returned["payload"]["items"].as_array().unwrap();
    assert!(!items.is_empty(), "must return at least one open item");
    for item in items {
        assert_eq!(item["status"].as_str().unwrap(), "open",
            "filter --status open must return only open items");
    }
}

#[test]
fn test_filter_by_priority_returns_only_matching_items() {
    let dir = setup_temp_dir();
    seed_multi_item_record(&dir);

    run_binary(&dir, &["--priority", "high"]);

    let events = read_pv_events(&dir);
    let returned = events.iter()
        .find(|e| e["event_type"] == "PriorityViewReturned")
        .expect("PriorityViewReturned not found");

    let items = returned["payload"]["items"].as_array().unwrap();
    assert!(!items.is_empty(), "must return at least one high priority item");
    for item in items {
        assert_eq!(item["priority"].as_str().unwrap(), "high",
            "filter --priority high must return only high priority items");
    }
    assert_eq!(returned["payload"]["item_count"].as_u64().unwrap(), 2,
        "exactly 2 high priority items expected");
}

#[test]
fn test_multiple_filters_are_conjunctive() {
    let dir = setup_temp_dir();
    seed_multi_item_record(&dir);

    // Only items that are both type=task AND priority=high
    run_binary(&dir, &["--type", "task", "--priority", "high"]);

    let events = read_pv_events(&dir);
    let returned = events.iter()
        .find(|e| e["event_type"] == "PriorityViewReturned")
        .expect("PriorityViewReturned not found");

    let items = returned["payload"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "only one task with high priority exists");
    assert_eq!(items[0]["item_id"].as_str().unwrap(), ITEM_TASK_HIGH_DOING);
}

#[test]
fn test_filters_applied_echo_in_returned_event() {
    let dir = setup_temp_dir();
    seed_multi_item_record(&dir);

    run_binary(&dir, &["--type", "task", "--priority", "high"]);

    let events = read_pv_events(&dir);
    let returned = events.iter()
        .find(|e| e["event_type"] == "PriorityViewReturned")
        .expect("PriorityViewReturned not found");

    let fa = &returned["payload"]["filters_applied"];
    assert_eq!(fa["type"].as_str().unwrap(), "task");
    assert_eq!(fa["priority"].as_str().unwrap(), "high");
    assert!(fa["status"].is_null(), "status filter must be null when not specified");
}

// ── Happy Path 3: Filtered view with no matching items ────────────────────────

#[test]
fn test_filtered_view_no_matches_returns_empty_list_not_failure() {
    let dir = setup_temp_dir();
    seed_multi_item_record(&dir);

    // No item has status=cancelled in our seeded record
    run_binary(&dir, &["--status", "cancelled"]);

    let events = read_pv_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"PriorityViewReturned"),
        "PriorityViewReturned must be emitted even when no items match filters");
    assert!(!types.contains(&"PriorityViewFailedEmptyRecord"),
        "EmptyRecord must NOT be emitted when the record has items but filters match none");
    assert!(!types.contains(&"PriorityViewFailedInvalidFilter"),
        "InvalidFilter must NOT be emitted for a valid status value");

    let returned = events.iter()
        .find(|e| e["event_type"] == "PriorityViewReturned")
        .unwrap();
    assert_eq!(returned["payload"]["item_count"].as_u64().unwrap(), 0);
    assert!(returned["payload"]["items"].as_array().unwrap().is_empty());
}

// ── Failure Path 1: EmptyRecord ───────────────────────────────────────────────

#[test]
fn test_empty_record_emits_failure_event() {
    let dir = setup_temp_dir();
    // Events directory exists but no items in the record

    run_binary(&dir, &[]);

    let events = read_pv_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"PriorityViewRequested"),        "PriorityViewRequested must be emitted");
    assert!(types.contains(&"PriorityViewFailedEmptyRecord"),"PriorityViewFailedEmptyRecord must be emitted");
    assert!(!types.contains(&"PriorityViewReturned"),         "PriorityViewReturned must NOT be emitted");
}

#[test]
fn test_empty_record_failure_reason() {
    let dir = setup_temp_dir();

    run_binary(&dir, &[]);

    let events = read_pv_events(&dir);
    let failure = events.iter()
        .find(|e| e["event_type"] == "PriorityViewFailedEmptyRecord")
        .expect("PriorityViewFailedEmptyRecord not found");

    assert_eq!(failure["payload"]["failure_reason"].as_str().unwrap(), "empty_record");
}

#[test]
fn test_empty_record_requested_before_failure() {
    let dir = setup_temp_dir();

    run_binary(&dir, &[]);

    let events = read_pv_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    let req_pos = types.iter().position(|&t| t == "PriorityViewRequested").unwrap();
    let fail_pos = types.iter().position(|&t| t == "PriorityViewFailedEmptyRecord").unwrap();
    assert!(req_pos < fail_pos, "PriorityViewRequested must precede PriorityViewFailedEmptyRecord");
}

// ── Failure Path 2: InvalidFilter ────────────────────────────────────────────

#[test]
fn test_invalid_type_emits_invalid_filter_failure() {
    let dir = setup_temp_dir();
    seed_multi_item_record(&dir);

    run_binary(&dir, &["--type", "epic"]);

    let events = read_pv_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"PriorityViewRequested"),          "PriorityViewRequested must be emitted");
    assert!(types.contains(&"PriorityViewFailedInvalidFilter"),"PriorityViewFailedInvalidFilter must be emitted");
    assert!(!types.contains(&"PriorityViewReturned"),           "PriorityViewReturned must NOT be emitted");
}

#[test]
fn test_invalid_status_emits_invalid_filter_failure() {
    let dir = setup_temp_dir();
    seed_multi_item_record(&dir);

    run_binary(&dir, &["--status", "blocked"]);

    let events = read_pv_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"PriorityViewFailedInvalidFilter"),
        "PriorityViewFailedInvalidFilter must be emitted for invalid status");
    assert!(!types.contains(&"PriorityViewReturned"),
        "PriorityViewReturned must NOT be emitted");
}

#[test]
fn test_invalid_priority_emits_invalid_filter_failure() {
    let dir = setup_temp_dir();
    seed_multi_item_record(&dir);

    run_binary(&dir, &["--priority", "critical"]);

    let events = read_pv_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"PriorityViewFailedInvalidFilter"),
        "PriorityViewFailedInvalidFilter must be emitted for invalid priority");
    assert!(!types.contains(&"PriorityViewReturned"),
        "PriorityViewReturned must NOT be emitted");
}

#[test]
fn test_invalid_filter_payload_identifies_field_and_value() {
    let dir = setup_temp_dir();
    seed_multi_item_record(&dir);

    run_binary(&dir, &["--type", "epic"]);

    let events = read_pv_events(&dir);
    let failure = events.iter()
        .find(|e| e["event_type"] == "PriorityViewFailedInvalidFilter")
        .expect("PriorityViewFailedInvalidFilter not found");

    assert_eq!(failure["payload"]["failure_reason"].as_str().unwrap(), "invalid_filter");
    assert_eq!(failure["payload"]["filter_field"].as_str().unwrap(),   "type");
    assert_eq!(failure["payload"]["filter_value"].as_str().unwrap(),   "epic");
}

// ── Invariants ────────────────────────────────────────────────────────────────

#[test]
fn test_view_does_not_emit_status_or_priority_update_events() {
    let dir = setup_temp_dir();
    seed_multi_item_record(&dir);

    run_binary(&dir, &[]);
    run_binary(&dir, &[]);

    let events = read_pv_events(&dir);
    for event in &events {
        let t = event["event_type"].as_str().unwrap();
        assert!(!t.contains("Updated"),
            "priority_view must not emit any Updated events; got '{}'", t);
    }
}

#[test]
fn test_running_view_twice_emits_same_item_count() {
    let dir = setup_temp_dir();
    seed_multi_item_record(&dir);

    run_binary(&dir, &[]);
    run_binary(&dir, &[]);

    let events = read_pv_events(&dir);
    let counts: Vec<u64> = events.iter()
        .filter(|e| e["event_type"] == "PriorityViewReturned")
        .map(|e| e["payload"]["item_count"].as_u64().unwrap())
        .collect();

    assert_eq!(counts.len(), 2, "must have two PriorityViewReturned events");
    assert_eq!(counts[0], counts[1],
        "item_count must be identical across invocations (view is read-only)");
}

// ── Telemetry ─────────────────────────────────────────────────────────────────

#[test]
fn test_all_events_have_required_base_fields() {
    let dir = setup_temp_dir();
    seed_multi_item_record(&dir);
    run_binary(&dir, &[]);

    let events = read_pv_events(&dir);
    assert!(!events.is_empty());

    for event in &events {
        let t = event["event_type"].as_str().unwrap_or("unknown");
        assert!(event["event_id"].as_str().is_some(),       "{}: event_id must be a string", t);
        assert!(event["event_type"].as_str().is_some(),     "{}: event_type must be a string", t);
        assert!(event["timestamp"].as_u64().is_some(),      "{}: timestamp must be a u64", t);
        assert!(event["correlation_id"].as_str().is_some(), "{}: correlation_id must be a string", t);
        assert!(event["source_module"].as_str().is_some(),  "{}: source_module must be a string", t);
        assert!(event["payload"].is_object(),               "{}: payload must be an object", t);
        assert_eq!(event["source_module"].as_str().unwrap(), "priority_view",
            "{}: source_module must be 'priority_view'", t);
        assert!(event["timestamp"].as_u64().unwrap() > 0,
            "{}: timestamp must be positive", t);
    }
}

#[test]
fn test_correlation_id_consistent_within_one_invocation() {
    let dir = setup_temp_dir();
    seed_multi_item_record(&dir);
    run_binary(&dir, &[]);

    let events = read_pv_events(&dir);
    assert!(events.len() >= 2);

    let cid = events[0]["correlation_id"].as_str().unwrap();
    for event in &events {
        assert_eq!(event["correlation_id"].as_str().unwrap(), cid,
            "all events from one invocation must share the same correlation_id");
    }
}

#[test]
fn test_separate_invocations_have_different_correlation_ids() {
    let dir = setup_temp_dir();
    seed_multi_item_record(&dir);

    run_binary(&dir, &[]);
    run_binary(&dir, &[]);

    let events = read_pv_events(&dir);
    let cids: Vec<&str> = events.iter()
        .filter(|e| e["event_type"] == "PriorityViewRequested")
        .map(|e| e["correlation_id"].as_str().unwrap())
        .collect();

    assert_eq!(cids.len(), 2, "must have two PriorityViewRequested events");
    assert_ne!(cids[0], cids[1],
        "different invocations must produce different correlation_ids");
}

// ── R7: Schema-driven filters ─────────────────────────────────────────────────

// Vocabulary schemas used by isolated tests (HOME removed to prevent default merge).

const CUSTOM_VOCAB: &str = r#"schemaVersion: 1
statuses:
  backlog:
  in_flight:
  shipped:
  open:
  resolved:
pageTypes:
  Feature:
    allowedStatuses: [backlog, in_flight, shipped]
    aliases: [feature]
  Bug:
    allowedStatuses: [open, resolved]
    aliases: [bug]
"#;

const ALIAS_VOCAB: &str = r#"schemaVersion: 1
statuses:
  active:
  inactive:
pageTypes:
  Initiative:
    allowedStatuses: [active, inactive]
    aliases: [epic]
"#;

const ITEM_FEATURE_1: &str = "f1000000-0000-0000-0000-000000000001";
const _ITEM_FEATURE_2: &str = "f1000000-0000-0000-0000-000000000002";
const ITEM_BUG_1:     &str = "f1000000-0000-0000-0000-000000000003";
const ITEM_WIDGET:    &str = "f1000000-0000-0000-0000-000000000004";
const ITEM_WIDGET_2:  &str = "f1000000-0000-0000-0000-000000000005";
const ITEM_INITIATIVE: &str = "f1000000-0000-0000-0000-000000000006";
const ITEM_EPIC:       &str = "f1000000-0000-0000-0000-000000000007";

// ── R7: HP1 — Custom vocabulary type filter succeeds ─────────────────────────

#[test]
fn test_r7_custom_vocabulary_type_filter_succeeds() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, CUSTOM_VOCAB);
    seed_incorporated_items(&dir, SESSION_A, &[
        (ITEM_FEATURE_1, "feature", "Ship search feature"),
        (ITEM_BUG_1,     "bug",     "Login crashes on mobile"),
    ]);

    run_binary_isolated(&dir, &["--type", "feature"]);

    let events = read_pv_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"PriorityViewReturned"),
        "PriorityViewReturned must be emitted for a valid custom vocabulary type filter");
    assert!(!types.contains(&"PriorityViewFailedInvalidFilter"),
        "InvalidFilter must NOT be emitted when type is recognized by the active vocabulary");

    let returned = events.iter().find(|e| e["event_type"] == "PriorityViewReturned").unwrap();
    let items = returned["payload"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "only the feature item should be returned");
    assert_eq!(items[0]["item_id"].as_str().unwrap(), ITEM_FEATURE_1);
}

#[test]
fn test_r7_type_filter_rejected_when_not_in_active_vocabulary() {
    // With isolated custom vocab (Feature/Bug only), "task" is not recognized.
    // This verifies filter validation uses the vocabulary, not a hardcoded list.
    let dir = setup_temp_dir();
    write_project_schema(&dir, CUSTOM_VOCAB);
    seed_incorporated_items(&dir, SESSION_A, &[
        (ITEM_FEATURE_1, "feature", "Ship search feature"),
    ]);

    run_binary_isolated(&dir, &["--type", "task"]);

    let events = read_pv_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"PriorityViewFailedInvalidFilter"),
        "InvalidFilter must be emitted when type is not in the active vocabulary");
    assert!(!types.contains(&"PriorityViewReturned"),
        "PriorityViewReturned must NOT be emitted on InvalidFilter");

    let failure = events.iter()
        .find(|e| e["event_type"] == "PriorityViewFailedInvalidFilter")
        .unwrap();
    assert_eq!(failure["payload"]["filter_field"].as_str().unwrap(), "type");
    assert_eq!(failure["payload"]["filter_value"].as_str().unwrap(), "task");
}

// ── R7: HP2 — Alias filter matching is bidirectional ─────────────────────────

#[test]
fn test_r7_alias_filter_matches_items_stored_as_canonical() {
    // Filter by alias "epic"; items stored as canonical "Initiative" must be returned.
    let dir = setup_temp_dir();
    write_project_schema(&dir, ALIAS_VOCAB);
    seed_incorporated_items(&dir, SESSION_A, &[
        (ITEM_INITIATIVE, "Initiative", "Q3 growth initiative"),
    ]);

    run_binary_isolated(&dir, &["--type", "epic"]);

    let events = read_pv_events(&dir);
    let returned = events.iter()
        .find(|e| e["event_type"] == "PriorityViewReturned")
        .expect("PriorityViewReturned must be emitted");

    let items = returned["payload"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 1,
        "item stored as canonical 'Initiative' must be matched by alias filter 'epic'");
    assert_eq!(items[0]["item_id"].as_str().unwrap(), ITEM_INITIATIVE);
}

#[test]
fn test_r7_canonical_filter_matches_items_stored_as_alias() {
    // Filter by canonical "Initiative"; items stored as alias "epic" must be returned.
    let dir = setup_temp_dir();
    write_project_schema(&dir, ALIAS_VOCAB);
    seed_incorporated_items(&dir, SESSION_A, &[
        (ITEM_EPIC, "epic", "Q4 growth epic"),
    ]);

    run_binary_isolated(&dir, &["--type", "Initiative"]);

    let events = read_pv_events(&dir);
    let returned = events.iter()
        .find(|e| e["event_type"] == "PriorityViewReturned")
        .expect("PriorityViewReturned must be emitted");

    let items = returned["payload"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 1,
        "item stored as alias 'epic' must be matched by canonical filter 'Initiative'");
    assert_eq!(items[0]["item_id"].as_str().unwrap(), ITEM_EPIC);
}

#[test]
fn test_r7_alias_filter_matches_both_canonical_and_alias_stored_items() {
    // Filter by alias; items stored as canonical AND alias are both returned.
    let dir = setup_temp_dir();
    write_project_schema(&dir, ALIAS_VOCAB);
    seed_incorporated_items(&dir, SESSION_A, &[
        (ITEM_INITIATIVE, "Initiative", "Q3 growth initiative"),
        (ITEM_EPIC,       "epic",       "Q4 growth epic"),
    ]);

    run_binary_isolated(&dir, &["--type", "epic"]);

    let events = read_pv_events(&dir);
    let returned = events.iter()
        .find(|e| e["event_type"] == "PriorityViewReturned")
        .expect("PriorityViewReturned must be emitted");

    let items = returned["payload"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 2,
        "both the canonical-stored and alias-stored items must be returned by an alias filter");
}

// ── R7: HP3 — Unrecognized items excluded; command completes ─────────────────
// Uses default schema (no isolation needed); "widget" is not a recognized type.

#[test]
fn test_r7_unrecognized_item_excluded_from_result() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[
        (ITEM_TASK_HIGH_DOING, "task",   "Fix critical bug"),
        (ITEM_WIDGET,          "widget", "Some widget"),
    ]);

    run_binary(&dir, &[]);

    let events = read_pv_events(&dir);
    let returned = events.iter()
        .find(|e| e["event_type"] == "PriorityViewReturned")
        .expect("PriorityViewReturned must be emitted — unrecognized type is not a failure");

    let items = returned["payload"]["items"].as_array().unwrap();
    assert!(items.iter().all(|i| i["item_id"].as_str() != Some(ITEM_WIDGET)),
        "item with unrecognized type must be excluded from the result");
    assert_eq!(items.len(), 1, "only the recognized-type task item must appear");
}

#[test]
fn test_r7_schema_type_unknown_emitted_for_excluded_item() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[
        (ITEM_TASK_HIGH_DOING, "task",   "Fix critical bug"),
        (ITEM_WIDGET,          "widget", "Some widget"),
    ]);

    run_binary(&dir, &[]);

    let all = read_all_events(&dir);
    let unknown_events: Vec<&Value> = all.iter()
        .filter(|e| e["source_module"].as_str() == Some("project_schema")
            && e["event_type"].as_str() == Some("SchemaTypeUnknown"))
        .collect();

    assert_eq!(unknown_events.len(), 1,
        "SchemaTypeUnknown must be emitted exactly once for the excluded widget item");
    assert_eq!(unknown_events[0]["payload"]["item_id"].as_str().unwrap(), ITEM_WIDGET);
    assert_eq!(unknown_events[0]["payload"]["unknown_type"].as_str().unwrap(), "widget");
}

#[test]
fn test_r7_unrecognized_item_exclusion_command_completes_successfully() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[
        (ITEM_TASK_HIGH_DOING, "task",   "Fix critical bug"),
        (ITEM_WIDGET,          "widget", "Some widget"),
    ]);

    let output = run_binary(&dir, &[]);

    assert!(output.status.success(),
        "command must exit successfully even when items with unrecognized types are excluded");

    let events = read_pv_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();
    assert!(types.contains(&"PriorityViewReturned"),
        "PriorityViewReturned must be emitted — exclusion is not a failure");
    assert!(!types.contains(&"PriorityViewFailedEmptyRecord"),
        "EmptyRecord must NOT fire when recognized-type items remain after exclusion");
}

// ── R7: HP4 — All items unrecognized; empty result, not EmptyRecord ──────────

#[test]
fn test_r7_all_items_unrecognized_returns_empty_result_not_failure() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[
        (ITEM_WIDGET,   "widget", "Some widget"),
        (ITEM_WIDGET_2, "gadget", "Some gadget"),
    ]);

    run_binary(&dir, &[]);

    let events = read_pv_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"PriorityViewReturned"),
        "PriorityViewReturned must be emitted even when all items are excluded");
    assert!(!types.contains(&"PriorityViewFailedEmptyRecord"),
        "EmptyRecord must NOT fire — the project record is not empty, only all types are unrecognized");

    let returned = events.iter().find(|e| e["event_type"] == "PriorityViewReturned").unwrap();
    assert_eq!(returned["payload"]["item_count"].as_u64().unwrap(), 0,
        "item_count must be 0 when all items are excluded");
}

#[test]
fn test_r7_all_items_unrecognized_schema_type_unknown_per_item() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[
        (ITEM_WIDGET,   "widget", "Some widget"),
        (ITEM_WIDGET_2, "gadget", "Some gadget"),
    ]);

    run_binary(&dir, &[]);

    let all = read_all_events(&dir);
    let unknown_count = all.iter()
        .filter(|e| e["source_module"].as_str() == Some("project_schema")
            && e["event_type"].as_str() == Some("SchemaTypeUnknown"))
        .count();

    assert_eq!(unknown_count, 2,
        "SchemaTypeUnknown must be emitted once per excluded item");
}

// ── R7: HP5 — Status globally valid but inapplicable to filtered type ─────────
// Key design decision: status filter validates against the global vocabulary
// union, not a per-type subset. A globally valid status that doesn't match
// any item of the filtered type produces an empty result — not an error.

#[test]
fn test_r7_status_globally_valid_but_locally_inapplicable_accepted() {
    // Feature allows [backlog, in_flight, shipped]; Bug allows [open, resolved].
    // Filter --type feature --status open: "open" is in the global union (from Bug),
    // so the filter is accepted. No features have status "open" → empty result.
    let dir = setup_temp_dir();
    write_project_schema(&dir, CUSTOM_VOCAB);
    seed_incorporated_items(&dir, SESSION_A, &[
        (ITEM_FEATURE_1, "feature", "Ship search feature"),
        (ITEM_BUG_1,     "bug",     "Login crash"),
    ]);
    seed_status(&dir, ITEM_FEATURE_1, "feature", "in_flight");
    seed_status(&dir, ITEM_BUG_1,     "bug",     "open");

    run_binary_isolated(&dir, &["--type", "feature", "--status", "open"]);

    let events = read_pv_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(!types.contains(&"PriorityViewFailedInvalidFilter"),
        "InvalidFilter must NOT fire — 'open' is in the global vocabulary status union");
    assert!(types.contains(&"PriorityViewReturned"),
        "PriorityViewReturned must be emitted — empty result is not a failure");

    let returned = events.iter().find(|e| e["event_type"] == "PriorityViewReturned").unwrap();
    assert_eq!(returned["payload"]["item_count"].as_u64().unwrap(), 0,
        "no features have status 'open' — result is correctly empty");
}

#[test]
fn test_r7_status_not_in_vocabulary_union_rejected() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, CUSTOM_VOCAB);
    seed_incorporated_items(&dir, SESSION_A, &[
        (ITEM_FEATURE_1, "feature", "Ship search feature"),
    ]);

    // "todo" is not in CUSTOM_VOCAB's statuses at all
    run_binary_isolated(&dir, &["--status", "todo"]);

    let events = read_pv_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"PriorityViewFailedInvalidFilter"),
        "InvalidFilter must be emitted when status is not in the vocabulary union");
    let failure = events.iter()
        .find(|e| e["event_type"] == "PriorityViewFailedInvalidFilter").unwrap();
    assert_eq!(failure["payload"]["filter_field"].as_str().unwrap(), "status");
    assert_eq!(failure["payload"]["filter_value"].as_str().unwrap(), "todo");
}

// ── R7: FP1 — SchemaInvalid aborts before PriorityViewRequested ──────────────

#[test]
fn test_r7_schema_invalid_priority_view_requested_not_emitted() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, "this: is: not: valid: yaml: [[[");
    seed_incorporated_items(&dir, SESSION_A, &[
        (ITEM_TASK_HIGH_DOING, "task", "Fix critical bug"),
    ]);

    run_binary(&dir, &[]);

    let events = read_pv_events(&dir);
    assert!(events.is_empty(),
        "No priority_view events must be emitted when the vocabulary is invalid");
}

#[test]
fn test_r7_schema_invalid_emits_project_schema_failure_event() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, "this: is: not: valid: yaml: [[[");
    seed_incorporated_items(&dir, SESSION_A, &[
        (ITEM_TASK_HIGH_DOING, "task", "Fix critical bug"),
    ]);

    run_binary(&dir, &[]);

    let all = read_all_events(&dir);
    let failures: Vec<&Value> = all.iter()
        .filter(|e| e["source_module"].as_str() == Some("project_schema"))
        .filter(|e| matches!(e["event_type"].as_str(),
            Some("SchemaParseError") | Some("SchemaValidationFailed")))
        .collect();

    assert!(!failures.is_empty(),
        "project_schema must emit a failure event when the vocabulary file is invalid");
}

#[test]
fn test_r7_schema_invalid_project_record_unchanged() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, "this: is: not: valid: yaml: [[[");
    seed_incorporated_items(&dir, SESSION_A, &[
        (ITEM_TASK_HIGH_DOING, "task", "Fix critical bug"),
    ]);

    run_binary(&dir, &[]);

    // No priority_view events, no item list returned, project record unchanged
    let all = read_all_events(&dir);
    let pv_events_count = all.iter()
        .filter(|e| e["source_module"].as_str() == Some("priority_view"))
        .count();
    assert_eq!(pv_events_count, 0,
        "no priority_view events must be written when vocabulary is invalid");
}

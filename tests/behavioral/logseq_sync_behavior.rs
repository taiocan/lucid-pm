//! Behavioral tests for logseq_sync.
//!
//! Tests verify observable outcomes: events emitted, payload shapes, ordering,
//! idempotency, and per-item skip behavior.
//! All assertions reference event names from events/logseq_sync_schema.md exactly.

use serde_json::{json, Value};
use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};
use tempfile::TempDir;

fn binary_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_logseq_sync"))
}

fn setup_temp_dir() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("events")).unwrap();
    dir
}

/// Seed pm_structuring + project_state events so logseq_sync can find items.
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

    let items_extracted = json!({
        "event_id": format!("seed-ext-{}", &session_id[..8]),
        "event_type": "ItemsExtracted",
        "timestamp": 1748000001000u64,
        "correlation_id": session_id,
        "source_module": "pm_structuring",
        "payload": { "items": items_json, "item_count": items.len(), "uncertain_count": 0 }
    });
    let extraction_confirmed = json!({
        "event_id": format!("seed-conf-{}", &session_id[..8]),
        "event_type": "ExtractionConfirmed",
        "timestamp": 1748000002000u64,
        "correlation_id": session_id,
        "source_module": "pm_structuring",
        "payload": { "accepted_item_ids": accepted_ids, "accepted_count": items.len() }
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

fn description_to_slug(desc: &str) -> String {
    let lower = desc.to_lowercase();
    let mut slug = String::new();
    let mut last_was_hyphen = false;
    for ch in lower.chars() {
        if ch.is_alphanumeric() { slug.push(ch); last_was_hyphen = false; }
        else if !last_was_hyphen && !slug.is_empty() { slug.push('-'); last_was_hyphen = true; }
    }
    let slug = slug.trim_end_matches('-').to_string();
    if slug.len() <= 120 { slug }
    else {
        let truncated = &slug[..120];
        match truncated.rfind('-') {
            Some(pos) if pos > 0 => truncated[..pos].to_string(),
            _ => truncated.to_string(),
        }
    }
}

/// Write a Logseq page in the canonical R3 format produced by logseq_export.
fn write_logseq_page(
    graph_dir: &str,
    item_id: &str,
    item_type: &str,
    description: &str,
    status: Option<&str>,
    priority: Option<&str>,
) {
    let pages_dir = std::path::Path::new(graph_dir).join("pages");
    fs::create_dir_all(&pages_dir).unwrap();
    let status_val   = status.unwrap_or("not-set");
    let priority_val = priority.unwrap_or("not-set");
    let slug = description_to_slug(description);
    let content = format!(
        "type:: {}\nstatus:: {}\npriority:: {}\ntags:: {}\n\n- item-id: {}\n",
        item_type, status_val, priority_val, item_type, item_id
    );
    fs::write(pages_dir.join(format!("{}.md", slug)), content).unwrap();
}

fn run_binary(dir: &TempDir, graph_dir: &str) -> std::process::Output {
    Command::new(binary_path())
        .current_dir(dir.path())
        .args(["--graph", graph_dir])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("Failed to run binary")
}

/// Read only logseq_sync events from the shared events file.
fn read_ls_events(dir: &TempDir) -> Vec<Value> {
    let path = dir.path().join("events/runtime_events.jsonl");
    if !path.exists() { return vec![]; }
    fs::read_to_string(path).unwrap()
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .filter(|e| e["source_module"].as_str() == Some("logseq_sync"))
        .collect()
}

const SESSION_A:    &str = "a4ca3a7e-61eb-4f36-b59e-f3abd166e351";
const ITEM_TASK:    &str = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeee01";
const ITEM_RISK:    &str = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeee02";
const ITEM_MILESTONE: &str = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeee03";

// ── Happy Path 1: Successful Sync With Changes ────────────────────────────────

#[test]
fn test_sync_with_status_change_emits_requested_updated_completed() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_TASK, "task", "Deploy by Friday")]);
    let graph_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();
    write_logseq_page(&graph_dir, ITEM_TASK, "task", "Deploy by Friday", Some("doing"), None);

    run_binary(&dir, &graph_dir);

    let events = read_ls_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"SyncRequested"),    "SyncRequested must be emitted");
    assert!(types.contains(&"ItemStatusUpdated"),"ItemStatusUpdated must be emitted");
    assert!(types.contains(&"SyncCompleted"),    "SyncCompleted must be emitted");
    assert!(!types.contains(&"SyncCompletedNoChanges"), "SyncCompletedNoChanges must NOT be emitted");

    let req_pos = types.iter().position(|&t| t == "SyncRequested").unwrap();
    let upd_pos = types.iter().position(|&t| t == "ItemStatusUpdated").unwrap();
    let cmp_pos = types.iter().position(|&t| t == "SyncCompleted").unwrap();
    assert!(req_pos < upd_pos, "SyncRequested must precede ItemStatusUpdated");
    assert!(upd_pos < cmp_pos, "ItemStatusUpdated must precede SyncCompleted");
}

#[test]
fn test_sync_status_update_payload_shape() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_TASK, "task", "Deploy by Friday")]);
    let graph_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();
    write_logseq_page(&graph_dir, ITEM_TASK, "task", "Deploy by Friday", Some("doing"), None);

    run_binary(&dir, &graph_dir);

    let events = read_ls_events(&dir);
    let updated = events.iter().find(|e| e["event_type"] == "ItemStatusUpdated")
        .expect("ItemStatusUpdated not found");

    assert_eq!(updated["payload"]["item_id"].as_str().unwrap(),   ITEM_TASK);
    assert_eq!(updated["payload"]["item_type"].as_str().unwrap(), "task");
    assert_eq!(updated["payload"]["new_status"].as_str().unwrap(),"doing");
    assert!(updated["payload"].get("previous_status").is_some(),  "previous_status must be present");
    assert!(updated["payload"]["previous_status"].is_null(),      "previous_status must be null on first sync");
}

#[test]
fn test_sync_priority_change_emits_priority_updated() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_RISK, "risk", "Vendor delay")]);
    let graph_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();
    write_logseq_page(&graph_dir, ITEM_RISK, "risk", "Vendor delay", None, Some("high"));

    run_binary(&dir, &graph_dir);

    let events = read_ls_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"ItemPriorityUpdated"), "ItemPriorityUpdated must be emitted");
    assert!(!types.contains(&"ItemStatusUpdated"),  "ItemStatusUpdated must NOT be emitted when only priority changed");

    let updated = events.iter().find(|e| e["event_type"] == "ItemPriorityUpdated")
        .expect("ItemPriorityUpdated not found");
    assert_eq!(updated["payload"]["item_id"].as_str().unwrap(),       ITEM_RISK);
    assert_eq!(updated["payload"]["item_type"].as_str().unwrap(),     "risk");
    assert_eq!(updated["payload"]["new_priority"].as_str().unwrap(),  "high");
    assert!(updated["payload"].get("previous_priority").is_some(),    "previous_priority must be present");
    assert!(updated["payload"]["previous_priority"].is_null(),        "previous_priority must be null on first sync");
}

#[test]
fn test_sync_both_status_and_priority_change_counts_in_completed() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_TASK, "task", "Deploy by Friday")]);
    let graph_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();
    write_logseq_page(&graph_dir, ITEM_TASK, "task", "Deploy by Friday", Some("todo"), Some("high"));

    run_binary(&dir, &graph_dir);

    let events = read_ls_events(&dir);
    let completed = events.iter().find(|e| e["event_type"] == "SyncCompleted")
        .expect("SyncCompleted not found");

    assert!(completed["payload"]["graph_dir"].as_str().is_some(), "graph_dir must be present");
    assert_eq!(completed["payload"]["changes_applied"].as_u64().unwrap(), 2,
        "changes_applied must count status + priority as two changes");
    assert_eq!(completed["payload"]["items_skipped"].as_u64().unwrap(), 0);
}

#[test]
fn test_sync_previous_status_reflects_prior_value_on_second_change() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_TASK, "task", "Deploy by Friday")]);
    let graph_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();

    write_logseq_page(&graph_dir, ITEM_TASK, "task", "Deploy by Friday", Some("todo"), None);
    run_binary(&dir, &graph_dir);

    write_logseq_page(&graph_dir, ITEM_TASK, "task", "Deploy by Friday", Some("doing"), None);
    run_binary(&dir, &graph_dir);

    let events = read_ls_events(&dir);
    let updated_events: Vec<&Value> = events.iter()
        .filter(|e| e["event_type"] == "ItemStatusUpdated")
        .collect();

    assert_eq!(updated_events.len(), 2, "Two ItemStatusUpdated events must be emitted across two syncs");
    assert_eq!(
        updated_events[1]["payload"]["previous_status"].as_str().unwrap(), "todo",
        "previous_status on second sync must equal the first sync's new_status"
    );
}

#[test]
fn test_sync_only_changed_items_emit_update_events() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[
        (ITEM_TASK, "task", "Deploy by Friday"),
        (ITEM_RISK, "risk", "Vendor delay"),
    ]);
    let graph_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();
    write_logseq_page(&graph_dir, ITEM_TASK, "task", "Deploy by Friday", Some("todo"), None);
    write_logseq_page(&graph_dir, ITEM_RISK, "risk", "Vendor delay", None, None); // no change

    run_binary(&dir, &graph_dir);

    let events = read_ls_events(&dir);
    let updated_events: Vec<&Value> = events.iter()
        .filter(|e| e["event_type"] == "ItemStatusUpdated")
        .collect();

    assert_eq!(updated_events.len(), 1, "Only the changed item must emit ItemStatusUpdated");
    assert_eq!(updated_events[0]["payload"]["item_id"].as_str().unwrap(), ITEM_TASK);
}

// ── Happy Path 2: Sync With No Changes ───────────────────────────────────────

#[test]
fn test_sync_no_changes_emits_sync_completed_no_changes() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_TASK, "task", "Deploy by Friday")]);
    let graph_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();
    // Page shows not-set; effective status is also null → no difference
    write_logseq_page(&graph_dir, ITEM_TASK, "task", "Deploy by Friday", None, None);

    run_binary(&dir, &graph_dir);

    let events = read_ls_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"SyncRequested"),          "SyncRequested must be emitted");
    assert!(types.contains(&"SyncCompletedNoChanges"), "SyncCompletedNoChanges must be emitted");
    assert!(!types.contains(&"ItemStatusUpdated"),      "ItemStatusUpdated must NOT be emitted");
    assert!(!types.contains(&"ItemPriorityUpdated"),    "ItemPriorityUpdated must NOT be emitted");
    assert!(!types.contains(&"SyncCompleted"),          "SyncCompleted must NOT be emitted");
}

#[test]
fn test_sync_no_changes_payload_shape() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[
        (ITEM_TASK, "task", "Deploy by Friday"),
        (ITEM_RISK, "risk", "Vendor delay"),
    ]);
    let graph_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();
    write_logseq_page(&graph_dir, ITEM_TASK, "task", "Deploy by Friday", None, None);
    write_logseq_page(&graph_dir, ITEM_RISK, "risk", "Vendor delay",     None, None);

    run_binary(&dir, &graph_dir);

    let events = read_ls_events(&dir);
    let no_changes = events.iter().find(|e| e["event_type"] == "SyncCompletedNoChanges")
        .expect("SyncCompletedNoChanges not found");

    assert!(no_changes["payload"]["graph_dir"].as_str().is_some(), "graph_dir must be present");
    assert_eq!(no_changes["payload"]["items_checked"].as_u64().unwrap(), 2,
        "items_checked must equal the number of items in the project record");
}

#[test]
fn test_second_sync_with_unchanged_pages_emits_no_changes() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_TASK, "task", "Deploy by Friday")]);
    let graph_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();
    write_logseq_page(&graph_dir, ITEM_TASK, "task", "Deploy by Friday", Some("todo"), None);

    run_binary(&dir, &graph_dir); // first run — applies change
    run_binary(&dir, &graph_dir); // second run — page unchanged

    let events = read_ls_events(&dir);
    let no_changes_count = events.iter()
        .filter(|e| e["event_type"] == "SyncCompletedNoChanges")
        .count();
    let update_count = events.iter()
        .filter(|e| e["event_type"] == "ItemStatusUpdated")
        .count();

    assert_eq!(no_changes_count, 1, "Second run must emit exactly one SyncCompletedNoChanges");
    assert_eq!(update_count, 1,     "ItemStatusUpdated must appear exactly once (first run only)");
}

// ── Failure Path 1: GraphNotAccessible ───────────────────────────────────────

#[test]
fn test_graph_not_accessible_emits_failure_event() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_TASK, "task", "Deploy by Friday")]);
    let graph_dir = dir.path().join("nonexistent_graph").to_string_lossy().into_owned();

    run_binary(&dir, &graph_dir);

    let events = read_ls_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"SyncRequested"),                "SyncRequested must be emitted");
    assert!(types.contains(&"SyncFailedGraphNotAccessible"), "SyncFailedGraphNotAccessible must be emitted");
    assert!(!types.contains(&"SyncCompleted"),               "SyncCompleted must NOT be emitted");
    assert!(!types.contains(&"SyncCompletedNoChanges"),      "SyncCompletedNoChanges must NOT be emitted");
    assert!(!types.contains(&"ItemStatusUpdated"),           "ItemStatusUpdated must NOT be emitted");
}

#[test]
fn test_graph_not_accessible_failure_payload() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_TASK, "task", "Deploy by Friday")]);
    let graph_dir = dir.path().join("nonexistent_graph").to_string_lossy().into_owned();

    run_binary(&dir, &graph_dir);

    let events = read_ls_events(&dir);
    let failure = events.iter().find(|e| e["event_type"] == "SyncFailedGraphNotAccessible")
        .expect("SyncFailedGraphNotAccessible not found");

    assert_eq!(failure["payload"]["failure_reason"].as_str().unwrap(), "graph_not_accessible");
    assert!(failure["payload"]["graph_dir"].as_str().is_some(), "graph_dir must be present in payload");
}

#[test]
fn test_graph_not_accessible_requested_precedes_failure() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_TASK, "task", "Deploy by Friday")]);
    let graph_dir = dir.path().join("nonexistent_graph").to_string_lossy().into_owned();

    run_binary(&dir, &graph_dir);

    let events = read_ls_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    let req_pos  = types.iter().position(|&t| t == "SyncRequested").unwrap();
    let fail_pos = types.iter().position(|&t| t == "SyncFailedGraphNotAccessible").unwrap();
    assert!(req_pos < fail_pos, "SyncRequested must precede SyncFailedGraphNotAccessible");
}

// ── Failure Path 2: ProjectRecordEmpty ───────────────────────────────────────

#[test]
fn test_empty_record_emits_failure_event() {
    let dir = setup_temp_dir();
    // No items seeded — project record is empty
    let graph_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();
    fs::create_dir_all(format!("{}/pages", graph_dir)).unwrap();

    run_binary(&dir, &graph_dir);

    let events = read_ls_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"SyncRequested"),         "SyncRequested must be emitted");
    assert!(types.contains(&"SyncFailedEmptyRecord"), "SyncFailedEmptyRecord must be emitted");
    assert!(!types.contains(&"SyncCompleted"),         "SyncCompleted must NOT be emitted");
}

#[test]
fn test_empty_record_failure_reason() {
    let dir = setup_temp_dir();
    let graph_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();
    fs::create_dir_all(format!("{}/pages", graph_dir)).unwrap();

    run_binary(&dir, &graph_dir);

    let events = read_ls_events(&dir);
    let failure = events.iter().find(|e| e["event_type"] == "SyncFailedEmptyRecord")
        .expect("SyncFailedEmptyRecord not found");

    assert_eq!(failure["payload"]["failure_reason"].as_str().unwrap(), "empty_project_record");
}

// ── Failure Path 3: InvalidStatusForType ─────────────────────────────────────

#[test]
fn test_invalid_status_for_type_emits_skip_event() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_MILESTONE, "milestone", "Q2 launch")]);
    let graph_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();
    // "doing" is valid for task but NOT for milestone
    write_logseq_page(&graph_dir, ITEM_MILESTONE, "milestone", "Q2 launch", Some("doing"), None);

    run_binary(&dir, &graph_dir);

    let events = read_ls_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"ItemSyncSkippedInvalidStatus"), "ItemSyncSkippedInvalidStatus must be emitted");
    assert!(!types.contains(&"ItemStatusUpdated"),           "ItemStatusUpdated must NOT be emitted for invalid status");
    assert!(types.contains(&"SyncCompleted"),               "SyncCompleted must be emitted — sync continues");
    assert!(!types.contains(&"SyncCompletedNoChanges"),     "SyncCompletedNoChanges must NOT be emitted when differences detected");
}

#[test]
fn test_invalid_status_skip_event_payload() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_MILESTONE, "milestone", "Q2 launch")]);
    let graph_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();
    write_logseq_page(&graph_dir, ITEM_MILESTONE, "milestone", "Q2 launch", Some("doing"), None);

    run_binary(&dir, &graph_dir);

    let events = read_ls_events(&dir);
    let skipped = events.iter().find(|e| e["event_type"] == "ItemSyncSkippedInvalidStatus")
        .expect("ItemSyncSkippedInvalidStatus not found");

    assert_eq!(skipped["payload"]["failure_reason"].as_str().unwrap(),  "invalid_status_for_type");
    assert_eq!(skipped["payload"]["item_id"].as_str().unwrap(),         ITEM_MILESTONE);
    assert_eq!(skipped["payload"]["item_type"].as_str().unwrap(),       "milestone");
    assert_eq!(skipped["payload"]["rejected_status"].as_str().unwrap(), "doing");
}

#[test]
fn test_invalid_status_does_not_block_valid_items_in_same_run() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[
        (ITEM_MILESTONE, "milestone", "Q2 launch"),
        (ITEM_TASK,      "task",      "Deploy by Friday"),
    ]);
    let graph_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();
    write_logseq_page(&graph_dir, ITEM_MILESTONE, "milestone", "Q2 launch", Some("doing"), None); // invalid
    write_logseq_page(&graph_dir, ITEM_TASK,      "task",      "Deploy by Friday", Some("todo"), None); // valid

    run_binary(&dir, &graph_dir);

    let events = read_ls_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"ItemSyncSkippedInvalidStatus"), "skip event must be emitted for invalid item");
    assert!(types.contains(&"ItemStatusUpdated"),            "ItemStatusUpdated must be emitted for valid item");

    let updated = events.iter().find(|e| e["event_type"] == "ItemStatusUpdated")
        .expect("ItemStatusUpdated not found");
    assert_eq!(updated["payload"]["item_id"].as_str().unwrap(), ITEM_TASK,
        "ItemStatusUpdated must be for the valid item, not the skipped one");
}

#[test]
fn test_sync_completed_reports_skipped_count() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_MILESTONE, "milestone", "Q2 launch")]);
    let graph_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();
    write_logseq_page(&graph_dir, ITEM_MILESTONE, "milestone", "Q2 launch", Some("doing"), None);

    run_binary(&dir, &graph_dir);

    let events = read_ls_events(&dir);
    let completed = events.iter().find(|e| e["event_type"] == "SyncCompleted")
        .expect("SyncCompleted not found");

    assert_eq!(completed["payload"]["changes_applied"].as_u64().unwrap(), 0,
        "changes_applied must be 0 when all detected changes were invalid");
    assert_eq!(completed["payload"]["items_skipped"].as_u64().unwrap(), 1,
        "items_skipped must reflect the number of skipped items");
}

#[test]
fn test_invalid_status_item_status_not_updated_in_record() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_MILESTONE, "milestone", "Q2 launch")]);
    let graph_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();
    write_logseq_page(&graph_dir, ITEM_MILESTONE, "milestone", "Q2 launch", Some("doing"), None);

    run_binary(&dir, &graph_dir);

    // Verify: no ItemStatusUpdated in the entire event log for this item
    let path = dir.path().join("events/runtime_events.jsonl");
    let all_events: Vec<Value> = fs::read_to_string(path).unwrap()
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();

    let status_updates_for_item: Vec<&Value> = all_events.iter()
        .filter(|e| {
            e["event_type"].as_str() == Some("ItemStatusUpdated")
                && e["payload"]["item_id"].as_str() == Some(ITEM_MILESTONE)
        })
        .collect();

    assert!(status_updates_for_item.is_empty(),
        "No ItemStatusUpdated must exist for the skipped item");
}

// ── Invariant: unrecognised pages silently ignored ────────────────────────────

#[test]
fn test_pages_without_item_in_record_are_silently_ignored() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_TASK, "task", "Deploy by Friday")]);
    let graph_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();
    // Write a page whose name does not match any project record item_id
    let pages_dir = std::path::Path::new(&graph_dir).join("pages");
    fs::create_dir_all(&pages_dir).unwrap();
    fs::write(
        pages_dir.join("random-unknown-page.md"),
        "status:: doing\npriority:: high\n",
    ).unwrap();
    // No page for ITEM_TASK — nothing to sync

    run_binary(&dir, &graph_dir);

    let events = read_ls_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();
    assert!(!types.contains(&"ItemStatusUpdated"),
        "Unknown page must not produce ItemStatusUpdated");
    assert!(types.contains(&"SyncCompletedNoChanges"),
        "SyncCompletedNoChanges must be emitted when no known item pages differ");
}

// ── Cross-module integration ──────────────────────────────────────────────────

fn item_status_binary_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../item_status/target/debug/item_status")
}

/// Append a logseq_sync-originated ItemStatusUpdated event directly to the log.
fn seed_logseq_sync_status_update(dir: &TempDir, item_id: &str, item_type: &str, new_status: &str) {
    let event = json!({
        "event_id": "cross-module-test-su",
        "event_type": "ItemStatusUpdated",
        "timestamp": 1748300001000u64,
        "correlation_id": "cross-module-test-cid-0001",
        "source_module": "logseq_sync",
        "payload": {
            "item_id": item_id,
            "item_type": item_type,
            "new_status": new_status,
            "previous_status": null
        }
    });
    let path = dir.path().join("events/runtime_events.jsonl");
    let mut file = fs::OpenOptions::new().create(true).append(true).open(&path).unwrap();
    writeln!(file, "{}", event).unwrap();
}

#[test]
fn test_item_status_module_reads_logseq_sync_originated_status_update() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_TASK, "task", "Deploy by Friday")]);
    seed_logseq_sync_status_update(&dir, ITEM_TASK, "task", "doing");

    let output = Command::new(item_status_binary_path())
        .current_dir(dir.path())
        .args(["get", ITEM_TASK])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("Failed to run item_status binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("doing"),
        "item_status get must reflect the status written by logseq_sync; got: {stdout}"
    );
}

// ── Telemetry ─────────────────────────────────────────────────────────────────

#[test]
fn test_all_events_have_required_base_fields() {
    let dir = setup_temp_dir();
    // Empty record path — reliable, no external deps
    let graph_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();
    fs::create_dir_all(format!("{}/pages", graph_dir)).unwrap();

    run_binary(&dir, &graph_dir);

    let events = read_ls_events(&dir);
    assert!(!events.is_empty(), "At least one event must be emitted");

    for event in &events {
        let t = event["event_type"].as_str().unwrap_or("unknown");
        assert!(!event["event_id"].is_null(),       "{}: event_id must be present", t);
        assert!(!event["event_type"].is_null(),     "{}: event_type must be present", t);
        assert!(!event["timestamp"].is_null(),      "{}: timestamp must be present", t);
        assert!(!event["correlation_id"].is_null(), "{}: correlation_id must be present", t);
        assert!(!event["source_module"].is_null(),  "{}: source_module must be present", t);
        assert!(!event["payload"].is_null(),        "{}: payload must be present", t);
        assert_eq!(event["source_module"].as_str().unwrap(), "logseq_sync",
            "{}: source_module must be 'logseq_sync'", t);
        assert!(event["timestamp"].as_u64().unwrap() > 0,
            "{}: timestamp must be positive", t);
    }
}

#[test]
fn test_correlation_id_consistent_within_one_invocation() {
    let dir = setup_temp_dir();
    let graph_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();
    fs::create_dir_all(format!("{}/pages", graph_dir)).unwrap();

    run_binary(&dir, &graph_dir);

    let events = read_ls_events(&dir);
    assert!(events.len() >= 2, "At least SyncRequested and a terminal event must be emitted");

    let first_cid = events[0]["correlation_id"].as_str().unwrap();
    for event in &events {
        assert_eq!(event["correlation_id"].as_str().unwrap(), first_cid,
            "All events from one invocation must share the same correlation_id");
    }
}

#[test]
fn test_separate_invocations_have_different_correlation_ids() {
    let dir = setup_temp_dir();
    let graph_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();
    fs::create_dir_all(format!("{}/pages", graph_dir)).unwrap();

    run_binary(&dir, &graph_dir);
    run_binary(&dir, &graph_dir);

    let events = read_ls_events(&dir);
    let cids: Vec<&str> = events.iter()
        .filter(|e| e["event_type"] == "SyncRequested")
        .map(|e| e["correlation_id"].as_str().unwrap())
        .collect();

    assert_eq!(cids.len(), 2);
    assert_ne!(cids[0], cids[1],
        "Different invocations must produce different correlation_ids");
}

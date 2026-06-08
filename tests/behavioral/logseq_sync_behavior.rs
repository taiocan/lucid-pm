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

// Minimal default vocabulary matching the installed ~/.lucidpm/default-schema.yaml.
// Makes tests portable and provides the backward-compatibility regression baseline:
// all status values valid under the pre-R9 hardcoded table must remain valid here.
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

const SESSION_A:      &str = "a4ca3a7e-61eb-4f36-b59e-f3abd166e351";
const ITEM_TASK:      &str = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeee01";
const ITEM_RISK:      &str = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeee02";
const ITEM_MILESTONE: &str = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeee03";
const ITEM_CUSTOM:    &str = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeee04";

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

// ── R9: Schema Load Failure (FP1) ─────────────────────────────────────────────

#[test]
fn test_schema_load_failed_emits_sync_requested_then_schema_invalid() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_TASK, "task", "Deploy by Friday")]);
    let graph_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();
    fs::create_dir_all(format!("{}/pages", graph_dir)).unwrap();
    write_logseq_page(&graph_dir, ITEM_TASK, "task", "Deploy by Friday", Some("todo"), None);
    // Overwrite schema with invalid YAML
    write_project_schema(&dir, "this: is: not: valid: yaml: [[[");

    run_binary(&dir, &graph_dir);

    let events = read_ls_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"SyncRequested"),          "SyncRequested must be emitted");
    assert!(types.contains(&"SyncFailedSchemaInvalid"),"SyncFailedSchemaInvalid must be emitted");
    assert!(!types.contains(&"SyncCompleted"),          "SyncCompleted must NOT be emitted");
    assert!(!types.contains(&"SyncCompletedNoChanges"), "SyncCompletedNoChanges must NOT be emitted");
    assert!(!types.contains(&"ItemStatusUpdated"),      "No Logseq pages read — ItemStatusUpdated must NOT be emitted");
}

#[test]
fn test_schema_load_failed_payload_shape() {
    let dir = setup_temp_dir();
    let graph_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();
    fs::create_dir_all(format!("{}/pages", graph_dir)).unwrap();
    write_project_schema(&dir, "this: is: not: valid: yaml: [[[");

    run_binary(&dir, &graph_dir);

    let events = read_ls_events(&dir);
    let failure = events.iter().find(|e| e["event_type"] == "SyncFailedSchemaInvalid")
        .expect("SyncFailedSchemaInvalid not found");

    assert_eq!(failure["payload"]["failure_reason"].as_str().unwrap(), "schema_load_failed");
}

#[test]
fn test_schema_load_failed_requested_precedes_failure() {
    let dir = setup_temp_dir();
    let graph_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();
    fs::create_dir_all(format!("{}/pages", graph_dir)).unwrap();
    write_project_schema(&dir, "this: is: not: valid: yaml: [[[");

    run_binary(&dir, &graph_dir);

    let events = read_ls_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    let req_pos  = types.iter().position(|&t| t == "SyncRequested").unwrap();
    let fail_pos = types.iter().position(|&t| t == "SyncFailedSchemaInvalid").unwrap();
    assert!(req_pos < fail_pos, "SyncRequested must precede SyncFailedSchemaInvalid");
}

// ── R9: Custom Vocabulary Accepted (HP1) ──────────────────────────────────────

#[test]
fn test_custom_vocabulary_status_accepted_as_valid() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, r#"schemaVersion: 1
statuses:
  reviewing:
pageTypes:
  Inspector:
    allowedStatuses: [reviewing]
"#);
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_CUSTOM, "Inspector", "Audit checklist")]);
    let graph_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();
    write_logseq_page(&graph_dir, ITEM_CUSTOM, "Inspector", "Audit checklist", Some("reviewing"), None);

    run_binary(&dir, &graph_dir);

    let events = read_ls_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"ItemStatusUpdated"),           "Custom vocabulary status must be accepted");
    assert!(!types.contains(&"ItemSyncSkippedInvalidStatus"), "No skip event for valid custom status");

    let updated = events.iter().find(|e| e["event_type"] == "ItemStatusUpdated").unwrap();
    assert_eq!(updated["payload"]["item_id"].as_str().unwrap(),  ITEM_CUSTOM);
    assert_eq!(updated["payload"]["new_status"].as_str().unwrap(), "reviewing");
}

// ── Invariant Falsification ───────────────────────────────────────────────────

// IF-1: No hardcoded status table consulted
// Fixture: custom status "reviewing" absent from any hardcoded table.
// Wrong assumption: hardcoded table consulted → "reviewing" not found → rejected.
// Correct: vocabulary defines "reviewing" for "Inspector" → accepted.
#[test]
fn test_vocabulary_defined_set_falsifies_hardcoded_table() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, r#"schemaVersion: 1
statuses:
  reviewing:
pageTypes:
  Inspector:
    allowedStatuses: [reviewing]
"#);
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_CUSTOM, "Inspector", "Audit checklist")]);
    let graph_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();
    write_logseq_page(&graph_dir, ITEM_CUSTOM, "Inspector", "Audit checklist", Some("reviewing"), None);

    run_binary(&dir, &graph_dir);

    let events = read_ls_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();
    assert!(types.contains(&"ItemStatusUpdated"),             "IF-1: custom status must be accepted via vocabulary");
    assert!(!types.contains(&"ItemSyncSkippedInvalidStatus"), "IF-1: no skip — hardcoded table must not be consulted");
}

// IF-2: No entity type name is a hardcode special case
// Fixture: completely new type "Inspector" not present in any hardcoded branch.
// Wrong assumption: match/branch on known type names → "Inspector" falls through → empty status set.
// Correct: vocabulary defines "Inspector" with statuses → "scheduled" accepted.
#[test]
fn test_unknown_type_name_falsifies_hardcoded_type_branching() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, r#"schemaVersion: 1
statuses:
  scheduled:
  done:
pageTypes:
  Inspector:
    allowedStatuses: [scheduled, done]
"#);
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_CUSTOM, "Inspector", "Site visit")]);
    let graph_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();
    write_logseq_page(&graph_dir, ITEM_CUSTOM, "Inspector", "Site visit", Some("scheduled"), None);

    run_binary(&dir, &graph_dir);

    let events = read_ls_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();
    assert!(types.contains(&"ItemStatusUpdated"),             "IF-2: unknown type must be handled via vocabulary");
    assert!(!types.contains(&"ItemSyncSkippedInvalidStatus"), "IF-2: no skip — type name must not gate validation");
}

// IF-3: Alias resolves to same status set as canonical (acceptance path)
// Fixture: canonical "Risk", alias "risk"; status "identified" defined for "Risk" concept.
// Item stored with type "risk" (alias form).
// Wrong assumption: page_types.get("risk") → None → false → "identified" rejected.
// Correct: resolve("risk") → "Risk", page_types.get("Risk") → allowedStatuses contains "identified" → accepted.
#[test]
fn test_alias_acceptance_falsifies_string_comparison() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, r#"schemaVersion: 1
statuses:
  identified:
pageTypes:
  Risk:
    allowedStatuses: [identified]
    aliases: [risk]
"#);
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_RISK, "risk", "Supply chain risk")]);
    let graph_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();
    write_logseq_page(&graph_dir, ITEM_RISK, "risk", "Supply chain risk", Some("identified"), None);

    run_binary(&dir, &graph_dir);

    let events = read_ls_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();
    assert!(types.contains(&"ItemStatusUpdated"),             "IF-3: alias item must accept status via canonical resolution");
    assert!(!types.contains(&"ItemSyncSkippedInvalidStatus"), "IF-3: no skip — alias resolved to canonical status set");
}

// IF-4: Alias rejection path is consistent with canonical rejection
// Fixture: same vocabulary; item stored as "risk"; Logseq shows "closed" (not in "Risk" set).
// Wrong assumption: alias resolution applied only on acceptance path → incorrect outcome on rejection.
// Correct: resolve("risk") → "Risk" → "closed" not in ["identified"] → ItemSyncSkippedInvalidStatus.
#[test]
fn test_alias_rejection_falsifies_acceptance_only_resolution() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, r#"schemaVersion: 1
statuses:
  identified:
pageTypes:
  Risk:
    allowedStatuses: [identified]
    aliases: [risk]
"#);
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_RISK, "risk", "Supply chain risk")]);
    let graph_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();
    // "closed" is not in the "Risk" concept's allowed statuses
    write_logseq_page(&graph_dir, ITEM_RISK, "risk", "Supply chain risk", Some("closed"), None);

    run_binary(&dir, &graph_dir);

    let events = read_ls_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();
    assert!(types.contains(&"ItemSyncSkippedInvalidStatus"), "IF-4: alias item must be rejected via canonical resolution");
    assert!(!types.contains(&"ItemStatusUpdated"),           "IF-4: no status update for rejected item");

    let skipped = events.iter().find(|e| e["event_type"] == "ItemSyncSkippedInvalidStatus").unwrap();
    assert_eq!(skipped["payload"]["rejected_status"].as_str().unwrap(), "closed");
}

// IF-5: Representation Ban — canonical-casing fixture
// Fixture: canonical "Risk" (uppercase R), alias "risk" (lowercase); status "open" valid for "Risk".
// Item type stored as "risk" (lowercase alias). Page shows "open".
// Wrong assumption: page_types.get("risk") → None (key is "Risk") → false → spurious skip.
// Correct: resolve_type("risk") → "Risk", page_types.get("Risk") → "open" in allowedStatuses → accepted.
#[test]
fn test_representation_ban_falsifies_direct_string_comparison() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, r#"schemaVersion: 1
statuses:
  open:
pageTypes:
  Risk:
    allowedStatuses: [open]
    aliases: [risk]
"#);
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_RISK, "risk", "Vendor risk")]);
    let graph_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();
    write_logseq_page(&graph_dir, ITEM_RISK, "risk", "Vendor risk", Some("open"), None);

    run_binary(&dir, &graph_dir);

    let events = read_ls_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();
    assert!(types.contains(&"ItemStatusUpdated"),             "IF-5: representation ban — stored alias must resolve correctly");
    assert!(!types.contains(&"ItemSyncSkippedInvalidStatus"), "IF-5: no spurious skip due to casing mismatch");
}

// IF-6: Default vocabulary preserves pre-R9 behavior (backward compatibility regression)
// Fixture: DEFAULT_SCHEMA matches the pre-R9 hardcoded table exactly.
// "todo" was valid for "task" before R9; must remain valid with vocabulary-driven validation.
// Wrong assumption: default vocabulary differs from old table → previously valid status rejected.
#[test]
fn test_default_vocabulary_preserves_pre_r9_behavior() {
    // setup_temp_dir() writes DEFAULT_SCHEMA which mirrors the old hardcoded table
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_TASK, "task", "Deploy by Friday")]);
    let graph_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();
    // "todo" was valid for "task" in the old hardcoded table
    write_logseq_page(&graph_dir, ITEM_TASK, "task", "Deploy by Friday", Some("todo"), None);

    run_binary(&dir, &graph_dir);

    let events = read_ls_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();
    assert!(types.contains(&"ItemStatusUpdated"),             "IF-6: pre-R9 valid status must still be accepted");
    assert!(!types.contains(&"ItemSyncSkippedInvalidStatus"), "IF-6: no regression — backward compat preserved");
}

// ── R9 Stage 9 Refinement: empty allowedStatuses edge case ───────────────────

// Contract FP2 note: "this applies equally when the entity type concept's
// vocabulary-defined status set is empty — any Logseq status for an item of
// that type triggers ItemSyncSkippedInvalidStatus."
#[test]
fn test_empty_allowed_statuses_rejects_any_status() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, r#"schemaVersion: 1
pageTypes:
  Classified:
    allowedStatuses: []
"#);
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_CUSTOM, "Classified", "Restricted item")]);
    let graph_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();
    write_logseq_page(&graph_dir, ITEM_CUSTOM, "Classified", "Restricted item", Some("open"), None);

    run_binary(&dir, &graph_dir);

    let events = read_ls_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();
    assert!(types.contains(&"ItemSyncSkippedInvalidStatus"),
        "Type with empty allowedStatuses must reject any Logseq status");
    assert!(!types.contains(&"ItemStatusUpdated"),
        "No status update for type with empty allowedStatuses");
}

// ── R_export_format: new task block format (logseq_sync) ─────────────────────

const BLOCK_SCHEMA: &str = r#"schemaVersion: 1
statuses:
  todo:
  doing:
  done:
  active:
pageTypes:
  WorkPackage:
    allowedStatuses: [todo, doing, done]
    aliases: [work_package]
  Stakeholder:
    allowedStatuses: [active]
    aliases: [stakeholder]
blockTypes:
  task_block:
    markers:
      TODO: todo
      DOING: doing
      DONE: done
"#;

const TASK_ID_S1: &str = "bbbbbbbb-1111-2222-3333-444444444444";
const ITEM_WP_S:  &str = "cccccccc-1111-2222-3333-444444444444";
const ITEM_SH_S:  &str = "dddddddd-1111-2222-3333-444444444444";

fn setup_dir_with_block_schema() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("events")).unwrap();
    write_project_schema(&dir, BLOCK_SCHEMA);
    dir
}

fn run_binary_isolated(dir: &TempDir, graph_dir: &str) -> std::process::Output {
    Command::new(binary_path())
        .current_dir(dir.path())
        .args(["--graph", graph_dir])
        .env("HOME", dir.path())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("Failed to run binary")
}

fn seed_task_added_s(
    dir: &TempDir,
    task_id: &str,
    parent_id: &str,
    marker: &str,
    owner_id: &str,
    scheduled_date: Option<&str>,
    deadline: Option<&str>,
) {
    let event = json!({
        "event_id":       format!("seed-task-{}", &task_id[..8]),
        "event_type":     "TaskAdded",
        "timestamp":      1748000010000u64,
        "correlation_id": "00000000-0000-0000-0000-000000000099",
        "source_module":  "task_model",
        "payload": {
            "task_id":        task_id,
            "item_type":      "task_block",
            "description":    "Review auth checklist",
            "parent_item_id": parent_id,
            "initial_marker": marker,
            "owner_id":       owner_id,
            "scheduled_date": scheduled_date,
            "deadline":       deadline,
        }
    });
    let path = dir.path().join("events/runtime_events.jsonl");
    let mut file = fs::OpenOptions::new().create(true).append(true).open(&path).unwrap();
    writeln!(file, "{}", event).unwrap();
}

fn write_page_with_task_new_format(
    graph_dir: &str,
    parent_id: &str,
    parent_slug: &str,
    task_id: &str,
    marker: &str,
    description: &str,
    owner_ref: &str,
    scheduled: Option<&str>,
    deadline: Option<&str>,
) {
    let pages_dir = std::path::Path::new(graph_dir).join("pages");
    fs::create_dir_all(&pages_dir).unwrap();
    let mut content = format!(
        "type:: work_package\nstatus:: not-set\npriority:: not-set\ntags:: work_package\n\n- item-id: {}\n",
        parent_id
    );
    content.push_str(&format!("\n- {} {} [[{}]]\n", marker, description, owner_ref));
    content.push_str(&format!("  :PROPERTIES:\n  :task-id: {}\n  :END:\n", task_id));
    if let Some(s) = scheduled {
        content.push_str(&format!("  SCHEDULED: {}\n", s));
    }
    if let Some(d) = deadline {
        content.push_str(&format!("  DEADLINE: {}\n", d));
    }
    fs::write(pages_dir.join(format!("{}.md", parent_slug)), content).unwrap();
}

fn read_task_sync_events(dir: &TempDir) -> Vec<Value> {
    let path = dir.path().join("events/runtime_events.jsonl");
    if !path.exists() { return vec![]; }
    fs::read_to_string(path).unwrap()
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .filter(|e| {
            let et = e["event_type"].as_str().unwrap_or("");
            et == "TaskMarkerUpdated" || et == "TaskOwnerUpdated" || et == "TaskDatesUpdated"
        })
        .collect()
}

#[test]
fn test_sync_new_format_task_marker_change_emits_task_marker_updated() {
    let dir = setup_dir_with_block_schema();
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_WP_S, "work_package", "Sprint Alpha")]);
    seed_task_added_s(&dir, TASK_ID_S1, ITEM_WP_S, "TODO", "TBD", None, None);
    let graph_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();
    write_page_with_task_new_format(
        &graph_dir, ITEM_WP_S, "sprint-alpha", TASK_ID_S1,
        "DOING", "Review auth checklist", "TBD",
        None, None,
    );

    run_binary_isolated(&dir, &graph_dir);

    let events = read_task_sync_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();
    assert!(
        types.contains(&"TaskMarkerUpdated"),
        "New-format task block: DOING in page vs TODO in record must emit TaskMarkerUpdated"
    );
    let upd = events.iter().find(|e| e["event_type"] == "TaskMarkerUpdated").unwrap();
    assert_eq!(upd["payload"]["task_id"].as_str().unwrap(), TASK_ID_S1);
    assert_eq!(upd["payload"]["new_marker"].as_str().unwrap(), "DOING");
}

#[test]
fn test_sync_new_format_task_owner_change_emits_task_owner_updated() {
    let dir = setup_dir_with_block_schema();
    seed_incorporated_items(&dir, SESSION_A, &[
        (ITEM_WP_S, "work_package", "Sprint Alpha"),
        (ITEM_SH_S, "stakeholder", "Alice Stakeholder"),
    ]);
    seed_task_added_s(&dir, TASK_ID_S1, ITEM_WP_S, "TODO", "TBD", None, None);
    let graph_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();
    write_page_with_task_new_format(
        &graph_dir, ITEM_WP_S, "sprint-alpha", TASK_ID_S1,
        "TODO", "Review auth checklist", "alice-stakeholder",
        None, None,
    );
    // Stakeholder page must exist so page_name_to_item can resolve [[alice-stakeholder]]
    let pages_dir = std::path::Path::new(&graph_dir).join("pages");
    fs::write(
        pages_dir.join("alice-stakeholder.md"),
        format!("type:: stakeholder\n\n- item-id: {}\n", ITEM_SH_S),
    ).unwrap();

    run_binary_isolated(&dir, &graph_dir);

    let events = read_task_sync_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();
    assert!(
        types.contains(&"TaskOwnerUpdated"),
        "New-format task block: [[alice-stakeholder]] vs TBD must emit TaskOwnerUpdated"
    );
    let upd = events.iter().find(|e| e["event_type"] == "TaskOwnerUpdated").unwrap();
    assert_eq!(upd["payload"]["task_id"].as_str().unwrap(), TASK_ID_S1);
    assert_eq!(upd["payload"]["new_owner_id"].as_str().unwrap(), ITEM_SH_S);
}

#[test]
fn test_sync_new_format_task_dates_change_emits_task_dates_updated() {
    let dir = setup_dir_with_block_schema();
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_WP_S, "work_package", "Sprint Alpha")]);
    seed_task_added_s(&dir, TASK_ID_S1, ITEM_WP_S, "TODO", "TBD", None, None);
    let graph_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();
    // <2026-06-20 Sat> — parse_logseq_date must extract "2026-06-20"
    write_page_with_task_new_format(
        &graph_dir, ITEM_WP_S, "sprint-alpha", TASK_ID_S1,
        "TODO", "Review auth checklist", "TBD",
        Some("<2026-06-20 Sat>"),
        None,
    );

    run_binary_isolated(&dir, &graph_dir);

    let events = read_task_sync_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();
    assert!(
        types.contains(&"TaskDatesUpdated"),
        "New-format task block: SCHEDULED line must emit TaskDatesUpdated"
    );
    let upd = events.iter().find(|e| e["event_type"] == "TaskDatesUpdated").unwrap();
    assert_eq!(upd["payload"]["task_id"].as_str().unwrap(), TASK_ID_S1);
    assert_eq!(upd["payload"]["new_scheduled_date"].as_str().unwrap(), "2026-06-20");
}

#[test]
fn test_sync_old_format_task_still_parseable() {
    let dir = setup_dir_with_block_schema();
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_WP_S, "work_package", "Sprint Alpha")]);
    seed_task_added_s(&dir, TASK_ID_S1, ITEM_WP_S, "TODO", "TBD", None, None);
    let graph_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();

    // Old format: inline task-id in the bullet line
    let pages_dir = std::path::Path::new(&graph_dir).join("pages");
    fs::create_dir_all(&pages_dir).unwrap();
    let content = format!(
        "type:: work_package\nstatus:: not-set\npriority:: not-set\ntags:: work_package\n\n- item-id: {}\n\n- DOING task-id: {} Review auth checklist\n",
        ITEM_WP_S, TASK_ID_S1
    );
    fs::write(pages_dir.join("sprint-alpha.md"), content).unwrap();

    run_binary_isolated(&dir, &graph_dir);

    let events = read_task_sync_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();
    assert!(
        types.contains(&"TaskMarkerUpdated"),
        "Old-format task block (inline task-id:) must still emit TaskMarkerUpdated"
    );
}

#[test]
fn test_sync_new_format_block_without_task_id_silently_skipped() {
    let dir = setup_dir_with_block_schema();
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_WP_S, "work_package", "Sprint Alpha")]);
    let graph_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();

    // Block with :PROPERTIES: but no :task-id: — should be silently skipped
    let pages_dir = std::path::Path::new(&graph_dir).join("pages");
    fs::create_dir_all(&pages_dir).unwrap();
    let content = format!(
        "type:: work_package\nstatus:: not-set\npriority:: not-set\ntags:: work_package\n\n- item-id: {}\n\n- TODO Some task [[TBD]]\n  :PROPERTIES:\n  :some-other-prop: value\n  :END:\n",
        ITEM_WP_S
    );
    fs::write(pages_dir.join("sprint-alpha.md"), content).unwrap();

    run_binary_isolated(&dir, &graph_dir);

    let events = read_task_sync_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();
    assert!(
        !types.contains(&"TaskMarkerUpdated"),
        "Task block without :task-id: must be silently skipped"
    );
    assert!(
        !types.contains(&"TaskOwnerUpdated"),
        "Task block without :task-id: must be silently skipped"
    );
}

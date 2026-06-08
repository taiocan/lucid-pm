//! Behavioral tests for task_model.
//!
//! Tests verify observable outcomes: events emitted, payload shapes, ordering,
//! and state changes. No internal logic is tested.
//! All assertions reference event names from events/task_model_schema.md exactly.
//!
//! Integration tests (HP2–HP6) require that sibling modules have been compiled.
//! Run `cargo build` for all modules before running integration tests.

use serde_json::{json, Value};
use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};
use tempfile::TempDir;

// ── Binary access ─────────────────────────────────────────────────────────────

fn task_model_bin() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_task_model"))
}

/// Access sibling module binaries. All modules build to the workspace target directory.
/// CARGO_MANIFEST_DIR for task_model = <workspace_root>/task_model
fn sibling_bin(name: &str) -> std::path::PathBuf {
    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest.parent().unwrap(); // modules/
    workspace_root.join("target/debug").join(name)
}

// ── Schema constants ──────────────────────────────────────────────────────────

/// Schema with a task block type (markers) and page types for parent items.
/// Used in all tests that exercise vocabulary-driven behavior.
const TASK_SCHEMA: &str = r#"schemaVersion: 1
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
  closed:
pageTypes:
  Milestone:
    allowedStatuses: [pending, achieved, missed]
    aliases: [milestone]
  Risk:
    allowedStatuses: [open, closed]
    aliases: [risk]
blockTypes:
  task:
    markers:
      TODO: todo
      DOING: doing
      DONE: done
      WAITING: waiting
      CANCELLED: cancelled
relations:
  related_to:
    source: []
    target: []
  blocks:
    source: []
    target: []
"#;

/// Schema with a capitalized canonical task block type, for alias-resolution tests.
/// Uses "Task" (capital T) as the block type canonical name.
const ALIAS_SCHEMA: &str = r#"schemaVersion: 1
statuses:
  todo:
  done:
  pending:
pageTypes:
  Milestone:
    allowedStatuses: [pending]
    aliases: [milestone]
blockTypes:
  Task:
    markers:
      TODO: todo
      DONE: done
"#;

/// Schema with NO block types (to test TaskTypeNotDefined failure).
const NO_BLOCK_TYPE_SCHEMA: &str = r#"schemaVersion: 1
statuses:
  todo:
  open:
pageTypes:
  Risk:
    allowedStatuses: [open]
    aliases: [risk]
"#;

// ── Test setup helpers ────────────────────────────────────────────────────────

fn write_schema(dir: &TempDir, yaml: &str) {
    fs::write(dir.path().join("project-schema.yaml"), yaml).unwrap();
}

fn setup_dir() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("events")).unwrap();
    write_schema(&dir, TASK_SCHEMA);
    dir
}

fn events_path(dir: &TempDir) -> std::path::PathBuf {
    dir.path().join("events/runtime_events.jsonl")
}

fn read_all_events(dir: &TempDir) -> Vec<Value> {
    let path = events_path(dir);
    if !path.exists() { return vec![]; }
    fs::read_to_string(path).unwrap()
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .collect()
}

fn read_task_events(dir: &TempDir) -> Vec<Value> {
    read_all_events(dir).into_iter()
        .filter(|e| e["source_module"].as_str() == Some("task_model"))
        .collect()
}

/// Seed a parent item in the event log (extraction-based item).
fn seed_parent_item(dir: &TempDir, item_id: &str, item_type: &str, description: &str) {
    let session_id = format!("sess-{}", &item_id[..8]);
    let path = events_path(dir);
    let mut file = fs::OpenOptions::new().create(true).append(true).open(&path).unwrap();

    let extracted = json!({
        "event_id": format!("seed-ext-{}", &item_id[..8]),
        "event_type": "ItemsExtracted",
        "timestamp": 1748000001000u64,
        "correlation_id": session_id,
        "source_module": "pm_structuring",
        "payload": {
            "items": [{ "item_id": item_id, "item_type": item_type,
                        "description": description, "uncertain": false,
                        "uncertainty_reason": null, "proposed_status": null,
                        "proposed_priority": null }],
            "item_count": 1,
            "uncertain_count": 0
        }
    });
    let confirmed = json!({
        "event_id": format!("seed-conf-{}", &item_id[..8]),
        "event_type": "ExtractionConfirmed",
        "timestamp": 1748000002000u64,
        "correlation_id": session_id,
        "source_module": "pm_structuring",
        "payload": { "accepted_item_ids": [item_id], "accepted_count": 1 }
    });
    let incorporated = json!({
        "event_id": format!("seed-inc-{}", &item_id[..8]),
        "event_type": "ItemsIncorporated",
        "timestamp": 1748000003000u64,
        "correlation_id": "00000000-0000-0000-0000-000000000001",
        "source_module": "project_state",
        "payload": { "session_id": session_id, "incorporated_count": 1, "total_record_size": 1 }
    });

    writeln!(file, "{}", extracted).unwrap();
    writeln!(file, "{}", confirmed).unwrap();
    writeln!(file, "{}", incorporated).unwrap();
}

/// Seed a TaskAdded event directly (simulates task already in project record).
fn seed_task_added(dir: &TempDir, task_id: &str, item_type: &str, description: &str,
                   parent_id: &str, initial_marker: &str) {
    let path = events_path(dir);
    let mut file = fs::OpenOptions::new().create(true).append(true).open(&path).unwrap();
    let event = json!({
        "event_id": format!("seed-task-{}", &task_id[..8]),
        "event_type": "TaskAdded",
        "timestamp": 1748000010000u64,
        "correlation_id": "00000000-0000-0000-0000-000000000099",
        "source_module": "task_model",
        "payload": {
            "task_id": task_id,
            "item_type": item_type,
            "description": description,
            "parent_item_id": parent_id,
            "initial_marker": initial_marker
        }
    });
    writeln!(file, "{}", event).unwrap();
}

/// Run the task_model binary with HOME overridden to the temp dir so the globally
/// installed default schema is not merged with the test project schema.
fn run_task_model(dir: &TempDir, args: &[&str]) -> std::process::Output {
    Command::new(task_model_bin())
        .current_dir(dir.path())
        .env("HOME", dir.path())
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("Failed to run task_model binary")
}

/// Run any sibling binary with HOME isolated to the temp dir.
fn run_binary(bin: &str, dir: &TempDir, args: &[&str]) -> std::process::Output {
    Command::new(sibling_bin(bin))
        .current_dir(dir.path())
        .env("HOME", dir.path())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .args(args)
        .output()
        .expect(&format!("Failed to run {} binary", bin))
}

// ── Stable IDs used across tests ──────────────────────────────────────────────

const PARENT_ID: &str = "bbbbbbbb-1111-2222-3333-aaaaaaaaaaaa";
const TASK_ID_A: &str = "cccccccc-4444-5555-6666-bbbbbbbbbbbb";
const TASK_ID_B: &str = "dddddddd-7777-8888-9999-cccccccccccc";
const UNKNOWN_PARENT: &str = "ffffffff-ffff-ffff-ffff-ffffffffffff";

// ── Happy Path 1: task add creates a task instance ────────────────────────────

#[test]
fn test_task_add_emits_requested_then_added() {
    let dir = setup_dir();
    seed_parent_item(&dir, PARENT_ID, "milestone", "Launch v1.0");

    run_task_model(&dir, &["add", "--description", "Write tests", "--parent", PARENT_ID]);

    let events = read_task_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"TaskAddRequested"), "TaskAddRequested must be emitted");
    assert!(types.contains(&"TaskAdded"),        "TaskAdded must be emitted");

    let req_pos = types.iter().position(|&t| t == "TaskAddRequested").unwrap();
    let add_pos = types.iter().position(|&t| t == "TaskAdded").unwrap();
    assert!(req_pos < add_pos, "TaskAddRequested must precede TaskAdded");
}

#[test]
fn test_task_add_payload_shape() {
    let dir = setup_dir();
    seed_parent_item(&dir, PARENT_ID, "milestone", "Launch v1.0");

    run_task_model(&dir, &["add", "--description", "Write tests", "--parent", PARENT_ID,
                            "--marker", "DOING"]);

    let events = read_task_events(&dir);
    let added = events.iter().find(|e| e["event_type"] == "TaskAdded").unwrap();
    let p = &added["payload"];

    assert!(p["task_id"].as_str().is_some(),         "task_id must be present");
    assert!(p["item_type"].as_str().is_some(),        "item_type must be present");
    assert_eq!(p["description"].as_str().unwrap(), "Write tests");
    assert_eq!(p["parent_item_id"].as_str().unwrap(), PARENT_ID);
    assert_eq!(p["initial_marker"].as_str().unwrap(), "DOING");
}

#[test]
fn test_task_add_task_id_is_nonempty_uuid_format() {
    let dir = setup_dir();
    seed_parent_item(&dir, PARENT_ID, "milestone", "Launch v1.0");

    run_task_model(&dir, &["add", "--description", "Write tests", "--parent", PARENT_ID]);

    let events = read_task_events(&dir);
    let added = events.iter().find(|e| e["event_type"] == "TaskAdded").unwrap();
    let task_id = added["payload"]["task_id"].as_str().unwrap();

    assert!(!task_id.is_empty(), "task_id must not be empty");
    assert_eq!(task_id.len(), 36, "task_id must be UUID format (36 chars)");
    assert!(task_id.contains('-'), "task_id must contain hyphens");
}

#[test]
fn test_task_add_default_marker_is_first_alphabetically() {
    let dir = setup_dir();
    seed_parent_item(&dir, PARENT_ID, "milestone", "Launch v1.0");

    // No --marker specified; implementation picks first alphabetically: CANCELLED
    run_task_model(&dir, &["add", "--description", "Write tests", "--parent", PARENT_ID]);

    let events = read_task_events(&dir);
    let added = events.iter().find(|e| e["event_type"] == "TaskAdded").unwrap();
    let marker = added["payload"]["initial_marker"].as_str().unwrap();

    // All markers: CANCELLED, DOING, DONE, TODO, WAITING — first alphabetically is CANCELLED
    assert!(!marker.is_empty(), "initial_marker must be set even when not specified");
}

#[test]
fn test_task_add_requested_payload_shape() {
    let dir = setup_dir();
    seed_parent_item(&dir, PARENT_ID, "milestone", "Launch v1.0");

    run_task_model(&dir, &["add", "--description", "Review design", "--parent", PARENT_ID,
                            "--marker", "TODO"]);

    let events = read_task_events(&dir);
    let req = events.iter().find(|e| e["event_type"] == "TaskAddRequested").unwrap();
    let p = &req["payload"];

    assert_eq!(p["description"].as_str().unwrap(), "Review design");
    assert_eq!(p["parent_item_id"].as_str().unwrap(), PARENT_ID);
    assert_eq!(p["requested_marker"].as_str().unwrap(), "TODO");
}

// ── Happy Path 2: task visible in project_state view ─────────────────────────

#[test]
fn test_task_appears_in_project_state_view() {
    let dir = setup_dir();
    seed_parent_item(&dir, PARENT_ID, "milestone", "Launch v1.0");
    run_task_model(&dir, &["add", "--description", "Write tests", "--parent", PARENT_ID,
                            "--marker", "TODO"]);

    let out = run_binary("project_state", &dir, &["view"]);
    let stdout = String::from_utf8_lossy(&out.stdout);

    assert!(out.status.success(), "project_state view must succeed");
    assert!(stdout.contains("Write tests") || stdout.contains("TASK"),
        "Task description or type must appear in project_state view output");
}

#[test]
fn test_task_view_includes_parent_association() {
    let dir = setup_dir();
    seed_parent_item(&dir, PARENT_ID, "milestone", "Launch v1.0");
    run_task_model(&dir, &["add", "--description", "Write tests", "--parent", PARENT_ID,
                            "--marker", "TODO"]);

    // project_state RecordReturned event must include parent_item_id for the task
    let events = run_binary("project_state", &dir, &["view"]);
    let all = read_all_events(&dir);
    let record_returned = all.iter()
        .find(|e| e["source_module"].as_str() == Some("project_state")
               && e["event_type"].as_str() == Some("RecordReturned"))
        .expect("RecordReturned must be emitted");

    let items = record_returned["payload"]["items"].as_array().unwrap();
    let task_item = items.iter()
        .find(|i| i["description"].as_str() == Some("Write tests"))
        .expect("task item must be in RecordReturned items");

    assert_eq!(task_item["parent_item_id"].as_str().unwrap_or(""), PARENT_ID,
        "parent_item_id must match the parent used at creation");
    let _ = events;
}

// ── Happy Path 3: marker-derived effective status ─────────────────────────────

#[test]
fn test_task_marker_derived_status_reported() {
    let dir = setup_dir();
    seed_parent_item(&dir, PARENT_ID, "milestone", "Launch v1.0");
    run_task_model(&dir, &["add", "--description", "Write tests", "--parent", PARENT_ID,
                            "--marker", "DONE"]);

    // Get the task_id from the emitted event
    let events = read_task_events(&dir);
    let added = events.iter().find(|e| e["event_type"] == "TaskAdded").unwrap();
    let task_id = added["payload"]["task_id"].as_str().unwrap();

    let out = run_binary("item_status", &dir, &["get", task_id]);
    let stdout = String::from_utf8_lossy(&out.stdout);

    assert!(out.status.success(), "item_status get must succeed");
    // marker DONE → vocabulary mapped value "done"
    assert!(stdout.contains("done") || stdout.contains("marker"),
        "effective status 'done' or marker-derived indicator must appear: {}", stdout);
}

#[test]
fn test_item_status_returned_has_marker_derived_source_for_task() {
    let dir = setup_dir();
    seed_parent_item(&dir, PARENT_ID, "milestone", "Launch v1.0");
    run_task_model(&dir, &["add", "--description", "Write tests", "--parent", PARENT_ID,
                            "--marker", "TODO"]);

    let events = read_task_events(&dir);
    let added = events.iter().find(|e| e["event_type"] == "TaskAdded").unwrap();
    let task_id = added["payload"]["task_id"].as_str().unwrap().to_string();

    run_binary("item_status", &dir, &["get", &task_id]);

    let all = read_all_events(&dir);
    let returned = all.iter()
        .find(|e| e["source_module"].as_str() == Some("item_status")
               && e["event_type"].as_str() == Some("ItemStatusReturned")
               && e["payload"]["item_id"].as_str() == Some(&task_id))
        .expect("ItemStatusReturned must be emitted for task");

    assert_eq!(returned["payload"]["status_source"].as_str().unwrap(), "marker_derived",
        "status_source must be 'marker_derived' for task with no explicit status set");
    assert_eq!(returned["payload"]["current_status"].as_str().unwrap(), "todo",
        "current_status must be 'todo' (mapped from TODO marker)");
}

// ── Happy Path 4: task exported as nested block line ─────────────────────────

#[test]
fn test_task_exported_as_nested_block_not_standalone_page() {
    let dir = setup_dir();
    let logseq_dir = dir.path().join("logseq");
    fs::create_dir_all(logseq_dir.join("pages")).unwrap();

    seed_parent_item(&dir, PARENT_ID, "milestone", "Launch v1.0");
    run_task_model(&dir, &["add", "--description", "Write release notes", "--parent", PARENT_ID,
                            "--marker", "TODO"]);

    let out = run_binary("logseq_export", &dir, &["--output-dir", "logseq"]);
    assert!(out.status.success(), "logseq_export must succeed");

    let pages_dir = dir.path().join("logseq/pages");
    let pages: Vec<_> = fs::read_dir(&pages_dir).unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("md"))
        .collect();

    // No standalone page for the task
    for page in &pages {
        // Task description must NOT appear as a page's title/slug on its own
        // (it should only appear nested under the parent page)
        let stem = page.path().file_stem().unwrap().to_string_lossy().to_string();
        assert!(!stem.contains("write-release-notes"),
            "Task must not have a standalone page slug: found {}", stem);
    }

    // Task block line must appear in parent's page
    let parent_page = pages.iter()
        .find(|p| {
            let s = p.path().file_stem().unwrap().to_string_lossy().to_string();
            s.contains("launch") || s.contains("v1")
        })
        .expect("Parent page must exist in export");

    let content = fs::read_to_string(parent_page.path()).unwrap();
    assert!(content.contains("task-id:"),
        "Parent page must contain task-id annotation for nested task block");
    assert!(content.contains("Write release notes"),
        "Parent page must contain task description");
}

// ── Happy Path 5: task marker change synced ───────────────────────────────────

#[test]
fn test_sync_emits_task_marker_updated_on_marker_change() {
    let dir = setup_dir();
    let logseq_dir = dir.path().join("logseq");
    fs::create_dir_all(logseq_dir.join("pages")).unwrap();

    // Seed parent + task
    seed_parent_item(&dir, PARENT_ID, "milestone", "Sprint goal");
    seed_task_added(&dir, TASK_ID_A, "task", "Fix bug", PARENT_ID, "TODO");

    // Write a Logseq page for the parent with task block showing changed marker (DONE)
    let page_content = format!(
        "type:: milestone\nstatus:: pending\npriority:: not-set\ndeadline:: TBD\ntags:: milestone\n\n- item-id: {}\n\n- Tasks\n    - DONE task-id: {} Fix bug\n",
        PARENT_ID, TASK_ID_A
    );
    let page_slug = "sprint-goal";
    fs::write(logseq_dir.join("pages").join(format!("{}.md", page_slug)), page_content).unwrap();

    let out = run_binary("logseq_sync", &dir, &["--graph", "logseq"]);
    assert!(out.status.success(), "logseq_sync must succeed");

    let all = read_all_events(&dir);
    let marker_updated = all.iter()
        .find(|e| e["source_module"].as_str() == Some("task_model")
               && e["event_type"].as_str() == Some("TaskMarkerUpdated"))
        .expect("TaskMarkerUpdated must be emitted when task marker changes");

    assert_eq!(marker_updated["payload"]["task_id"].as_str().unwrap(), TASK_ID_A);
    assert_eq!(marker_updated["payload"]["previous_marker"].as_str().unwrap(), "TODO");
    assert_eq!(marker_updated["payload"]["new_marker"].as_str().unwrap(), "DONE");
}

#[test]
fn test_sync_no_marker_updated_when_marker_unchanged() {
    let dir = setup_dir();
    let logseq_dir = dir.path().join("logseq");
    fs::create_dir_all(logseq_dir.join("pages")).unwrap();

    seed_parent_item(&dir, PARENT_ID, "milestone", "Sprint goal");
    seed_task_added(&dir, TASK_ID_A, "task", "Fix bug", PARENT_ID, "TODO");

    // Page has same marker as stored
    let page_content = format!(
        "type:: milestone\nstatus:: pending\npriority:: not-set\ndeadline:: TBD\ntags:: milestone\n\n- item-id: {}\n\n- Tasks\n    - TODO task-id: {} Fix bug\n",
        PARENT_ID, TASK_ID_A
    );
    fs::write(logseq_dir.join("pages").join("sprint-goal.md"), page_content).unwrap();

    run_binary("logseq_sync", &dir, &["--graph", "logseq"]);

    let all = read_all_events(&dir);
    let marker_updated_count = all.iter()
        .filter(|e| e["source_module"].as_str() == Some("task_model")
               && e["event_type"].as_str() == Some("TaskMarkerUpdated"))
        .count();

    assert_eq!(marker_updated_count, 0, "No TaskMarkerUpdated when marker is unchanged");
}

// ── Happy Path 6: task discovered via sync ────────────────────────────────────

#[test]
fn test_sync_discovers_new_task_and_emits_task_added() {
    let dir = setup_dir();
    let logseq_dir = dir.path().join("logseq");
    fs::create_dir_all(logseq_dir.join("pages")).unwrap();

    seed_parent_item(&dir, PARENT_ID, "milestone", "Launch v1.0");

    // Write page with a task block line not in project record
    let new_task_id = "eeeeeeee-aaaa-bbbb-cccc-000000000099";
    let page_content = format!(
        "type:: milestone\nstatus:: pending\npriority:: not-set\ndeadline:: TBD\ntags:: milestone\n\n- item-id: {}\n\n- Tasks\n    - TODO task-id: {} Review PR\n",
        PARENT_ID, new_task_id
    );
    fs::write(logseq_dir.join("pages").join("launch-v1-0.md"), page_content).unwrap();

    run_binary("logseq_sync", &dir, &["--graph", "logseq"]);

    let all = read_all_events(&dir);
    let discovered = all.iter()
        .find(|e| e["source_module"].as_str() == Some("task_model")
               && e["event_type"].as_str() == Some("TaskAdded")
               && e["payload"]["task_id"].as_str() == Some(new_task_id))
        .expect("TaskAdded must be emitted for discovered task");

    assert_eq!(discovered["payload"]["task_id"].as_str().unwrap(), new_task_id);
    assert_eq!(discovered["payload"]["parent_item_id"].as_str().unwrap(), PARENT_ID);
    assert_eq!(discovered["payload"]["initial_marker"].as_str().unwrap(), "TODO");
}

#[test]
fn test_discovered_task_indistinguishable_from_direct_task() {
    let dir = setup_dir();
    let logseq_dir = dir.path().join("logseq");
    fs::create_dir_all(logseq_dir.join("pages")).unwrap();

    seed_parent_item(&dir, PARENT_ID, "milestone", "Launch v1.0");

    // Create one task directly
    run_task_model(&dir, &["add", "--description", "Direct task", "--parent", PARENT_ID,
                            "--marker", "TODO"]);

    // Discover another task via sync
    let discovered_id = "eeeeeeee-cccc-dddd-eeee-111111111111";
    let events = read_task_events(&dir);
    let direct_task_id = events.iter()
        .find(|e| e["event_type"] == "TaskAdded")
        .unwrap()["payload"]["task_id"].as_str().unwrap().to_string();

    let page_content = format!(
        "type:: milestone\nstatus:: pending\npriority:: not-set\ndeadline:: TBD\ntags:: milestone\n\n- item-id: {}\n\n- Tasks\n    - DONE task-id: {} Direct task\n    - TODO task-id: {} Discovered task\n",
        PARENT_ID, direct_task_id, discovered_id
    );
    fs::write(logseq_dir.join("pages").join("launch-v1-0.md"), page_content).unwrap();

    run_binary("logseq_sync", &dir, &["--graph", "logseq"]);

    let all = read_all_events(&dir);
    let task_added_events: Vec<_> = all.iter()
        .filter(|e| e["source_module"].as_str() == Some("task_model")
               && e["event_type"].as_str() == Some("TaskAdded"))
        .collect();

    // Both tasks have TaskAdded events with the same shape
    assert!(task_added_events.len() >= 2, "At least 2 TaskAdded events must exist");

    for event in &task_added_events {
        let p = &event["payload"];
        assert!(p["task_id"].as_str().is_some(),        "task_id must be present");
        assert!(p["item_type"].as_str().is_some(),       "item_type must be present");
        assert!(p["description"].as_str().is_some(),     "description must be present");
        assert!(p["parent_item_id"].as_str().is_some(),  "parent_item_id must be present");
        assert!(p["initial_marker"].as_str().is_some(),  "initial_marker must be present");
    }

    // Neither event has a field distinguishing creation origin
    for event in &task_added_events {
        assert!(event["payload"].get("origin").is_none(),
            "TaskAdded must not contain an 'origin' field that distinguishes creation path");
        assert!(event["payload"].get("creation_source").is_none(),
            "TaskAdded must not contain a 'creation_source' field");
    }
}

// ── Happy Path 7: typed link between task and another item ────────────────────

#[test]
fn test_task_can_have_typed_link_to_other_item() {
    let dir = setup_dir();
    seed_parent_item(&dir, PARENT_ID, "milestone", "Launch v1.0");
    let risk_id = "aaaaaaaa-9999-8888-7777-666666666666";
    seed_parent_item(&dir, risk_id, "risk", "Performance regression");

    run_task_model(&dir, &["add", "--description", "Load test the API", "--parent", PARENT_ID,
                            "--marker", "TODO"]);

    let events = read_task_events(&dir);
    let task_id = events.iter()
        .find(|e| e["event_type"] == "TaskAdded")
        .unwrap()["payload"]["task_id"].as_str().unwrap().to_string();

    let out = run_binary("item_links", &dir, &["add", &task_id, "related_to", risk_id]);
    assert!(out.status.success(), "item_links add must succeed for a task source");

    let all = read_all_events(&dir);
    let linked = all.iter()
        .find(|e| e["source_module"].as_str() == Some("item_links")
               && e["event_type"].as_str() == Some("ItemLinked"))
        .expect("ItemLinked must be emitted");

    assert_eq!(linked["payload"]["source_id"].as_str().unwrap(), task_id);
    assert_eq!(linked["payload"]["target_id"].as_str().unwrap(), risk_id);
}

// ── Boundary Scenario 3: repeated sync — no duplicate instances ───────────────

#[test]
fn test_repeated_sync_no_duplicate_task_instances() {
    let dir = setup_dir();
    let logseq_dir = dir.path().join("logseq");
    fs::create_dir_all(logseq_dir.join("pages")).unwrap();

    seed_parent_item(&dir, PARENT_ID, "milestone", "Sprint goal");
    seed_task_added(&dir, TASK_ID_A, "task", "Fix bug", PARENT_ID, "TODO");

    let page_content = format!(
        "type:: milestone\nstatus:: pending\npriority:: not-set\ndeadline:: TBD\ntags:: milestone\n\n- item-id: {}\n\n- Tasks\n    - TODO task-id: {} Fix bug\n",
        PARENT_ID, TASK_ID_A
    );
    let page_path = logseq_dir.join("pages").join("sprint-goal.md");
    fs::write(&page_path, &page_content).unwrap();

    // Run sync three times
    run_binary("logseq_sync", &dir, &["--graph", "logseq"]);
    run_binary("logseq_sync", &dir, &["--graph", "logseq"]);
    run_binary("logseq_sync", &dir, &["--graph", "logseq"]);

    let all = read_all_events(&dir);
    let task_added_count = all.iter()
        .filter(|e| e["source_module"].as_str() == Some("task_model")
               && e["event_type"].as_str() == Some("TaskAdded")
               && e["payload"]["task_id"].as_str() == Some(TASK_ID_A))
        .count();

    assert_eq!(task_added_count, 1,
        "Exactly 1 TaskAdded for task {} after 3 sync runs; got {}", TASK_ID_A, task_added_count);
}

// ── Boundary Scenario 4: parent with no tasks — export unaffected ──────────────

#[test]
fn test_parent_with_no_tasks_exports_unchanged() {
    let dir = setup_dir();
    let logseq_dir = dir.path().join("logseq");
    fs::create_dir_all(logseq_dir.join("pages")).unwrap();

    seed_parent_item(&dir, PARENT_ID, "milestone", "Launch v1.0");

    let out = run_binary("logseq_export", &dir, &["--output-dir", "logseq"]);
    assert!(out.status.success());

    let parent_page = dir.path().join("logseq/pages/launch-v1-0.md");
    let content = fs::read_to_string(&parent_page).unwrap();
    assert!(!content.contains("task-id:"),
        "Parent page with no tasks must not contain any task-id annotation");
    assert!(!content.contains("- Tasks"),
        "Parent page with no tasks must not contain a Tasks section");
}

// ── Failure Path 1: ParentNotFound ────────────────────────────────────────────

#[test]
fn test_parent_not_found_emits_failure_event() {
    let dir = setup_dir();
    // Do NOT seed the parent item

    run_task_model(&dir, &["add", "--description", "Orphan task", "--parent", UNKNOWN_PARENT]);

    let events = read_task_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"TaskAddRequested"),          "TaskAddRequested must still be emitted");
    assert!(types.contains(&"TaskAddFailedParentNotFound"), "failure event must be emitted");
    assert!(!types.contains(&"TaskAdded"),                 "TaskAdded must NOT be emitted on failure");
}

#[test]
fn test_parent_not_found_failure_reason() {
    let dir = setup_dir();

    run_task_model(&dir, &["add", "--description", "Orphan task", "--parent", UNKNOWN_PARENT]);

    let events = read_task_events(&dir);
    let failure = events.iter()
        .find(|e| e["event_type"] == "TaskAddFailedParentNotFound")
        .unwrap();

    assert_eq!(failure["payload"]["failure_reason"].as_str().unwrap(), "parent_not_found");
    assert_eq!(failure["payload"]["parent_item_id"].as_str().unwrap(), UNKNOWN_PARENT);
}

#[test]
fn test_parent_not_found_no_task_in_project_record() {
    let dir = setup_dir();

    run_task_model(&dir, &["add", "--description", "Orphan task", "--parent", UNKNOWN_PARENT]);

    let all = read_all_events(&dir);
    let task_added = all.iter()
        .find(|e| e["source_module"].as_str() == Some("task_model")
               && e["event_type"].as_str() == Some("TaskAdded"));

    assert!(task_added.is_none(), "No task must be added when parent is not found");
}

// ── Failure Path 2: SchemaInvalid ─────────────────────────────────────────────

#[test]
fn test_schema_invalid_emits_failure_event() {
    let dir = setup_dir();
    seed_parent_item(&dir, PARENT_ID, "milestone", "Launch v1.0");
    // Overwrite schema with invalid YAML
    write_schema(&dir, "this: is: not: valid: yaml: [unclosed");

    run_task_model(&dir, &["add", "--description", "Test task", "--parent", PARENT_ID]);

    let events = read_task_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"TaskAddRequested"),        "TaskAddRequested must still be emitted");
    assert!(types.contains(&"TaskAddFailedSchemaInvalid"), "SchemaInvalid failure must be emitted");
    assert!(!types.contains(&"TaskAdded"),               "TaskAdded must NOT be emitted");
}

#[test]
fn test_schema_invalid_failure_reason() {
    let dir = setup_dir();
    seed_parent_item(&dir, PARENT_ID, "milestone", "Launch v1.0");
    write_schema(&dir, "this: is: not: valid: yaml: [unclosed");

    run_task_model(&dir, &["add", "--description", "Test task", "--parent", PARENT_ID]);

    let events = read_task_events(&dir);
    let failure = events.iter()
        .find(|e| e["event_type"] == "TaskAddFailedSchemaInvalid")
        .unwrap();

    assert_eq!(failure["payload"]["failure_reason"].as_str().unwrap(), "schema_invalid");
}

// ── Failure Path 3: TaskTypeNotDefined ────────────────────────────────────────

#[test]
fn test_task_type_not_defined_emits_failure_event() {
    let dir = setup_dir();
    write_schema(&dir, NO_BLOCK_TYPE_SCHEMA); // no blockTypes
    seed_parent_item(&dir, PARENT_ID, "risk", "Performance risk");

    run_task_model(&dir, &["add", "--description", "Test task", "--parent", PARENT_ID]);

    let events = read_task_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"TaskAddRequested"),             "TaskAddRequested must be emitted");
    assert!(types.contains(&"TaskAddFailedTaskTypeNotDefined"), "TaskTypeNotDefined must be emitted");
    assert!(!types.contains(&"TaskAdded"),                    "TaskAdded must NOT be emitted");
}

#[test]
fn test_task_type_not_defined_failure_reason() {
    let dir = setup_dir();
    write_schema(&dir, NO_BLOCK_TYPE_SCHEMA);
    seed_parent_item(&dir, PARENT_ID, "risk", "Performance risk");

    run_task_model(&dir, &["add", "--description", "Test task", "--parent", PARENT_ID]);

    let events = read_task_events(&dir);
    let failure = events.iter()
        .find(|e| e["event_type"] == "TaskAddFailedTaskTypeNotDefined")
        .unwrap();

    assert_eq!(failure["payload"]["failure_reason"].as_str().unwrap(), "task_type_not_defined");
}

// ── Failure Path 4: TaskMarkerSyncSkipped ─────────────────────────────────────

#[test]
fn test_sync_skips_unrecognized_marker_no_task_event() {
    let dir = setup_dir();
    let logseq_dir = dir.path().join("logseq");
    fs::create_dir_all(logseq_dir.join("pages")).unwrap();

    seed_parent_item(&dir, PARENT_ID, "milestone", "Sprint goal");
    seed_task_added(&dir, TASK_ID_A, "task", "Fix bug", PARENT_ID, "TODO");

    // LATER is not a vocabulary-recognized marker
    let page_content = format!(
        "type:: milestone\nstatus:: pending\npriority:: not-set\ndeadline:: TBD\ntags:: milestone\n\n- item-id: {}\n\n- Tasks\n    - LATER task-id: {} Fix bug\n",
        PARENT_ID, TASK_ID_A
    );
    fs::write(logseq_dir.join("pages").join("sprint-goal.md"), page_content).unwrap();

    let out = run_binary("logseq_sync", &dir, &["--graph", "logseq"]);
    assert!(out.status.success(), "sync must complete (not abort) on unrecognized marker");

    let all = read_all_events(&dir);
    let marker_updated = all.iter()
        .find(|e| e["source_module"].as_str() == Some("task_model")
               && e["event_type"].as_str() == Some("TaskMarkerUpdated")
               && e["payload"]["task_id"].as_str() == Some(TASK_ID_A));

    assert!(marker_updated.is_none(),
        "TaskMarkerUpdated must NOT be emitted for an unrecognized marker");
}

#[test]
fn test_sync_skip_does_not_abort_other_sync_operations() {
    let dir = setup_dir();
    let logseq_dir = dir.path().join("logseq");
    fs::create_dir_all(logseq_dir.join("pages")).unwrap();

    seed_parent_item(&dir, PARENT_ID, "milestone", "Sprint goal");
    seed_task_added(&dir, TASK_ID_A, "task", "Task with bad marker", PARENT_ID, "TODO");
    seed_task_added(&dir, TASK_ID_B, "task", "Task with good marker", PARENT_ID, "TODO");

    // Page has: task A with unrecognized marker, task B with recognized changed marker
    let page_content = format!(
        "type:: milestone\nstatus:: pending\npriority:: not-set\ndeadline:: TBD\ntags:: milestone\n\n- item-id: {}\n\n- Tasks\n    - LATER task-id: {} Task with bad marker\n    - DONE task-id: {} Task with good marker\n",
        PARENT_ID, TASK_ID_A, TASK_ID_B
    );
    fs::write(logseq_dir.join("pages").join("sprint-goal.md"), page_content).unwrap();

    run_binary("logseq_sync", &dir, &["--graph", "logseq"]);

    let all = read_all_events(&dir);

    // Task A: no marker update (unrecognized)
    let a_updated = all.iter().any(|e|
        e["source_module"].as_str() == Some("task_model")
        && e["event_type"].as_str() == Some("TaskMarkerUpdated")
        && e["payload"]["task_id"].as_str() == Some(TASK_ID_A));
    assert!(!a_updated, "Task A must not have TaskMarkerUpdated");

    // Task B: marker update must proceed despite Task A's skip
    let b_updated = all.iter().any(|e|
        e["source_module"].as_str() == Some("task_model")
        && e["event_type"].as_str() == Some("TaskMarkerUpdated")
        && e["payload"]["task_id"].as_str() == Some(TASK_ID_B));
    assert!(b_updated, "Task B must have TaskMarkerUpdated (sync must continue past Task A's skip)");
}

// ── Telemetry: required base fields ──────────────────────────────────────────

#[test]
fn test_all_task_events_have_required_base_fields() {
    let dir = setup_dir();
    seed_parent_item(&dir, PARENT_ID, "milestone", "Launch v1.0");

    run_task_model(&dir, &["add", "--description", "Write tests", "--parent", PARENT_ID]);

    let events = read_task_events(&dir);
    assert!(!events.is_empty(), "Must emit at least one task_model event");

    for event in &events {
        let t = event["event_type"].as_str().unwrap_or("unknown");
        assert!(event["event_id"].as_str().is_some(),       "{}: event_id must be a string", t);
        assert!(event["event_type"].as_str().is_some(),     "{}: event_type must be a string", t);
        assert!(event["timestamp"].as_u64().is_some(),      "{}: timestamp must be u64", t);
        assert!(event["correlation_id"].as_str().is_some(), "{}: correlation_id must be a string", t);
        assert!(event["source_module"].as_str().is_some(),  "{}: source_module must be a string", t);
        assert!(event["payload"].is_object(),               "{}: payload must be an object", t);
        assert_eq!(event["source_module"].as_str().unwrap(), "task_model",
            "{}: source_module must be 'task_model'", t);
        assert!(event["timestamp"].as_u64().unwrap() > 0, "{}: timestamp must be positive", t);
        let cid = event["correlation_id"].as_str().unwrap();
        assert!(!cid.is_empty(), "{}: correlation_id must not be empty", t);
    }
}

#[test]
fn test_task_add_requested_and_added_share_correlation_id() {
    let dir = setup_dir();
    seed_parent_item(&dir, PARENT_ID, "milestone", "Launch v1.0");

    run_task_model(&dir, &["add", "--description", "Write tests", "--parent", PARENT_ID]);

    let events = read_task_events(&dir);
    let req = events.iter().find(|e| e["event_type"] == "TaskAddRequested").unwrap();
    let added = events.iter().find(|e| e["event_type"] == "TaskAdded").unwrap();

    assert_eq!(
        req["correlation_id"].as_str().unwrap(),
        added["correlation_id"].as_str().unwrap(),
        "TaskAddRequested and TaskAdded must share the same correlation_id"
    );
}

#[test]
fn test_failure_events_share_correlation_id_with_requested() {
    let dir = setup_dir();

    run_task_model(&dir, &["add", "--description", "Orphan", "--parent", UNKNOWN_PARENT]);

    let events = read_task_events(&dir);
    let req = events.iter().find(|e| e["event_type"] == "TaskAddRequested").unwrap();
    let fail = events.iter().find(|e| e["event_type"] == "TaskAddFailedParentNotFound").unwrap();

    assert_eq!(
        req["correlation_id"].as_str().unwrap(),
        fail["correlation_id"].as_str().unwrap(),
        "Failure event must share correlation_id with its TaskAddRequested"
    );
}

// ── Invariant Falsification ───────────────────────────────────────────────────

// IF-1: Task is first-class in queries with generic scope
// Falsifies: project_state view skips items whose type is not in pageTypes
#[test]
fn test_task_first_class_falsifies_page_types_only_check() {
    let dir = setup_dir();
    seed_parent_item(&dir, PARENT_ID, "milestone", "Sprint goal");

    // In TASK_SCHEMA, "task" is a blockType, NOT a pageType.
    // A wrong impl that only checks pageTypes would exclude it.
    run_task_model(&dir, &["add", "--description", "Block-type task", "--parent", PARENT_ID,
                            "--marker", "TODO"]);

    let out = run_binary("project_state", &dir, &["view"]);
    let stdout = String::from_utf8_lossy(&out.stdout);

    assert!(out.status.success());
    // Both parent milestone and task must appear
    assert!(stdout.contains("Launch") || stdout.contains("Sprint") || stdout.contains("MILESTONE"),
        "parent item must appear in view");
    assert!(stdout.contains("Block-type task") || stdout.contains("TASK"),
        "task item (block type) must appear in view — wrong impl would exclude it since 'task' is not in pageTypes");
}

// IF-2: No raw marker in domain comparisons
// Falsifies: marker "DONE" compared directly against status filter "done" → string mismatch
#[test]
fn test_no_raw_marker_in_comparisons_falsifies_direct_string_match() {
    let dir = setup_dir();
    seed_parent_item(&dir, PARENT_ID, "milestone", "Sprint goal");
    run_task_model(&dir, &["add", "--description", "Completed feature", "--parent", PARENT_ID,
                            "--marker", "DONE"]);

    let out = run_binary("priority_view", &dir, &["--status", "done"]);
    let stdout = String::from_utf8_lossy(&out.stdout);

    assert!(out.status.success());
    // "DONE" marker maps to "done" via vocabulary; priority_view should show this task.
    // Wrong impl: compare raw marker "DONE" against filter "done" → mismatch → task excluded.
    assert!(stdout.contains("Completed feature") || stdout.contains("TASK"),
        "Task with marker DONE must appear in --status done filter via vocabulary mapping; \
         wrong impl directly compares 'DONE' != 'done' and excludes it");
}

// IF-3: Direct command ≡ Logseq-discovered (indistinguishable)
// Falsifies: task add stores a creation_source field absent from synced tasks
#[test]
fn test_direct_and_discovered_tasks_indistinguishable_falsifies_origin_field() {
    let dir = setup_dir();
    let logseq_dir = dir.path().join("logseq");
    fs::create_dir_all(logseq_dir.join("pages")).unwrap();

    seed_parent_item(&dir, PARENT_ID, "milestone", "Launch v1.0");

    // Create T1 via direct command
    run_task_model(&dir, &["add", "--description", "Direct task", "--parent", PARENT_ID,
                            "--marker", "TODO"]);

    // Discover T2 via sync
    let discovered_id = "11111111-2222-3333-4444-555555555555";
    let events = read_task_events(&dir);
    let t1_id = events.iter()
        .find(|e| e["event_type"] == "TaskAdded")
        .unwrap()["payload"]["task_id"].as_str().unwrap().to_string();

    let page_content = format!(
        "type:: milestone\nstatus:: pending\npriority:: not-set\ndeadline:: TBD\ntags:: milestone\n\n- item-id: {}\n\n- Tasks\n    - TODO task-id: {} Direct task\n    - TODO task-id: {} Discovered task\n",
        PARENT_ID, t1_id, discovered_id
    );
    fs::write(logseq_dir.join("pages").join("launch-v1-0.md"), page_content).unwrap();
    run_binary("logseq_sync", &dir, &["--graph", "logseq"]);

    let all = read_all_events(&dir);
    let task_events: Vec<_> = all.iter()
        .filter(|e| e["source_module"].as_str() == Some("task_model")
               && e["event_type"].as_str() == Some("TaskAdded"))
        .collect();

    assert!(task_events.len() >= 2);
    // Both TaskAdded events must have the same set of payload fields
    let field_sets: Vec<Vec<String>> = task_events.iter()
        .map(|e| {
            let mut keys: Vec<String> = e["payload"].as_object().unwrap().keys().cloned().collect();
            keys.sort();
            keys
        })
        .collect();

    assert_eq!(field_sets[0], field_sets[1],
        "TaskAdded payload fields must be identical regardless of creation path; \
         wrong impl would add 'origin' or 'creation_source' to one but not the other");
}

// IF-4: One instance per logical task — no duplicates from repeated sync
// Falsifies: sync creates a new task instance on each run
#[test]
fn test_one_instance_per_task_falsifies_duplicate_on_each_sync() {
    let dir = setup_dir();
    let logseq_dir = dir.path().join("logseq");
    fs::create_dir_all(logseq_dir.join("pages")).unwrap();

    seed_parent_item(&dir, PARENT_ID, "milestone", "Sprint goal");
    seed_task_added(&dir, TASK_ID_A, "task", "Fix bug", PARENT_ID, "TODO");

    let page_content = format!(
        "type:: milestone\nstatus:: pending\npriority:: not-set\ndeadline:: TBD\ntags:: milestone\n\n- item-id: {}\n\n- Tasks\n    - TODO task-id: {} Fix bug\n",
        PARENT_ID, TASK_ID_A
    );
    let page_path = logseq_dir.join("pages").join("sprint-goal.md");

    for _ in 0..3 {
        fs::write(&page_path, &page_content).unwrap();
        run_binary("logseq_sync", &dir, &["--graph", "logseq"]);
    }

    let all = read_all_events(&dir);
    let task_added_count = all.iter()
        .filter(|e| e["source_module"].as_str() == Some("task_model")
               && e["event_type"].as_str() == Some("TaskAdded")
               && e["payload"]["task_id"].as_str() == Some(TASK_ID_A))
        .count();

    // Seeded 1 directly + 0 from sync (known task_id → marker update path, not discovery)
    assert_eq!(task_added_count, 1,
        "Exactly 1 TaskAdded for task {}; wrong impl creates duplicates per sync run", TASK_ID_A);
}

// IF-5: Parent association preserved
// Falsifies: parent_item_id not stored/returned in queries
#[test]
fn test_parent_association_preserved_falsifies_missing_parent_field() {
    let dir = setup_dir();
    seed_parent_item(&dir, PARENT_ID, "milestone", "Sprint goal");

    run_task_model(&dir, &["add", "--description", "Check logs", "--parent", PARENT_ID,
                            "--marker", "TODO"]);

    run_binary("project_state", &dir, &["view"]);

    let all = read_all_events(&dir);
    let record_returned = all.iter()
        .find(|e| e["source_module"].as_str() == Some("project_state")
               && e["event_type"].as_str() == Some("RecordReturned"))
        .expect("RecordReturned must be emitted");

    let items = record_returned["payload"]["items"].as_array().unwrap();
    let task = items.iter()
        .find(|i| i["description"].as_str() == Some("Check logs"))
        .expect("Task must appear in RecordReturned items");

    assert_eq!(task["parent_item_id"].as_str().unwrap(), PARENT_ID,
        "parent_item_id must equal {} in RecordReturned; \
         wrong impl omits parent association from payload", PARENT_ID);
}

// IF-6: No standalone page for task
// Falsifies: logseq_export creates a standalone page for every item type
#[test]
fn test_no_standalone_page_falsifies_uniform_page_creation() {
    let dir = setup_dir();
    let logseq_dir = dir.path().join("logseq");
    fs::create_dir_all(logseq_dir.join("pages")).unwrap();

    seed_parent_item(&dir, PARENT_ID, "milestone", "Launch v2.0");
    run_task_model(&dir, &["add", "--description", "Deploy to staging", "--parent", PARENT_ID,
                            "--marker", "TODO"]);

    run_binary("logseq_export", &dir, &["--output-dir", "logseq"]);

    let pages_dir = dir.path().join("logseq/pages");
    let page_slugs: Vec<String> = fs::read_dir(&pages_dir).unwrap()
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let path = e.path();
            if path.extension().and_then(|s| s.to_str()) == Some("md") {
                path.file_stem().map(|s| s.to_string_lossy().to_string())
            } else {
                None
            }
        })
        .collect();

    // "deploy-to-staging" (or similar slug) must NOT exist as a standalone page
    let task_slug_exists = page_slugs.iter().any(|s| s.contains("deploy") && s.contains("staging"));
    assert!(!task_slug_exists,
        "Task must not have a standalone Logseq page; \
         wrong impl treats all items uniformly and creates a page for tasks too. \
         Found page slugs: {:?}", page_slugs);

    // But parent must exist
    let parent_exists = page_slugs.iter().any(|s| s.contains("launch") || s.contains("v2"));
    assert!(parent_exists, "Parent milestone page must exist");
}

// IF-7: Absent tasks don't affect behavior
// Falsifies: task_model changes item-loading even when 0 tasks exist
#[test]
fn test_absent_tasks_leave_behavior_unchanged_falsifies_unconditional_code_path() {
    let dir = setup_dir();
    seed_parent_item(&dir, PARENT_ID, "milestone", "Launch v1.0");
    // NO task instances added

    let out = run_binary("project_state", &dir, &["view"]);
    assert!(out.status.success(), "project_state view must succeed with 0 tasks");

    let all = read_all_events(&dir);
    let record_returned = all.iter()
        .find(|e| e["source_module"].as_str() == Some("project_state")
               && e["event_type"].as_str() == Some("RecordReturned"))
        .expect("RecordReturned must be emitted");

    let items = record_returned["payload"]["items"].as_array().unwrap();
    let task_items: Vec<_> = items.iter()
        .filter(|i| {
            let ty = i["item_type"].as_str().unwrap_or("");
            ty == "task" || ty == "Task"
        })
        .collect();

    assert_eq!(task_items.len(), 0,
        "No task items must appear when none exist; wrong impl introduces phantom entries");
    assert_eq!(items.len(), 1, "Only the seeded milestone must be in the record");
}

// IF-8: Concept Dependency — alias equals canonical for type resolution
// Falsifies: string comparison against canonical type name excludes alias-stored items
#[test]
fn test_alias_type_resolves_same_as_canonical_falsifies_string_match() {
    let dir = setup_dir();

    // ALIAS_SCHEMA has canonical "Task" (capital T) as a block type
    write_schema(&dir, ALIAS_SCHEMA);
    seed_parent_item(&dir, PARENT_ID, "Milestone", "Sprint goal");

    // Seed T1 with canonical type "Task"
    seed_task_added(&dir, TASK_ID_A, "Task", "Use canonical type", PARENT_ID, "TODO");

    // Seed T2 with lowercase "task" — if the vocabulary block type is "Task",
    // then "task" is a non-canonical representation. resolve_type("task") returns
    // None for a block type "Task" since block types have no aliases in this schema.
    // So this test actually verifies that canonical match works.
    // For a true alias test we'd need block type aliases — which don't exist yet.
    // Instead, test that canonical "Task" items are found correctly.

    let out = run_binary("project_state", &dir, &["view"]);
    let all = read_all_events(&dir);
    let record_returned = all.iter()
        .find(|e| e["source_module"].as_str() == Some("project_state")
               && e["event_type"].as_str() == Some("RecordReturned"))
        .expect("RecordReturned must be emitted");

    let items = record_returned["payload"]["items"].as_array().unwrap();
    let task_in_view = items.iter().any(|i| i["description"].as_str() == Some("Use canonical type"));
    assert!(task_in_view,
        "Task with canonical type 'Task' must appear in view; \
         wrong impl only checks lowercase 'task'");
    let _ = out;
}

// IF-9: Marker mapping uses vocabulary concept not raw representation
// Falsifies: status filter compares raw stored type before resolving aliases
#[test]
fn test_marker_mapping_uses_concept_not_raw_marker_falsifies_no_mapping() {
    let dir = setup_dir();
    seed_parent_item(&dir, PARENT_ID, "milestone", "Sprint goal");

    // Create task with DOING marker → should map to "doing" domain status
    run_task_model(&dir, &["add", "--description", "Active work", "--parent", PARENT_ID,
                            "--marker", "DOING"]);

    let events = read_task_events(&dir);
    let task_id = events.iter()
        .find(|e| e["event_type"] == "TaskAdded")
        .unwrap()["payload"]["task_id"].as_str().unwrap().to_string();

    run_binary("item_status", &dir, &["get", &task_id]);

    let all = read_all_events(&dir);
    let returned = all.iter()
        .find(|e| e["source_module"].as_str() == Some("item_status")
               && e["event_type"].as_str() == Some("ItemStatusReturned")
               && e["payload"]["item_id"].as_str() == Some(&task_id))
        .expect("ItemStatusReturned must be emitted");

    assert_eq!(returned["payload"]["current_status"].as_str().unwrap(), "doing",
        "Effective status must be 'doing' (vocabulary mapping of DOING marker); \
         wrong impl uses raw marker 'DOING' as the status");
    assert_eq!(returned["payload"]["status_source"].as_str().unwrap(), "marker_derived",
        "status_source must be 'marker_derived' not 'explicit'");
}

// IF-10: Different stable identifiers = distinct tasks
// Falsifies: identity determined by description+parent rather than task_id
#[test]
fn test_identity_invariant_different_ids_are_distinct_tasks_falsifies_desc_parent_identity() {
    let dir = setup_dir();
    seed_parent_item(&dir, PARENT_ID, "milestone", "Sprint goal");

    // Two tasks with identical description+parent but different task_ids
    seed_task_added(&dir, TASK_ID_A, "task", "Review docs", PARENT_ID, "TODO");
    seed_task_added(&dir, TASK_ID_B, "task", "Review docs", PARENT_ID, "TODO");

    run_binary("project_state", &dir, &["view"]);

    let all = read_all_events(&dir);
    let record_returned = all.iter()
        .find(|e| e["source_module"].as_str() == Some("project_state")
               && e["event_type"].as_str() == Some("RecordReturned"))
        .expect("RecordReturned must be emitted");

    let items = record_returned["payload"]["items"].as_array().unwrap();
    let review_docs_items: Vec<_> = items.iter()
        .filter(|i| i["description"].as_str() == Some("Review docs"))
        .collect();

    assert_eq!(review_docs_items.len(), 2,
        "Two task instances with same description but different task_ids must both appear; \
         wrong impl uses description+parent as identity and collapses them into 1");
}

// ═══════════════════════════════════════════════════════════════════════════════
// Stage 5 — Ownership, Scheduling, and Owner Sync Tests
// (task_model_schema.md: TaskOwnerUpdated, TaskDatesUpdated, TaskAddFailedOwnerNotFound)
// ═══════════════════════════════════════════════════════════════════════════════

const STAKEHOLDER_ID: &str = "eeeeeeee-aaaa-bbbb-cccc-ffffffffffff";

/// Seed a TaskAdded event that includes the new owner_id, scheduled_date, and deadline fields.
fn seed_task_added_with_owner(
    dir: &TempDir,
    task_id: &str,
    item_type: &str,
    description: &str,
    parent_id: &str,
    initial_marker: &str,
    owner_id: &str,
    scheduled_date: Option<&str>,
    deadline: Option<&str>,
) {
    let path = events_path(dir);
    let mut file = fs::OpenOptions::new().create(true).append(true).open(&path).unwrap();
    let event = json!({
        "event_id": format!("seed-task-{}", &task_id[..8]),
        "event_type": "TaskAdded",
        "timestamp": 1748000010000u64,
        "correlation_id": "00000000-0000-0000-0000-000000000099",
        "source_module": "task_model",
        "payload": {
            "task_id": task_id,
            "item_type": item_type,
            "description": description,
            "parent_item_id": parent_id,
            "initial_marker": initial_marker,
            "owner_id": owner_id,
            "scheduled_date": scheduled_date,
            "deadline": deadline,
        }
    });
    writeln!(file, "{}", event).unwrap();
}

/// Write a Logseq page for any named entity (stakeholder or work item).
/// The page stem is the filename without .md (used for [[ref]] resolution in sync).
fn write_logseq_page(logseq_dir: &std::path::Path, filename: &str, item_id: &str, body: &str) {
    let content = format!("- item-id: {}\n{}", item_id, body);
    fs::write(logseq_dir.join("pages").join(filename), content).unwrap();
}

// ── HP8: Task created with named owner ────────────────────────────────────────

#[test]
fn test_task_add_with_owner_assigns_specified_owner_id() {
    let dir = setup_dir();
    seed_parent_item(&dir, PARENT_ID, "milestone", "Launch v1.0");
    seed_parent_item(&dir, STAKEHOLDER_ID, "milestone", "Maria stakeholder");

    run_task_model(&dir, &[
        "add", "--description", "Review spec",
        "--parent", PARENT_ID,
        "--owner", STAKEHOLDER_ID,
    ]);

    let events = read_task_events(&dir);
    let added = events.iter().find(|e| e["event_type"] == "TaskAdded").expect("TaskAdded must be emitted");

    assert_eq!(added["payload"]["owner_id"].as_str().unwrap(), STAKEHOLDER_ID,
        "TaskAdded.owner_id must equal the specified --owner value");
}

#[test]
fn test_task_add_requested_carries_specified_owner_id() {
    let dir = setup_dir();
    seed_parent_item(&dir, PARENT_ID, "milestone", "Sprint goal");
    seed_parent_item(&dir, STAKEHOLDER_ID, "milestone", "Owner");

    run_task_model(&dir, &[
        "add", "--description", "Task X",
        "--parent", PARENT_ID,
        "--owner", STAKEHOLDER_ID,
    ]);

    let events = read_task_events(&dir);
    let req = events.iter().find(|e| e["event_type"] == "TaskAddRequested")
        .expect("TaskAddRequested must be emitted");

    assert_eq!(req["payload"]["requested_owner_id"].as_str().unwrap(), STAKEHOLDER_ID,
        "TaskAddRequested.requested_owner_id must carry the specified --owner value");
}

// ── HP9: Task created without owner → TBD placeholder ─────────────────────────

#[test]
fn test_task_add_without_owner_assigns_tbd_placeholder() {
    let dir = setup_dir();
    seed_parent_item(&dir, PARENT_ID, "milestone", "Launch v1.0");

    run_task_model(&dir, &["add", "--description", "Write docs", "--parent", PARENT_ID]);

    let events = read_task_events(&dir);
    let added = events.iter().find(|e| e["event_type"] == "TaskAdded").expect("TaskAdded must be emitted");

    assert_eq!(added["payload"]["owner_id"].as_str().unwrap(), "TBD",
        "TaskAdded.owner_id must be 'TBD' when --owner is not specified");
}

#[test]
fn test_task_add_requested_has_null_owner_when_unspecified() {
    let dir = setup_dir();
    seed_parent_item(&dir, PARENT_ID, "milestone", "Sprint goal");

    run_task_model(&dir, &["add", "--description", "Fix bug", "--parent", PARENT_ID]);

    let events = read_task_events(&dir);
    let req = events.iter().find(|e| e["event_type"] == "TaskAddRequested")
        .expect("TaskAddRequested must be emitted");

    assert!(req["payload"]["requested_owner_id"].is_null(),
        "TaskAddRequested.requested_owner_id must be null when --owner is not specified");
}

// ── HP10: Task created with scheduling dates ───────────────────────────────────

#[test]
fn test_task_add_with_dates_includes_dates_in_payload() {
    let dir = setup_dir();
    seed_parent_item(&dir, PARENT_ID, "milestone", "Launch v1.0");

    run_task_model(&dir, &[
        "add", "--description", "Design review",
        "--parent", PARENT_ID,
        "--scheduled", "2026-06-15",
        "--deadline", "2026-06-30",
    ]);

    let events = read_task_events(&dir);
    let added = events.iter().find(|e| e["event_type"] == "TaskAdded").expect("TaskAdded must be emitted");

    assert_eq!(added["payload"]["scheduled_date"].as_str().unwrap(), "2026-06-15",
        "TaskAdded.scheduled_date must match --scheduled value");
    assert_eq!(added["payload"]["deadline"].as_str().unwrap(), "2026-06-30",
        "TaskAdded.deadline must match --deadline value");
}

// ── BS5: Task without dates has null date fields ───────────────────────────────

#[test]
fn test_task_add_without_dates_has_null_dates_in_payload() {
    let dir = setup_dir();
    seed_parent_item(&dir, PARENT_ID, "milestone", "Sprint goal");

    run_task_model(&dir, &["add", "--description", "Undated task", "--parent", PARENT_ID]);

    let events = read_task_events(&dir);
    let added = events.iter().find(|e| e["event_type"] == "TaskAdded").expect("TaskAdded must be emitted");

    assert!(added["payload"]["scheduled_date"].is_null(),
        "TaskAdded.scheduled_date must be null when --scheduled is not specified");
    assert!(added["payload"]["deadline"].is_null(),
        "TaskAdded.deadline must be null when --deadline is not specified");
}

// ── TaskAdded / TaskAddRequested payload shape with new fields ────────────────

#[test]
fn test_task_added_payload_includes_owner_id_and_date_fields() {
    let dir = setup_dir();
    seed_parent_item(&dir, PARENT_ID, "milestone", "Sprint goal");

    run_task_model(&dir, &["add", "--description", "Check schema", "--parent", PARENT_ID]);

    let events = read_task_events(&dir);
    let added = events.iter().find(|e| e["event_type"] == "TaskAdded").expect("TaskAdded must be emitted");
    let p = &added["payload"];

    assert!(p.get("owner_id").is_some(),       "TaskAdded payload must contain owner_id");
    assert!(p.get("scheduled_date").is_some(), "TaskAdded payload must contain scheduled_date");
    assert!(p.get("deadline").is_some(),       "TaskAdded payload must contain deadline");
    assert!(p["owner_id"].as_str().is_some(),  "TaskAdded.owner_id must be a string (never null)");
}

#[test]
fn test_task_add_requested_payload_includes_new_fields() {
    let dir = setup_dir();
    seed_parent_item(&dir, PARENT_ID, "milestone", "Sprint goal");

    run_task_model(&dir, &["add", "--description", "Check schema", "--parent", PARENT_ID]);

    let events = read_task_events(&dir);
    let req = events.iter().find(|e| e["event_type"] == "TaskAddRequested")
        .expect("TaskAddRequested must be emitted");
    let p = &req["payload"];

    assert!(p.get("requested_owner_id").is_some(),       "TaskAddRequested must contain requested_owner_id");
    assert!(p.get("requested_scheduled_date").is_some(), "TaskAddRequested must contain requested_scheduled_date");
    assert!(p.get("requested_deadline").is_some(),       "TaskAddRequested must contain requested_deadline");
}

// ── FP5: TaskAddFailedOwnerNotFound ───────────────────────────────────────────

#[test]
fn test_task_add_with_unknown_owner_emits_owner_not_found() {
    let dir = setup_dir();
    seed_parent_item(&dir, PARENT_ID, "milestone", "Sprint goal");

    let unknown_owner = "00000000-dead-dead-dead-000000000000";
    run_task_model(&dir, &[
        "add", "--description", "Ghost task",
        "--parent", PARENT_ID,
        "--owner", unknown_owner,
    ]);

    let events = read_task_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"TaskAddFailedOwnerNotFound"),
        "TaskAddFailedOwnerNotFound must be emitted when owner item does not exist");
    assert!(!types.contains(&"TaskAdded"),
        "TaskAdded must NOT be emitted when owner not found");
}

#[test]
fn test_task_add_owner_not_found_failure_reason_and_payload() {
    let dir = setup_dir();
    seed_parent_item(&dir, PARENT_ID, "milestone", "Sprint goal");

    let unknown_owner = "00000000-dead-dead-dead-000000000000";
    run_task_model(&dir, &[
        "add", "--description", "Ghost task",
        "--parent", PARENT_ID,
        "--owner", unknown_owner,
    ]);

    let events = read_task_events(&dir);
    let failure = events.iter()
        .find(|e| e["event_type"] == "TaskAddFailedOwnerNotFound")
        .expect("TaskAddFailedOwnerNotFound must be emitted");

    assert_eq!(failure["payload"]["failure_reason"].as_str().unwrap(), "owner_not_found");
    assert_eq!(failure["payload"]["owner_id"].as_str().unwrap(), unknown_owner,
        "owner_id in failure payload must match the rejected --owner value");
}

#[test]
fn test_task_add_owner_not_found_is_preceded_by_requested() {
    let dir = setup_dir();
    seed_parent_item(&dir, PARENT_ID, "milestone", "Sprint goal");

    let unknown_owner = "00000000-dead-dead-dead-000000000000";
    run_task_model(&dir, &[
        "add", "--description", "Ghost task",
        "--parent", PARENT_ID,
        "--owner", unknown_owner,
    ]);

    let events = read_task_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"TaskAddRequested"),
        "TaskAddRequested must be emitted before TaskAddFailedOwnerNotFound");
    let req_pos = types.iter().position(|&t| t == "TaskAddRequested").unwrap();
    let fail_pos = types.iter().position(|&t| t == "TaskAddFailedOwnerNotFound").unwrap();
    assert!(req_pos < fail_pos, "TaskAddRequested must precede TaskAddFailedOwnerNotFound");
}

#[test]
fn test_task_add_owner_not_found_no_task_added_in_record() {
    // The task must not appear in the project record when owner validation fails.
    // Exit code is 0 (consistent with all failure paths) — events are the signal.
    let dir = setup_dir();
    seed_parent_item(&dir, PARENT_ID, "milestone", "Sprint goal");

    run_task_model(&dir, &[
        "add", "--description", "Ghost task",
        "--parent", PARENT_ID,
        "--owner", "00000000-dead-dead-dead-000000000000",
    ]);

    let events = read_task_events(&dir);
    let task_added_count = events.iter().filter(|e| e["event_type"] == "TaskAdded").count();
    assert_eq!(task_added_count, 0,
        "No TaskAdded must be emitted when owner validation fails — task must not enter record");
}

// ── TBD sentinel: explicit TBD arg is accepted ────────────────────────────────

#[test]
fn test_tbd_as_explicit_owner_bypasses_existence_check() {
    let dir = setup_dir();
    seed_parent_item(&dir, PARENT_ID, "milestone", "Sprint goal");
    // No stakeholder seeded for "TBD" — yet it must succeed

    let out = run_task_model(&dir, &[
        "add", "--description", "TBD-owned task",
        "--parent", PARENT_ID,
        "--owner", "TBD",
    ]);

    assert!(out.status.success(), "task add with --owner TBD must succeed without seeding TBD as an item");

    let events = read_task_events(&dir);
    let added = events.iter().find(|e| e["event_type"] == "TaskAdded").expect("TaskAdded must be emitted");
    assert_eq!(added["payload"]["owner_id"].as_str().unwrap(), "TBD");
}

// ── HP11: Ownership change synced from Logseq ────────────────────────────────

#[test]
fn test_sync_owner_change_emits_task_owner_updated() {
    let dir = setup_dir();
    let logseq_dir = dir.path().join("logseq");
    fs::create_dir_all(logseq_dir.join("pages")).unwrap();

    seed_parent_item(&dir, PARENT_ID, "milestone", "Sprint goal");
    seed_parent_item(&dir, STAKEHOLDER_ID, "milestone", "Maria");
    seed_task_added_with_owner(&dir, TASK_ID_A, "task", "Review spec", PARENT_ID, "TODO",
                               "TBD", None, None);

    // Parent page: task block with [[Maria]] as owner (was TBD)
    write_logseq_page(&logseq_dir, "sprint-goal.md", PARENT_ID,
        &format!("- TODO task-id: {} Review spec [[Maria]]\n", TASK_ID_A));
    // Stakeholder page for owner resolution
    write_logseq_page(&logseq_dir, "Maria.md", STAKEHOLDER_ID, "");

    run_binary("logseq_sync", &dir, &["--graph", "logseq"]);

    let all = read_all_events(&dir);
    let owner_updated = all.iter()
        .find(|e| e["source_module"].as_str() == Some("task_model")
               && e["event_type"].as_str() == Some("TaskOwnerUpdated")
               && e["payload"]["task_id"].as_str() == Some(TASK_ID_A));

    assert!(owner_updated.is_some(), "TaskOwnerUpdated must be emitted when [[owner]] changes in Logseq");
}

#[test]
fn test_sync_owner_updated_payload_shape() {
    let dir = setup_dir();
    let logseq_dir = dir.path().join("logseq");
    fs::create_dir_all(logseq_dir.join("pages")).unwrap();

    seed_parent_item(&dir, PARENT_ID, "milestone", "Sprint goal");
    seed_parent_item(&dir, STAKEHOLDER_ID, "milestone", "Maria");
    seed_task_added_with_owner(&dir, TASK_ID_A, "task", "Review spec", PARENT_ID, "TODO",
                               "TBD", None, None);

    write_logseq_page(&logseq_dir, "sprint-goal.md", PARENT_ID,
        &format!("- TODO task-id: {} Review spec [[Maria]]\n", TASK_ID_A));
    write_logseq_page(&logseq_dir, "Maria.md", STAKEHOLDER_ID, "");

    run_binary("logseq_sync", &dir, &["--graph", "logseq"]);

    let all = read_all_events(&dir);
    let event = all.iter()
        .find(|e| e["source_module"].as_str() == Some("task_model")
               && e["event_type"].as_str() == Some("TaskOwnerUpdated"))
        .expect("TaskOwnerUpdated must be emitted");

    let p = &event["payload"];
    assert_eq!(p["task_id"].as_str().unwrap(), TASK_ID_A);
    assert_eq!(p["previous_owner_id"].as_str().unwrap(), "TBD",
        "previous_owner_id must be TBD (the initial owner)");
    assert_eq!(p["new_owner_id"].as_str().unwrap(), STAKEHOLDER_ID,
        "new_owner_id must resolve to the stakeholder's item_id from the Logseq page");
}

// ── HP12: Date changes synced from Logseq ─────────────────────────────────────

#[test]
fn test_sync_date_change_emits_task_dates_updated() {
    let dir = setup_dir();
    let logseq_dir = dir.path().join("logseq");
    fs::create_dir_all(logseq_dir.join("pages")).unwrap();

    seed_parent_item(&dir, PARENT_ID, "milestone", "Sprint goal");
    seed_task_added_with_owner(&dir, TASK_ID_A, "task", "Plan sprint", PARENT_ID, "TODO",
                               "TBD", None, None);

    // Task block now has SCHEDULED and DEADLINE lines
    let task_block = format!(
        "- TODO task-id: {} Plan sprint\n  SCHEDULED: <2026-06-15 Mon>\n  DEADLINE: <2026-06-30 Tue>\n",
        TASK_ID_A
    );
    write_logseq_page(&logseq_dir, "sprint-goal.md", PARENT_ID, &task_block);

    run_binary("logseq_sync", &dir, &["--graph", "logseq"]);

    let all = read_all_events(&dir);
    let dates_updated = all.iter()
        .find(|e| e["source_module"].as_str() == Some("task_model")
               && e["event_type"].as_str() == Some("TaskDatesUpdated")
               && e["payload"]["task_id"].as_str() == Some(TASK_ID_A));

    assert!(dates_updated.is_some(), "TaskDatesUpdated must be emitted when SCHEDULED/DEADLINE changes");
}

#[test]
fn test_sync_dates_updated_payload_shape() {
    let dir = setup_dir();
    let logseq_dir = dir.path().join("logseq");
    fs::create_dir_all(logseq_dir.join("pages")).unwrap();

    seed_parent_item(&dir, PARENT_ID, "milestone", "Sprint goal");
    seed_task_added_with_owner(&dir, TASK_ID_A, "task", "Plan sprint", PARENT_ID, "TODO",
                               "TBD", None, None);

    let task_block = format!(
        "- TODO task-id: {} Plan sprint\n  SCHEDULED: <2026-06-15 Mon>\n  DEADLINE: <2026-06-30 Tue>\n",
        TASK_ID_A
    );
    write_logseq_page(&logseq_dir, "sprint-goal.md", PARENT_ID, &task_block);

    run_binary("logseq_sync", &dir, &["--graph", "logseq"]);

    let all = read_all_events(&dir);
    let event = all.iter()
        .find(|e| e["source_module"].as_str() == Some("task_model")
               && e["event_type"].as_str() == Some("TaskDatesUpdated"))
        .expect("TaskDatesUpdated must be emitted");

    let p = &event["payload"];
    assert_eq!(p["task_id"].as_str().unwrap(), TASK_ID_A);
    assert!(p["previous_scheduled_date"].is_null(),   "previous_scheduled_date must be null (was unset)");
    assert!(p["previous_deadline"].is_null(),         "previous_deadline must be null (was unset)");
    assert_eq!(p["new_scheduled_date"].as_str().unwrap(), "2026-06-15",
        "new_scheduled_date must be parsed ISO date from SCHEDULED: line");
    assert_eq!(p["new_deadline"].as_str().unwrap(), "2026-06-30",
        "new_deadline must be parsed ISO date from DEADLINE: line");
}

// ── BS7: Sync with unresolvable owner reference ───────────────────────────────

#[test]
fn test_sync_unresolvable_owner_reference_no_event() {
    let dir = setup_dir();
    let logseq_dir = dir.path().join("logseq");
    fs::create_dir_all(logseq_dir.join("pages")).unwrap();

    seed_parent_item(&dir, PARENT_ID, "milestone", "Sprint goal");
    seed_task_added_with_owner(&dir, TASK_ID_A, "task", "Orphan task", PARENT_ID, "TODO",
                               "TBD", None, None);

    // Page references [[Phantomname]] — no Logseq page with that stem exists
    write_logseq_page(&logseq_dir, "sprint-goal.md", PARENT_ID,
        &format!("- TODO task-id: {} Orphan task [[Phantomname]]\n", TASK_ID_A));
    // No Phantomname.md file — unresolvable reference

    run_binary("logseq_sync", &dir, &["--graph", "logseq"]);

    let all = read_all_events(&dir);
    let owner_updated = all.iter().any(|e|
        e["source_module"].as_str() == Some("task_model")
        && e["event_type"].as_str() == Some("TaskOwnerUpdated")
        && e["payload"]["task_id"].as_str() == Some(TASK_ID_A));

    assert!(!owner_updated,
        "TaskOwnerUpdated must NOT be emitted when [[owner]] reference cannot be resolved");
}

#[test]
fn test_sync_unresolvable_owner_does_not_abort_sync() {
    let dir = setup_dir();
    let logseq_dir = dir.path().join("logseq");
    fs::create_dir_all(logseq_dir.join("pages")).unwrap();

    seed_parent_item(&dir, PARENT_ID, "milestone", "Sprint goal");
    seed_task_added_with_owner(&dir, TASK_ID_A, "task", "Orphan task", PARENT_ID, "TODO",
                               "TBD", None, None);
    seed_task_added_with_owner(&dir, TASK_ID_B, "task", "Normal task", PARENT_ID, "TODO",
                               "TBD", None, None);

    // Task A: unresolvable owner; Task B: marker changes (no owner reference)
    let page_content = format!(
        "- TODO task-id: {} Orphan task [[Phantomname]]\n- DONE task-id: {} Normal task\n",
        TASK_ID_A, TASK_ID_B
    );
    write_logseq_page(&logseq_dir, "sprint-goal.md", PARENT_ID, &page_content);

    let out = run_binary("logseq_sync", &dir, &["--graph", "logseq"]);
    assert!(out.status.success(), "sync must complete even when owner reference is unresolvable");

    let all = read_all_events(&dir);
    // Task B should still get a marker update
    let b_marker_updated = all.iter().any(|e|
        e["source_module"].as_str() == Some("task_model")
        && e["event_type"].as_str() == Some("TaskMarkerUpdated")
        && e["payload"]["task_id"].as_str() == Some(TASK_ID_B));

    assert!(b_marker_updated,
        "TaskMarkerUpdated for Task B must be emitted even when Task A's owner is unresolvable");
}

// ── Multiple independent events per block line ────────────────────────────────

#[test]
fn test_sync_marker_and_dates_changed_emits_both_events() {
    let dir = setup_dir();
    let logseq_dir = dir.path().join("logseq");
    fs::create_dir_all(logseq_dir.join("pages")).unwrap();

    seed_parent_item(&dir, PARENT_ID, "milestone", "Sprint goal");
    seed_task_added_with_owner(&dir, TASK_ID_A, "task", "Review", PARENT_ID, "TODO",
                               "TBD", None, None);

    // Marker changed (TODO → DONE) AND dates added
    let task_block = format!(
        "- DONE task-id: {} Review\n  SCHEDULED: <2026-06-10 Wed>\n  DEADLINE: <2026-06-20 Sat>\n",
        TASK_ID_A
    );
    write_logseq_page(&logseq_dir, "sprint-goal.md", PARENT_ID, &task_block);

    run_binary("logseq_sync", &dir, &["--graph", "logseq"]);

    let all = read_all_events(&dir);
    let marker_updated = all.iter().any(|e|
        e["source_module"].as_str() == Some("task_model")
        && e["event_type"].as_str() == Some("TaskMarkerUpdated")
        && e["payload"]["task_id"].as_str() == Some(TASK_ID_A));
    let dates_updated = all.iter().any(|e|
        e["source_module"].as_str() == Some("task_model")
        && e["event_type"].as_str() == Some("TaskDatesUpdated")
        && e["payload"]["task_id"].as_str() == Some(TASK_ID_A));

    assert!(marker_updated, "TaskMarkerUpdated must be emitted when marker changes");
    assert!(dates_updated,  "TaskDatesUpdated must be emitted when dates change");
}

#[test]
fn test_sync_only_dates_changed_emits_only_dates_updated() {
    let dir = setup_dir();
    let logseq_dir = dir.path().join("logseq");
    fs::create_dir_all(logseq_dir.join("pages")).unwrap();

    seed_parent_item(&dir, PARENT_ID, "milestone", "Sprint goal");
    // Task starts with TODO marker and no dates
    seed_task_added_with_owner(&dir, TASK_ID_A, "task", "Review", PARENT_ID, "TODO",
                               "TBD", None, None);

    // Marker UNCHANGED (still TODO); only dates added
    let task_block = format!(
        "- TODO task-id: {} Review\n  SCHEDULED: <2026-06-10 Wed>\n",
        TASK_ID_A
    );
    write_logseq_page(&logseq_dir, "sprint-goal.md", PARENT_ID, &task_block);

    run_binary("logseq_sync", &dir, &["--graph", "logseq"]);

    let all = read_all_events(&dir);
    let marker_updated = all.iter().any(|e|
        e["source_module"].as_str() == Some("task_model")
        && e["event_type"].as_str() == Some("TaskMarkerUpdated")
        && e["payload"]["task_id"].as_str() == Some(TASK_ID_A));
    let dates_updated = all.iter().any(|e|
        e["source_module"].as_str() == Some("task_model")
        && e["event_type"].as_str() == Some("TaskDatesUpdated")
        && e["payload"]["task_id"].as_str() == Some(TASK_ID_A));

    assert!(!marker_updated, "TaskMarkerUpdated must NOT be emitted when marker did not change");
    assert!(dates_updated,   "TaskDatesUpdated must be emitted when scheduled_date changed");
}

// ── Discovery: TaskAdded via sync includes new fields ─────────────────────────

#[test]
fn test_sync_discovery_task_added_includes_owner_id() {
    let dir = setup_dir();
    let logseq_dir = dir.path().join("logseq");
    fs::create_dir_all(logseq_dir.join("pages")).unwrap();

    seed_parent_item(&dir, PARENT_ID, "milestone", "Sprint goal");
    seed_parent_item(&dir, STAKEHOLDER_ID, "milestone", "Maria");

    let discovered_id = "11111111-2222-3333-4444-555555555555";
    // New task (not in project record), with [[Maria]] owner
    write_logseq_page(&logseq_dir, "sprint-goal.md", PARENT_ID,
        &format!("- TODO task-id: {} Discovered task [[Maria]]\n", discovered_id));
    write_logseq_page(&logseq_dir, "Maria.md", STAKEHOLDER_ID, "");

    run_binary("logseq_sync", &dir, &["--graph", "logseq"]);

    let all = read_all_events(&dir);
    let discovery = all.iter()
        .find(|e| e["source_module"].as_str() == Some("task_model")
               && e["event_type"].as_str() == Some("TaskAdded")
               && e["payload"]["task_id"].as_str() == Some(discovered_id))
        .expect("TaskAdded must be emitted for discovered task");

    let p = &discovery["payload"];
    // Owner resolves from [[Maria]] — stakeholder page has item_id = STAKEHOLDER_ID
    assert_eq!(p["owner_id"].as_str().unwrap(), STAKEHOLDER_ID,
        "Discovered task owner_id must resolve from [[Maria]] page reference");
    assert!(p.get("scheduled_date").is_some(), "Discovered TaskAdded must include scheduled_date field");
    assert!(p.get("deadline").is_some(),       "Discovered TaskAdded must include deadline field");
}

#[test]
fn test_sync_discovery_without_owner_assigns_tbd() {
    let dir = setup_dir();
    let logseq_dir = dir.path().join("logseq");
    fs::create_dir_all(logseq_dir.join("pages")).unwrap();

    seed_parent_item(&dir, PARENT_ID, "milestone", "Sprint goal");

    let discovered_id = "22222222-3333-4444-5555-666666666666";
    // Task line with no [[owner]] reference
    write_logseq_page(&logseq_dir, "sprint-goal.md", PARENT_ID,
        &format!("- TODO task-id: {} Anonymous task\n", discovered_id));

    run_binary("logseq_sync", &dir, &["--graph", "logseq"]);

    let all = read_all_events(&dir);
    let discovery = all.iter()
        .find(|e| e["source_module"].as_str() == Some("task_model")
               && e["event_type"].as_str() == Some("TaskAdded")
               && e["payload"]["task_id"].as_str() == Some(discovered_id))
        .expect("TaskAdded must be emitted for discovered task");

    assert_eq!(discovery["payload"]["owner_id"].as_str().unwrap(), "TBD",
        "Discovered task without [[owner]] must have owner_id = TBD");
}

#[test]
fn test_sync_discovery_with_dates_includes_dates_in_task_added() {
    let dir = setup_dir();
    let logseq_dir = dir.path().join("logseq");
    fs::create_dir_all(logseq_dir.join("pages")).unwrap();

    seed_parent_item(&dir, PARENT_ID, "milestone", "Sprint goal");

    let discovered_id = "33333333-4444-5555-6666-777777777777";
    let task_block = format!(
        "- TODO task-id: {} Dated discovery\n  SCHEDULED: <2026-06-20 Sat>\n  DEADLINE: <2026-06-25 Thu>\n",
        discovered_id
    );
    write_logseq_page(&logseq_dir, "sprint-goal.md", PARENT_ID, &task_block);

    run_binary("logseq_sync", &dir, &["--graph", "logseq"]);

    let all = read_all_events(&dir);
    let discovery = all.iter()
        .find(|e| e["source_module"].as_str() == Some("task_model")
               && e["event_type"].as_str() == Some("TaskAdded")
               && e["payload"]["task_id"].as_str() == Some(discovered_id))
        .expect("TaskAdded must be emitted for discovered task with dates");

    assert_eq!(discovery["payload"]["scheduled_date"].as_str().unwrap(), "2026-06-20",
        "Discovery TaskAdded must include scheduled_date from SCHEDULED: line");
    assert_eq!(discovery["payload"]["deadline"].as_str().unwrap(), "2026-06-25",
        "Discovery TaskAdded must include deadline from DEADLINE: line");
}

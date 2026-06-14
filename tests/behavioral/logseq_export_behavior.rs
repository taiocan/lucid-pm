//! Behavioral tests for logseq_export.
//!
//! Tests verify observable outcomes: events emitted, payload shapes, ordering,
//! page files written on disk, and idempotency.
//! All assertions reference event names from events/logseq_export_schema.md exactly.

use serde_json::{json, Value};
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};
use tempfile::TempDir;

fn binary_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_logseq_export"))
}

fn setup_temp_dir() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("events")).unwrap();
    dir
}

fn seed_incorporated_items(
    dir: &TempDir,
    session_id: &str,
    items: &[(&str, &str, &str, Option<&str>, Option<&str>)],
) {
    let items_json: Vec<Value> = items
        .iter()
        .map(|(id, typ, desc, ps, pp)| {
            json!({
                "item_id": id,
                "item_type": typ,
                "description": desc,
                "uncertain": false,
                "uncertainty_reason": null,
                "proposed_status": ps,
                "proposed_priority": pp,
            })
        })
        .collect();

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
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .unwrap();
    writeln!(file, "{}", items_extracted).unwrap();
    writeln!(file, "{}", extraction_confirmed).unwrap();
    writeln!(file, "{}", items_incorporated).unwrap();
}

fn run_binary(dir: &TempDir, output_dir: &str) -> std::process::Output {
    Command::new(binary_path())
        .current_dir(dir.path())
        .args(["--output-dir", output_dir])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("Failed to run binary")
}

fn read_le_events(dir: &TempDir) -> Vec<Value> {
    let path = dir.path().join("events/runtime_events.jsonl");
    if !path.exists() {
        return vec![];
    }
    fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .filter(|e| e["source_module"].as_str() == Some("logseq_export"))
        .collect()
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

fn page_path(output_dir: &str, slug: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(output_dir)
        .join("pages")
        .join(format!("{}.md", slug))
}

const SESSION_A: &str = "a4ca3a7e-61eb-4f36-b59e-f3abd166e351";
const ITEM_TASK: &str = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeee01";
const ITEM_RISK: &str = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeee02";

// ── Happy Path: Successful Export ─────────────────────────────────────────────

#[test]
fn test_export_emits_requested_then_completed() {
    let dir = setup_temp_dir();
    seed_incorporated_items(
        &dir,
        SESSION_A,
        &[
            (ITEM_TASK, "task", "Deploy API by Friday", Some("todo"), Some("high")),
            (ITEM_RISK, "risk", "Vendor delay risk", Some("open"), Some("medium")),
        ],
    );
    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();

    run_binary(&dir, &output_dir);

    let events = read_le_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"ExportRequested"),  "ExportRequested must be emitted");
    assert!(types.contains(&"ExportCompleted"),  "ExportCompleted must be emitted");
    assert!(!types.contains(&"ExportFailedEmptyRecord"),      "must NOT emit EmptyRecord failure");
    assert!(!types.contains(&"ExportFailedOutputUnavailable"),"must NOT emit OutputUnavailable failure");
    assert!(!types.contains(&"ExportFailedRecordUnreadable"), "must NOT emit RecordUnreadable failure");

    let req_pos = types.iter().position(|&t| t == "ExportRequested").unwrap();
    let cmp_pos = types.iter().position(|&t| t == "ExportCompleted").unwrap();
    assert!(req_pos < cmp_pos, "ExportRequested must precede ExportCompleted");
}

#[test]
fn test_export_completed_payload_shape() {
    let dir = setup_temp_dir();
    seed_incorporated_items(
        &dir,
        SESSION_A,
        &[
            (ITEM_TASK, "task", "Deploy API by Friday", Some("todo"), Some("high")),
            (ITEM_RISK, "risk", "Vendor delay risk", Some("open"), Some("medium")),
        ],
    );
    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();

    run_binary(&dir, &output_dir);

    let events = read_le_events(&dir);
    let completed = events
        .iter()
        .find(|e| e["event_type"] == "ExportCompleted")
        .expect("ExportCompleted not found");

    let p = &completed["payload"];
    assert!(p["output_dir"].as_str().is_some(), "output_dir must be present");
    assert_eq!(p["item_count"].as_u64().unwrap(), 2, "item_count must equal number of items");
    let pages = p["pages_written"].as_array().expect("pages_written must be an array");
    assert_eq!(pages.len(), 2, "pages_written must list one path per item");
}

#[test]
fn test_export_writes_one_page_per_item() {
    let dir = setup_temp_dir();
    seed_incorporated_items(
        &dir,
        SESSION_A,
        &[
            (ITEM_TASK, "task", "Deploy API by Friday", None, None),
            (ITEM_RISK, "risk", "Vendor delay risk", None, None),
        ],
    );
    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();

    run_binary(&dir, &output_dir);

    assert!(page_path(&output_dir, &description_to_slug("Deploy API by Friday")).exists(), "Page for ITEM_TASK must exist on disk");
    assert!(page_path(&output_dir, &description_to_slug("Vendor delay risk")).exists(), "Page for ITEM_RISK must exist on disk");
}

#[test]
fn test_export_page_contains_status_and_priority() {
    let dir = setup_temp_dir();
    seed_incorporated_items(
        &dir,
        SESSION_A,
        &[(ITEM_TASK, "task", "Deploy API by Friday", Some("doing"), Some("high"))],
    );
    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();

    run_binary(&dir, &output_dir);

    let content = fs::read_to_string(page_path(&output_dir, &description_to_slug("Deploy API by Friday")))
        .expect("Page file must be readable");

    assert!(content.contains("status:: doing"),   "Page must contain status:: doing");
    assert!(content.contains("priority:: high"),  "Page must contain priority:: high");
}

#[test]
fn test_export_page_contains_tags_property_for_type() {
    let dir = setup_temp_dir();
    seed_incorporated_items(
        &dir,
        SESSION_A,
        &[(ITEM_TASK, "task", "Deploy API by Friday", None, None)],
    );
    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();

    run_binary(&dir, &output_dir);

    let content = fs::read_to_string(page_path(&output_dir, &description_to_slug("Deploy API by Friday")))
        .expect("Page file must be readable");

    assert!(content.contains("tags:: task"),
        "Page must contain 'tags:: task' for type-based navigation via Logseq tags");
}

#[test]
fn test_export_page_uses_slug_filename_and_canonical_format() {
    let dir = setup_temp_dir();
    seed_incorporated_items(
        &dir,
        SESSION_A,
        &[(ITEM_TASK, "task", "Deploy API by Friday", None, None)],
    );
    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();

    run_binary(&dir, &output_dir);

    // Page must be written with slug filename, not UUID filename
    let slug_path = page_path(&output_dir, "deploy-api-by-friday");
    assert!(slug_path.exists(), "Page must exist at slug-named path 'deploy-api-by-friday.md'");

    let uuid_path = page_path(&output_dir, ITEM_TASK);
    assert!(!uuid_path.exists(), "Page must NOT exist at UUID-named path");

    let content = fs::read_to_string(&slug_path).expect("Page file must be readable");

    assert!(
        content.starts_with("type:: task"),
        "Page must start with 'type:: <type>' — no title:: property in canonical format"
    );
    assert!(content.contains("tags:: task"),   "Page must contain 'tags:: task'");
    assert!(
        content.contains(&format!("- item-id: {}", ITEM_TASK)),
        "Page must contain plain-text '- item-id: <uuid>' bullet"
    );
    assert!(
        !content.contains("title:: "),
        "Page must NOT contain 'title::' — filename is the page title in Logseq"
    );
    assert!(
        !content.contains("item-id:: "),
        "Page must NOT use Logseq property syntax 'item-id::' — it creates a noise index page"
    );
}

#[test]
fn test_export_page_not_set_status_and_priority_when_absent() {
    let dir = setup_temp_dir();
    seed_incorporated_items(
        &dir,
        SESSION_A,
        &[(ITEM_TASK, "task", "Deploy API by Friday", None, None)],
    );
    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();

    run_binary(&dir, &output_dir);

    let content = fs::read_to_string(page_path(&output_dir, &description_to_slug("Deploy API by Friday")))
        .expect("Page file must be readable");

    assert!(content.contains("status:: not-set"),   "Page must show status:: not-set when no status recorded");
    assert!(content.contains("priority:: not-set"), "Page must show priority:: not-set when no priority recorded");
}

#[test]
fn test_export_does_not_modify_event_log_content() {
    let dir = setup_temp_dir();
    seed_incorporated_items(
        &dir,
        SESSION_A,
        &[(ITEM_TASK, "task", "Deploy API by Friday", None, None)],
    );
    let events_path = dir.path().join("events/runtime_events.jsonl");
    let before = fs::read_to_string(&events_path).unwrap();
    let line_count_before = before.lines().count();

    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();
    run_binary(&dir, &output_dir);

    let after = fs::read_to_string(&events_path).unwrap();
    let existing_lines: Vec<&str> = after.lines().collect();

    for (i, line) in before.lines().enumerate() {
        assert_eq!(existing_lines[i], line,
            "Line {} of events file must not be modified by export", i + 1);
    }
    assert!(
        after.lines().count() > line_count_before,
        "Export must append new events; original lines must be unchanged"
    );
}

// ── Happy Path: Idempotent Re-export ─────────────────────────────────────────

#[test]
fn test_re_export_produces_identical_page_content() {
    let dir = setup_temp_dir();
    seed_incorporated_items(
        &dir,
        SESSION_A,
        &[
            (ITEM_TASK, "task", "Deploy API by Friday", Some("todo"), Some("high")),
        ],
    );
    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();

    run_binary(&dir, &output_dir);
    let content_first = fs::read_to_string(page_path(&output_dir, "deploy-api-by-friday")).unwrap();

    run_binary(&dir, &output_dir);
    let content_second = fs::read_to_string(page_path(&output_dir, "deploy-api-by-friday")).unwrap();

    assert_eq!(content_first, content_second,
        "Re-export of the same project state must produce identical page content");
}

#[test]
fn test_re_export_emits_export_completed_each_time() {
    let dir = setup_temp_dir();
    seed_incorporated_items(
        &dir,
        SESSION_A,
        &[(ITEM_TASK, "task", "Deploy API by Friday", None, None)],
    );
    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();

    run_binary(&dir, &output_dir);
    run_binary(&dir, &output_dir);

    let events = read_le_events(&dir);
    let completed_count = events
        .iter()
        .filter(|e| e["event_type"] == "ExportCompleted")
        .count();

    assert_eq!(completed_count, 2,
        "Each export run must emit exactly one ExportCompleted event");
}

// ── Stale page cleanup ────────────────────────────────────────────────────────

#[test]
fn test_stale_pages_deleted_on_re_export() {
    let dir = setup_temp_dir();
    // First export: one item
    seed_incorporated_items(
        &dir,
        SESSION_A,
        &[(ITEM_TASK, "task", "Deploy API by Friday", None, None)],
    );
    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();
    run_binary(&dir, &output_dir);

    let slug_path = page_path(&output_dir, "deploy-api-by-friday");
    assert!(slug_path.exists(), "Page must exist after first export");

    // Plant a stale page (simulates a UUID-named leftover or old slug)
    let stale_path = page_path(&output_dir, "some-old-page");
    fs::write(&stale_path, "type:: task\nstatus:: not-set\n").unwrap();

    // Re-export same state: stale page must be deleted
    run_binary(&dir, &output_dir);

    assert!(slug_path.exists(),   "Current item page must still exist after re-export");
    assert!(!stale_path.exists(), "Stale page must be deleted by re-export");
}

// ── Failure Path 1: EmptyProjectRecord ───────────────────────────────────────

#[test]
fn test_empty_record_emits_failure_event() {
    let dir = setup_temp_dir();
    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();

    run_binary(&dir, &output_dir);

    let events = read_le_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"ExportRequested"),          "ExportRequested must be emitted");
    assert!(types.contains(&"ExportFailedEmptyRecord"),  "ExportFailedEmptyRecord must be emitted");
    assert!(!types.contains(&"ExportCompleted"),          "ExportCompleted must NOT be emitted");
}

#[test]
fn test_empty_record_failure_payload() {
    let dir = setup_temp_dir();
    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();

    run_binary(&dir, &output_dir);

    let events = read_le_events(&dir);
    let failure = events
        .iter()
        .find(|e| e["event_type"] == "ExportFailedEmptyRecord")
        .expect("ExportFailedEmptyRecord not found");

    assert_eq!(
        failure["payload"]["failure_reason"].as_str().unwrap(),
        "empty_project_record"
    );
}

#[test]
fn test_empty_record_writes_no_pages() {
    let dir = setup_temp_dir();
    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();

    run_binary(&dir, &output_dir);

    let pages_dir = std::path::PathBuf::from(&output_dir).join("pages");
    if pages_dir.exists() {
        let count = fs::read_dir(&pages_dir).unwrap().count();
        assert_eq!(count, 0, "No pages must be written when record is empty");
    }
}

// ── Failure Path 2: OutputDirectoryNotAccessible ─────────────────────────────

#[test]
fn test_output_dir_not_accessible_emits_failure() {
    let dir = setup_temp_dir();
    seed_incorporated_items(
        &dir,
        SESSION_A,
        &[(ITEM_TASK, "task", "Deploy API by Friday", None, None)],
    );

    let locked = dir.path().join("locked");
    fs::create_dir(&locked).unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o444)).unwrap();
    let output_dir = locked.join("output").to_string_lossy().into_owned();

    run_binary(&dir, &output_dir);

    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();

    let events = read_le_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"ExportRequested"),                "ExportRequested must be emitted");
    assert!(types.contains(&"ExportFailedOutputUnavailable"),  "ExportFailedOutputUnavailable must be emitted");
    assert!(!types.contains(&"ExportCompleted"),                "ExportCompleted must NOT be emitted");
}

#[test]
fn test_output_dir_not_accessible_failure_payload() {
    let dir = setup_temp_dir();
    seed_incorporated_items(
        &dir,
        SESSION_A,
        &[(ITEM_TASK, "task", "Deploy API by Friday", None, None)],
    );

    let locked = dir.path().join("locked2");
    fs::create_dir(&locked).unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o444)).unwrap();
    let output_dir = locked.join("output").to_string_lossy().into_owned();

    run_binary(&dir, &output_dir);

    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();

    let events = read_le_events(&dir);
    let failure = events
        .iter()
        .find(|e| e["event_type"] == "ExportFailedOutputUnavailable")
        .expect("ExportFailedOutputUnavailable not found");

    assert_eq!(
        failure["payload"]["failure_reason"].as_str().unwrap(),
        "output_directory_not_accessible"
    );
    assert!(
        failure["payload"]["output_dir"].as_str().is_some(),
        "output_dir must be present in failure payload"
    );
}

#[test]
fn test_output_dir_failure_writes_no_partial_pages() {
    let dir = setup_temp_dir();
    seed_incorporated_items(
        &dir,
        SESSION_A,
        &[(ITEM_TASK, "task", "Deploy API by Friday", None, None)],
    );

    let locked = dir.path().join("locked3");
    fs::create_dir(&locked).unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o444)).unwrap();
    let output_dir = locked.join("output").to_string_lossy().into_owned();

    run_binary(&dir, &output_dir);

    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();

    let pages_dir = std::path::PathBuf::from(&output_dir).join("pages");
    assert!(!pages_dir.exists(),
        "No pages directory must exist after OutputDirectoryNotAccessible failure");
}

// ── Failure Path 3: ProjectRecordUnreadable ───────────────────────────────────

#[test]
fn test_record_unreadable_emits_failure() {
    let dir = setup_temp_dir();
    let events_path = dir.path().join("events/runtime_events.jsonl");
    fs::write(&events_path, b"this is not valid json\n").unwrap();

    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();
    run_binary(&dir, &output_dir);

    let content = fs::read_to_string(&events_path).unwrap();
    let le_events: Vec<Value> = content
        .lines()
        .filter(|l| !l.is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .filter(|e: &Value| e["source_module"].as_str() == Some("logseq_export"))
        .collect();

    let types: Vec<&str> = le_events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"ExportRequested"),               "ExportRequested must be emitted");
    assert!(types.contains(&"ExportFailedRecordUnreadable"),  "ExportFailedRecordUnreadable must be emitted");
    assert!(!types.contains(&"ExportCompleted"),               "ExportCompleted must NOT be emitted");
}

#[test]
fn test_record_unreadable_failure_payload() {
    let dir = setup_temp_dir();
    let events_path = dir.path().join("events/runtime_events.jsonl");
    fs::write(&events_path, b"not json\n").unwrap();

    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();
    run_binary(&dir, &output_dir);

    let content = fs::read_to_string(&events_path).unwrap();
    let le_events: Vec<Value> = content
        .lines()
        .filter(|l| !l.is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .filter(|e: &Value| e["source_module"].as_str() == Some("logseq_export"))
        .collect();

    let failure = le_events
        .iter()
        .find(|e| e["event_type"] == "ExportFailedRecordUnreadable")
        .expect("ExportFailedRecordUnreadable not found");

    assert_eq!(
        failure["payload"]["failure_reason"].as_str().unwrap(),
        "project_record_unreadable"
    );
    assert!(
        failure["payload"]["error_detail"].as_str().is_some(),
        "error_detail must be present in failure payload"
    );
}

// ── Telemetry ─────────────────────────────────────────────────────────────────

#[test]
fn test_all_events_have_required_base_fields() {
    let dir = setup_temp_dir();
    seed_incorporated_items(
        &dir,
        SESSION_A,
        &[(ITEM_TASK, "task", "Deploy API by Friday", None, None)],
    );
    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();

    run_binary(&dir, &output_dir);

    let events = read_le_events(&dir);
    assert!(!events.is_empty());

    for event in &events {
        let t = event["event_type"].as_str().unwrap_or("unknown");
        assert!(!event["event_id"].is_null(),        "{}: event_id must be present", t);
        assert!(!event["event_type"].is_null(),      "{}: event_type must be present", t);
        assert!(!event["timestamp"].is_null(),       "{}: timestamp must be present", t);
        assert!(!event["correlation_id"].is_null(),  "{}: correlation_id must be present", t);
        assert!(!event["source_module"].is_null(),   "{}: source_module must be present", t);
        assert!(!event["payload"].is_null(),         "{}: payload must be present", t);
        assert_eq!(
            event["source_module"].as_str().unwrap(),
            "logseq_export",
            "{}: source_module must be 'logseq_export'",
            t
        );
        assert!(
            event["timestamp"].as_u64().unwrap() > 0,
            "{}: timestamp must be positive",
            t
        );
    }
}

#[test]
fn test_correlation_id_consistent_within_one_invocation() {
    let dir = setup_temp_dir();
    seed_incorporated_items(
        &dir,
        SESSION_A,
        &[(ITEM_TASK, "task", "Deploy API by Friday", None, None)],
    );
    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();

    run_binary(&dir, &output_dir);

    let events = read_le_events(&dir);
    assert!(events.len() >= 2);

    let first_cid = events[0]["correlation_id"].as_str().unwrap();
    for event in &events {
        assert_eq!(
            event["correlation_id"].as_str().unwrap(),
            first_cid,
            "All events from one invocation must share the same correlation_id"
        );
    }
}

#[test]
fn test_separate_invocations_have_different_correlation_ids() {
    let dir = setup_temp_dir();
    seed_incorporated_items(
        &dir,
        SESSION_A,
        &[(ITEM_TASK, "task", "Deploy API by Friday", None, None)],
    );
    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();

    run_binary(&dir, &output_dir);
    run_binary(&dir, &output_dir);

    let events = read_le_events(&dir);
    let cids: Vec<&str> = events
        .iter()
        .filter(|e| e["event_type"] == "ExportRequested")
        .map(|e| e["correlation_id"].as_str().unwrap())
        .collect();

    assert_eq!(cids.len(), 2);
    assert_ne!(cids[0], cids[1],
        "Different invocations must produce different correlation_ids");
}

// ── R_export_format: task block format and work package page properties ───────

const WP_SCHEMA: &str = r#"schemaVersion: 1
statuses:
  todo:
  doing:
  done:
  active:
  inactive:
pageTypes:
  work_package:
    allowedStatuses: [todo, doing, done]
  stakeholder:
    allowedStatuses: [active, inactive]
blockTypes:
  task_block:
    markers:
      TODO: todo
      DOING: doing
      DONE: done
relations:
  assigned_to:
    source: [work_package]
    target: [stakeholder]
  blocks:
    source: [work_package]
    target: [work_package]
renderers:
  logseq:
    relations:
      assigned_to:
        forwardLabel: "Assigned To"
        inverseLabel: "Owns"
      blocks:
        forwardLabel: "Blocking"
        inverseLabel: "Blocked By"
"#;

const ITEM_WP:   &str = "00000001-0000-0000-0000-000000000001";
const ITEM_WP2:  &str = "00000001-0000-0000-0000-000000000002";
const ITEM_SH:   &str = "00000001-0000-0000-0000-000000000003";
const TASK_ID_1: &str = "10000001-0000-0000-0000-000000000001";

fn write_schema_file(dir: &TempDir, yaml: &str) {
    fs::write(dir.path().join("project-schema.yaml"), yaml).unwrap();
}

fn run_binary_isolated(dir: &TempDir, output_dir: &str) -> std::process::Output {
    Command::new(binary_path())
        .current_dir(dir.path())
        .args(["--output-dir", output_dir])
        .env("HOME", dir.path())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("Failed to run binary")
}

fn seed_task_added(
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
            "description":    "Test task",
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

fn seed_item_linked(dir: &TempDir, source_id: &str, link_type: &str, target_id: &str) {
    let event = json!({
        "event_id":       format!("seed-link-{}-{}", &source_id[..8], &target_id[..8]),
        "event_type":     "ItemLinked",
        "timestamp":      1748000011000u64,
        "correlation_id": "00000000-0000-0000-0000-000000000099",
        "source_module":  "item_links",
        "payload": {
            "source_id": source_id,
            "link_type": link_type,
            "target_id": target_id,
        }
    });
    let path = dir.path().join("events/runtime_events.jsonl");
    let mut file = fs::OpenOptions::new().create(true).append(true).open(&path).unwrap();
    writeln!(file, "{}", event).unwrap();
}

#[test]
fn test_task_block_line_has_marker_description_owner() {
    let dir = setup_temp_dir();
    write_schema_file(&dir, WP_SCHEMA);
    seed_incorporated_items(
        &dir, SESSION_A,
        &[(ITEM_WP, "work_package", "Sprint Alpha", None, None),
          (ITEM_SH, "stakeholder", "Alice Stakeholder", None, None)],
    );
    seed_task_added(&dir, TASK_ID_1, ITEM_WP, "TODO", ITEM_SH, None, None);
    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();

    run_binary_isolated(&dir, &output_dir);

    let content = fs::read_to_string(page_path(&output_dir, "sprint-alpha"))
        .expect("sprint-alpha.md must exist");
    assert!(
        content.contains("- TODO Test task [[alice-stakeholder]]"),
        "Task block line must be '- MARKER description [[owner-slug]]', got:\n{}",
        content
    );
}

#[test]
fn test_task_block_has_properties_drawer_with_task_id() {
    let dir = setup_temp_dir();
    write_schema_file(&dir, WP_SCHEMA);
    seed_incorporated_items(
        &dir, SESSION_A,
        &[(ITEM_WP, "work_package", "Sprint Alpha", None, None),
          (ITEM_SH, "stakeholder", "Alice Stakeholder", None, None)],
    );
    seed_task_added(&dir, TASK_ID_1, ITEM_WP, "TODO", ITEM_SH, None, None);
    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();

    run_binary_isolated(&dir, &output_dir);

    let content = fs::read_to_string(page_path(&output_dir, "sprint-alpha"))
        .expect("sprint-alpha.md must exist");
    assert!(content.contains("  :PROPERTIES:"), "Task block must contain :PROPERTIES:");
    assert!(
        content.contains(&format!("  :task-id: {}", TASK_ID_1)),
        "Task block must contain :task-id: <uuid>"
    );
    assert!(content.contains("  :END:"), "Task block must contain :END:");
}

#[test]
fn test_task_block_has_no_tasks_section_header() {
    let dir = setup_temp_dir();
    write_schema_file(&dir, WP_SCHEMA);
    seed_incorporated_items(
        &dir, SESSION_A,
        &[(ITEM_WP, "work_package", "Sprint Alpha", None, None)],
    );
    seed_task_added(&dir, TASK_ID_1, ITEM_WP, "TODO", "TBD", None, None);
    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();

    run_binary_isolated(&dir, &output_dir);

    let content = fs::read_to_string(page_path(&output_dir, "sprint-alpha"))
        .expect("sprint-alpha.md must exist");
    assert!(
        !content.contains("- Tasks\n"),
        "New task block format must NOT include a '- Tasks' section header"
    );
}

#[test]
fn test_task_block_tbd_owner_omits_wiki_link() {
    // R16: TBD placeholder owner → no owner reference of any kind on the block line.
    let dir = setup_temp_dir();
    write_schema_file(&dir, WP_SCHEMA);
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_WP, "work_package", "Sprint Alpha", None, None)]);
    seed_task_added(&dir, TASK_ID_1, ITEM_WP, "TODO", "TBD", None, None);
    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();

    run_binary_isolated(&dir, &output_dir);

    let content = fs::read_to_string(page_path(&output_dir, "sprint-alpha"))
        .expect("sprint-alpha.md must exist");
    assert!(
        !content.contains("[[TBD]]"),
        "TBD-owned task must NOT render [[TBD]] in the block line, got:\n{}",
        content
    );
    assert!(
        content.contains("- TODO Test task\n"),
        "TBD-owned task block line must be '- MARKER description' with no owner reference, got:\n{}",
        content
    );
}

#[test]
fn test_tbd_owner_no_wiki_link_pattern_of_any_kind() {
    // R16 falsification: TBD owner → no [[...]] pattern of any kind on the block line.
    // Falsifies: implementation that always emits some owner wiki-link (e.g. [[TBD]], [[unassigned]]).
    let dir = setup_temp_dir();
    write_schema_file(&dir, WP_SCHEMA);
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_WP, "work_package", "Sprint Alpha", None, None)]);
    seed_task_added(&dir, TASK_ID_1, ITEM_WP, "TODO", "TBD", None, None);
    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();

    run_binary_isolated(&dir, &output_dir);

    let content = fs::read_to_string(page_path(&output_dir, "sprint-alpha"))
        .expect("sprint-alpha.md must exist");
    let task_line = content
        .lines()
        .find(|l| l.trim_start().starts_with("- TODO Test task"))
        .expect("Task block line must exist in page content");
    assert!(
        !task_line.contains("[["),
        "TBD-owned task block line must contain no [[...]] pattern of any kind, got: {:?}",
        task_line
    );
}

#[test]
fn test_tbd_owner_does_not_suppress_properties_drawer_and_dates() {
    // R16 boundary + falsification: TBD owner with dates → PROPERTIES drawer, SCHEDULED,
    // and DEADLINE all present. Falsifies: implementation that strips drawer alongside owner ref.
    let dir = setup_temp_dir();
    write_schema_file(&dir, WP_SCHEMA);
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_WP, "work_package", "Sprint Alpha", None, None)]);
    seed_task_added(&dir, TASK_ID_1, ITEM_WP, "TODO", "TBD", Some("2026-06-15"), Some("2026-06-30"));
    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();

    run_binary_isolated(&dir, &output_dir);

    let content = fs::read_to_string(page_path(&output_dir, "sprint-alpha"))
        .expect("sprint-alpha.md must exist");
    assert!(content.contains("  :PROPERTIES:"), "TBD-owner task must still have :PROPERTIES: drawer");
    assert!(
        content.contains(&format!("  :task-id: {}", TASK_ID_1)),
        "TBD-owner task must still carry :task-id: in the properties drawer"
    );
    assert!(content.contains("  :END:"), "TBD-owner task must still have :END:");
    assert!(
        content.contains("  SCHEDULED: <2026-06-15 Mon>"),
        "TBD-owner task must still render SCHEDULED line, got:\n{}",
        content
    );
    assert!(
        content.contains("  DEADLINE: <2026-06-30 Tue>"),
        "TBD-owner task must still render DEADLINE line, got:\n{}",
        content
    );
}

#[test]
fn test_task_block_named_owner_with_dates_renders_wiki_link_and_dates() {
    // R16 happy path: named owner + dates → block line has [[owner-slug]], SCHEDULED, DEADLINE.
    let dir = setup_temp_dir();
    write_schema_file(&dir, WP_SCHEMA);
    seed_incorporated_items(
        &dir, SESSION_A,
        &[(ITEM_WP, "work_package", "Sprint Alpha", None, None),
          (ITEM_SH, "stakeholder", "Alice Stakeholder", None, None)],
    );
    seed_task_added(&dir, TASK_ID_1, ITEM_WP, "DOING", ITEM_SH, Some("2026-06-15"), Some("2026-06-30"));
    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();

    run_binary_isolated(&dir, &output_dir);

    let content = fs::read_to_string(page_path(&output_dir, "sprint-alpha"))
        .expect("sprint-alpha.md must exist");
    assert!(
        content.contains("- DOING Test task [[alice-stakeholder]]"),
        "Named-owner task with dates must render '- MARKER description [[owner-slug]]', got:\n{}",
        content
    );
    assert!(content.contains("  SCHEDULED: <2026-06-15 Mon>"), "SCHEDULED must be present");
    assert!(content.contains("  DEADLINE: <2026-06-30 Tue>"), "DEADLINE must be present");
}

#[test]
fn test_task_block_no_dates_omits_scheduled_and_deadline_lines() {
    let dir = setup_temp_dir();
    write_schema_file(&dir, WP_SCHEMA);
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_WP, "work_package", "Sprint Alpha", None, None)]);
    seed_task_added(&dir, TASK_ID_1, ITEM_WP, "TODO", "TBD", None, None);
    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();

    run_binary_isolated(&dir, &output_dir);

    let content = fs::read_to_string(page_path(&output_dir, "sprint-alpha"))
        .expect("sprint-alpha.md must exist");
    assert!(!content.contains("SCHEDULED:"), "No SCHEDULED line when task has no scheduled_date");
    assert!(!content.contains("DEADLINE:"),  "No DEADLINE line when task has no deadline");
}

#[test]
fn test_task_block_with_dates_renders_logseq_date_format() {
    let dir = setup_temp_dir();
    write_schema_file(&dir, WP_SCHEMA);
    seed_incorporated_items(&dir, SESSION_A, &[(ITEM_WP, "work_package", "Sprint Alpha", None, None)]);
    seed_task_added(&dir, TASK_ID_1, ITEM_WP, "DOING", "TBD", Some("2026-06-15"), Some("2026-06-30"));
    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();

    run_binary_isolated(&dir, &output_dir);

    let content = fs::read_to_string(page_path(&output_dir, "sprint-alpha"))
        .expect("sprint-alpha.md must exist");
    assert!(
        content.contains("  SCHEDULED: <2026-06-15 Mon>"),
        "SCHEDULED must use Logseq date format <YYYY-MM-DD DDD>"
    );
    assert!(
        content.contains("  DEADLINE: <2026-06-30 Tue>"),
        "DEADLINE must use Logseq date format <YYYY-MM-DD DDD>"
    );
}

#[test]
fn test_work_package_assigned_to_renders_as_page_property() {
    let dir = setup_temp_dir();
    write_schema_file(&dir, WP_SCHEMA);
    seed_incorporated_items(
        &dir, SESSION_A,
        &[(ITEM_WP, "work_package", "Sprint Alpha", None, None),
          (ITEM_SH, "stakeholder", "Alice Stakeholder", None, None)],
    );
    seed_item_linked(&dir, ITEM_WP, "assigned_to", ITEM_SH);
    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();

    run_binary_isolated(&dir, &output_dir);

    let content = fs::read_to_string(page_path(&output_dir, "sprint-alpha"))
        .expect("sprint-alpha.md must exist");
    assert!(
        content.contains("assigned-to:: [[alice-stakeholder]]"),
        "Work package must render assigned_to as 'assigned-to::' page property, got:\n{}",
        content
    );
    assert!(
        !content.contains("- Assigned To"),
        "Work package must NOT render assigned_to as a content section"
    );
}

#[test]
fn test_work_package_blocking_renders_as_page_property() {
    let dir = setup_temp_dir();
    write_schema_file(&dir, WP_SCHEMA);
    seed_incorporated_items(
        &dir, SESSION_A,
        &[(ITEM_WP, "work_package", "Sprint Alpha", None, None),
          (ITEM_WP2, "work_package", "Infrastructure Setup", None, None)],
    );
    seed_item_linked(&dir, ITEM_WP, "blocks", ITEM_WP2);
    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();

    run_binary_isolated(&dir, &output_dir);

    let content = fs::read_to_string(page_path(&output_dir, "sprint-alpha"))
        .expect("sprint-alpha.md must exist");
    assert!(
        content.contains("blocking:: [[infrastructure-setup]]"),
        "Work package must render blocks (outgoing) as 'blocking::' page property, got:\n{}",
        content
    );
    assert!(
        !content.contains("- Blocking"),
        "Work package must NOT render blocks relation as a content section"
    );
}

#[test]
fn test_work_package_multiple_blocked_by_space_separated_on_one_property_line() {
    let dir = setup_temp_dir();
    write_schema_file(&dir, WP_SCHEMA);
    const ITEM_WP3: &str = "00000001-0000-0000-0000-000000000004";
    seed_incorporated_items(
        &dir, SESSION_A,
        &[(ITEM_WP, "work_package", "Sprint Alpha", None, None),
          (ITEM_WP2, "work_package", "Infrastructure Setup", None, None),
          (ITEM_WP3, "work_package", "Mobile Backend", None, None)],
    );
    seed_item_linked(&dir, ITEM_WP2, "blocks", ITEM_WP);
    seed_item_linked(&dir, ITEM_WP3, "blocks", ITEM_WP);
    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();

    run_binary_isolated(&dir, &output_dir);

    let content = fs::read_to_string(page_path(&output_dir, "sprint-alpha"))
        .expect("sprint-alpha.md must exist");
    let blocked_line = content.lines().find(|l| l.starts_with("blocked-by::"));
    assert!(blocked_line.is_some(), "sprint-alpha page must have a blocked-by:: property line");
    let bl = blocked_line.unwrap();
    assert!(
        bl.contains("[[infrastructure-setup]]") && bl.contains("[[mobile-backend]]"),
        "Both blocked-by targets must appear on one 'blocked-by::' line, got: {}",
        bl
    );
}

#[test]
fn test_non_work_package_relations_remain_content_sections() {
    let dir = setup_temp_dir();
    write_schema_file(&dir, WP_SCHEMA);
    seed_incorporated_items(
        &dir, SESSION_A,
        &[(ITEM_WP, "work_package", "Sprint Alpha", None, None),
          (ITEM_SH, "stakeholder", "Alice Stakeholder", None, None)],
    );
    seed_item_linked(&dir, ITEM_WP, "assigned_to", ITEM_SH);
    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();

    run_binary_isolated(&dir, &output_dir);

    // Stakeholder page (non-work-package) must use content sections for its inverse relation
    let content = fs::read_to_string(page_path(&output_dir, "alice-stakeholder"))
        .expect("alice-stakeholder.md must exist");
    assert!(
        content.contains("- Owns"),
        "Non-work-package items must render inverse relations as '- Label' content sections"
    );
    assert!(
        !content.contains("owns::"),
        "Non-work-package items must NOT render relations as page properties"
    );
}

// ── R15: Dashboard.md generation ─────────────────────────────────────────────

// Schema with all five operational types present.
const DASHBOARD_SCHEMA_FULL: &str = "schemaVersion: 1
pageTypes:
  Milestone:
    aliases: [milestone]
  WorkPackage:
    aliases: [workpackage]
  Risk:
    aliases: [risk]
  Stakeholder:
    aliases: [stakeholder]
blockTypes:
  task:
    markers:
      TODO: todo
      DOING: doing
      DONE: done
";

// Schema without Milestone.
const DASHBOARD_SCHEMA_NO_MILESTONE: &str = "schemaVersion: 1
pageTypes:
  WorkPackage:
    aliases: [workpackage]
  Risk:
    aliases: [risk]
  Stakeholder:
    aliases: [stakeholder]
blockTypes:
  task:
    markers:
      TODO: todo
      DOING: doing
      DONE: done
";

// Schema with no recognized operational types.
const DASHBOARD_SCHEMA_NO_OP_TYPES: &str = "schemaVersion: 1
pageTypes:
  CustomType:
    aliases: []
";

// Schema where work-package equivalent has canonical key \"Workstream\" (alias \"workpackage\").
// Falsifies hardcoded \"work-package\" in Dashboard query.
const DASHBOARD_SCHEMA_WORKSTREAM: &str = "schemaVersion: 1
pageTypes:
  Workstream:
    aliases: [workpackage]
  Risk:
    aliases: [risk]
blockTypes:
  task:
    markers:
      TODO: todo
      DOING: doing
";

const ITEM_MS: &str = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeee10";

fn dashboard_path_for(output_dir: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(output_dir).join("pages").join("Dashboard.md")
}

#[test]
fn test_r15_fresh_export_creates_dashboard_with_type_slugs() {
    // HP: fresh export with recognized operational types → Dashboard.md created.
    // Type slugs in queries are derived from canonical schema keys via type_to_logseq_tag.
    let dir = setup_temp_dir();
    write_schema_file(&dir, DASHBOARD_SCHEMA_FULL);
    seed_incorporated_items(
        &dir, SESSION_A,
        &[(ITEM_MS, "milestone", "Q3 Release", None, None)],
    );
    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();

    run_binary_isolated(&dir, &output_dir);

    let dash = dashboard_path_for(&output_dir);
    assert!(dash.exists(), "Dashboard.md must be created on fresh export with recognized types");
    let content = fs::read_to_string(&dash).unwrap();
    assert!(
        content.contains("\"milestone\""),
        "Dashboard must contain 'milestone' type slug (from Milestone canonical key), got:\n{}",
        content
    );
    assert!(
        content.contains("\"work-package\""),
        "Dashboard must contain 'work-package' type slug (from WorkPackage canonical key), got:\n{}",
        content
    );
}

#[test]
fn test_r15_dashboard_not_overwritten_when_already_exists() {
    // HP: pre-existing Dashboard.md → not modified by export (custom content preserved).
    let dir = setup_temp_dir();
    write_schema_file(&dir, DASHBOARD_SCHEMA_FULL);
    seed_incorporated_items(
        &dir, SESSION_A,
        &[(ITEM_MS, "milestone", "Q3 Release", None, None)],
    );
    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();

    let pages_dir = std::path::PathBuf::from(&output_dir).join("pages");
    fs::create_dir_all(&pages_dir).unwrap();
    let dash = pages_dir.join("Dashboard.md");
    let custom = "custom-dashboard-content-that-must-not-be-overwritten";
    fs::write(&dash, custom).unwrap();

    run_binary_isolated(&dir, &output_dir);

    let after = fs::read_to_string(&dash).unwrap();
    assert_eq!(after, custom, "Pre-existing Dashboard.md must not be modified by export");
}

#[test]
fn test_r15_dashboard_section_omitted_for_absent_schema_type() {
    // HP: schema without Milestone → no Pending Milestones section in Dashboard.
    // Other sections for present types are still generated.
    let dir = setup_temp_dir();
    write_schema_file(&dir, DASHBOARD_SCHEMA_NO_MILESTONE);
    const ITEM_WP_DASH: &str = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeee11";
    seed_incorporated_items(
        &dir, SESSION_A,
        &[(ITEM_WP_DASH, "workpackage", "Beta Sprint", None, None)],
    );
    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();

    run_binary_isolated(&dir, &output_dir);

    let dash = dashboard_path_for(&output_dir);
    assert!(dash.exists(), "Dashboard.md must be created when at least one operational type present");
    let content = fs::read_to_string(&dash).unwrap();
    assert!(
        !content.contains("Pending Milestones"),
        "Dashboard must NOT contain Milestone section when Milestone absent from schema"
    );
    assert!(
        content.contains("\"work-package\""),
        "Dashboard must still contain WorkPackage section when WorkPackage present, got:\n{}",
        content
    );
}

#[test]
fn test_r15_no_dashboard_when_no_recognized_operational_types() {
    // Boundary: schema with no recognized operational types → no Dashboard.md written.
    let dir = setup_temp_dir();
    write_schema_file(&dir, DASHBOARD_SCHEMA_NO_OP_TYPES);
    const ITEM_CUSTOM: &str = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeee12";
    seed_incorporated_items(
        &dir, SESSION_A,
        &[(ITEM_CUSTOM, "CustomType", "Some Custom Item", None, None)],
    );
    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();

    run_binary_isolated(&dir, &output_dir);

    let dash = dashboard_path_for(&output_dir);
    assert!(
        !dash.exists(),
        "Dashboard.md must NOT be created when no recognized operational types are in the schema"
    );
}

#[test]
fn test_r15_dashboard_type_slug_derived_from_schema_not_hardcoded() {
    // Falsification: schema with "Workstream" pageType aliased as "workpackage".
    // Dashboard must use "workstream" (from canonical key), not the hardcoded "work-package".
    // Falsifies: implementation that hardcodes "work-package" in the WP query.
    let dir = setup_temp_dir();
    write_schema_file(&dir, DASHBOARD_SCHEMA_WORKSTREAM);
    const ITEM_WS: &str = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeee13";
    seed_incorporated_items(
        &dir, SESSION_A,
        &[(ITEM_WS, "workpackage", "Q3 Workstream", None, None)],
    );
    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();

    run_binary_isolated(&dir, &output_dir);

    let dash = dashboard_path_for(&output_dir);
    assert!(dash.exists(), "Dashboard.md must be created");
    let content = fs::read_to_string(&dash).unwrap();
    assert!(
        content.contains("\"workstream\""),
        "Dashboard must use 'workstream' from Workstream canonical key, got:\n{}",
        content
    );
    assert!(
        !content.contains("\"work-package\""),
        "Dashboard must NOT contain hardcoded 'work-package' when canonical key is Workstream, got:\n{}",
        content
    );
}

#[test]
fn test_r15_reexport_does_not_overwrite_dashboard() {
    // HP Idempotent re-export: Dashboard.md once created is not overwritten on re-export.
    let dir = setup_temp_dir();
    write_schema_file(&dir, DASHBOARD_SCHEMA_FULL);
    seed_incorporated_items(
        &dir, SESSION_A,
        &[(ITEM_MS, "milestone", "Q3 Release", None, None)],
    );
    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();

    run_binary_isolated(&dir, &output_dir);
    let dash = dashboard_path_for(&output_dir);
    assert!(dash.exists(), "Dashboard.md must exist after first export");

    let custom = "custom-content-written-after-first-export";
    fs::write(&dash, custom).unwrap();

    run_binary_isolated(&dir, &output_dir);
    let after = fs::read_to_string(&dash).unwrap();
    assert_eq!(after, custom, "Re-export must not overwrite Dashboard.md");
}

#[test]
fn test_r15_stale_page_deletion_preserves_dashboard() {
    // Dashboard.md is exempt from stale page deletion (it is never an item page).
    let dir = setup_temp_dir();
    write_schema_file(&dir, DASHBOARD_SCHEMA_FULL);
    seed_incorporated_items(
        &dir, SESSION_A,
        &[(ITEM_MS, "milestone", "Q3 Release", None, None)],
    );
    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();

    run_binary_isolated(&dir, &output_dir);
    let dash = dashboard_path_for(&output_dir);
    assert!(dash.exists(), "Dashboard.md must exist after first export");

    run_binary_isolated(&dir, &output_dir);
    assert!(
        dash.exists(),
        "Dashboard.md must NOT be deleted by stale page cleanup on re-export"
    );
}

// ── F16: Extraction-sourced WP task attribution ───────────────────────────────

const SESSION_F16: &str = "f1600000-0000-0000-0000-000000000001";
const TASK_EXTRACTED: &str = "20000001-0000-0000-0000-000000000001";

/// Seed ItemsExtracted + ExtractionConfirmed + ItemsIncorporated for items that
/// include per-item F16 fields (parent_item_id, initial_marker).
/// Tuple: (id, item_type, description, proposed_status, proposed_priority, parent_item_id, initial_marker)
fn seed_extracted_items_f16(
    dir: &TempDir,
    session_id: &str,
    items: &[(&str, &str, &str, Option<&str>, Option<&str>, Option<&str>, Option<&str>)],
) {
    let items_json: Vec<Value> = items
        .iter()
        .map(|(id, typ, desc, ps, pp, pid, im)| {
            json!({
                "item_id": id,
                "item_type": typ,
                "description": desc,
                "uncertain": false,
                "uncertainty_reason": null,
                "proposed_status": ps,
                "proposed_priority": pp,
                "parent_item_id": pid,
                "initial_marker": im,
            })
        })
        .collect();

    let accepted_ids: Vec<&str> = items.iter().map(|(id, ..)| *id).collect();
    let n = items.len();

    let path = dir.path().join("events/runtime_events.jsonl");
    let mut file = fs::OpenOptions::new().create(true).append(true).open(&path).unwrap();
    writeln!(file, "{}", json!({
        "event_id":       format!("f16-ext-{}", &session_id[..8]),
        "event_type":     "ItemsExtracted",
        "timestamp":      1748200001000u64,
        "correlation_id": session_id,
        "source_module":  "pm_structuring",
        "payload": { "items": items_json, "item_count": n, "uncertain_count": 0u64 }
    })).unwrap();
    writeln!(file, "{}", json!({
        "event_id":       format!("f16-conf-{}", &session_id[..8]),
        "event_type":     "ExtractionConfirmed",
        "timestamp":      1748200002000u64,
        "correlation_id": session_id,
        "source_module":  "pm_structuring",
        "payload": { "accepted_item_ids": accepted_ids, "accepted_count": n }
    })).unwrap();
    writeln!(file, "{}", json!({
        "event_id":       format!("f16-inc-{}", &session_id[..8]),
        "event_type":     "ItemsIncorporated",
        "timestamp":      1748200003000u64,
        "correlation_id": "00000000-0000-0000-0000-00000000f160",
        "source_module":  "project_state",
        "payload": {
            "session_id": session_id,
            "incorporated_count": n,
            "total_record_size": n,
        }
    })).unwrap();
}

#[test]
fn test_f16_extraction_task_with_parent_id_renders_as_nested_block() {
    // HP5: extraction-sourced task with parent_item_id is rendered as a nested
    // task block under the WP page, not as a separate page.
    // This directly falsifies the Stage 0 gap where ..Default::default() lost parent_item_id.
    let dir = setup_temp_dir();
    write_schema_file(&dir, WP_SCHEMA);
    seed_incorporated_items(
        &dir, SESSION_A,
        &[(ITEM_WP, "work_package", "Platform Integration", None, None)],
    );
    seed_extracted_items_f16(
        &dir, SESSION_F16,
        &[(TASK_EXTRACTED, "task_block", "Set up CI pipeline",
           None, None, Some(ITEM_WP), Some("TODO"))],
    );
    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();

    run_binary_isolated(&dir, &output_dir);

    let wp_content = fs::read_to_string(page_path(&output_dir, "platform-integration"))
        .expect("WP page must exist");
    assert!(
        wp_content.contains("TODO") && wp_content.contains("Set up CI pipeline"),
        "WP page must contain the extraction-attributed task block, got:\n{}",
        wp_content
    );
    assert!(
        wp_content.contains(":task-id:"),
        "Nested task block must have a :PROPERTIES: drawer with :task-id:, got:\n{}",
        wp_content
    );

    let task_slug = description_to_slug("Set up CI pipeline");
    assert!(
        !page_path(&output_dir, &task_slug).exists(),
        "Extraction task with parent_item_id must NOT be written as a separate page"
    );
}

#[test]
fn test_f16_extraction_task_uses_initial_marker_in_block_line() {
    // HP6: initial_marker from the ItemsExtracted payload is used in the task block line.
    // Confirms that initial_marker read from find_confirmed_items is forwarded to rendering.
    let dir = setup_temp_dir();
    write_schema_file(&dir, WP_SCHEMA);
    seed_incorporated_items(
        &dir, SESSION_A,
        &[(ITEM_WP, "work_package", "Sprint Beta", None, None)],
    );
    seed_extracted_items_f16(
        &dir, SESSION_F16,
        &[(TASK_EXTRACTED, "task_block", "Write API documentation",
           None, None, Some(ITEM_WP), Some("DOING"))],
    );
    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();

    run_binary_isolated(&dir, &output_dir);

    let wp_content = fs::read_to_string(page_path(&output_dir, "sprint-beta"))
        .expect("WP page must exist");
    assert!(
        wp_content.contains("DOING Write API documentation"),
        "Task block line must use the initial_marker 'DOING' from ItemsExtracted payload, got:\n{}",
        wp_content
    );
}

#[test]
fn test_f16_extraction_task_without_parent_id_not_rendered_as_page() {
    // HP3: extraction task with no parent_item_id is a blockType item with no parent →
    // not rendered as a Logseq page (it stays in the project record as an orphan task).
    let dir = setup_temp_dir();
    write_schema_file(&dir, WP_SCHEMA);
    seed_extracted_items_f16(
        &dir, SESSION_F16,
        &[(TASK_EXTRACTED, "task_block", "Orphan unassigned task",
           None, None, None, Some("TODO"))],
    );
    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();

    run_binary_isolated(&dir, &output_dir);

    let task_slug = description_to_slug("Orphan unassigned task");
    assert!(
        !page_path(&output_dir, &task_slug).exists(),
        "Orphan extraction task (blockType, no parent_item_id) must NOT be exported as a page"
    );
}

#[test]
fn test_f16_parent_item_id_survives_extraction_confirmation_flow() {
    // Boundary: parent_item_id is propagated through the full extraction flow
    // (ItemsExtracted → ExtractionConfirmed → find_confirmed_items → export routing).
    // If lost at find_confirmed_items, the task block would not appear nested.
    let dir = setup_temp_dir();
    write_schema_file(&dir, WP_SCHEMA);
    seed_incorporated_items(
        &dir, SESSION_A,
        &[(ITEM_WP, "work_package", "Backend Platform", None, None)],
    );
    seed_extracted_items_f16(
        &dir, SESSION_F16,
        &[(TASK_EXTRACTED, "task_block", "Implement auth service",
           None, None, Some(ITEM_WP), Some("TODO"))],
    );
    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();

    run_binary_isolated(&dir, &output_dir);

    let wp_content = fs::read_to_string(page_path(&output_dir, "backend-platform"))
        .expect("WP page must exist");
    assert!(
        wp_content.contains("Implement auth service"),
        "parent_item_id must survive the extraction confirmation flow and produce a nested block, got:\n{}",
        wp_content
    );
}

#[test]
fn test_f16_no_wp_page_created_for_unresolvable_reference() {
    // Falsification: no WP page is auto-created when the WP is not in the project record.
    // A task with no parent_item_id (unresolvable WP) must not cause a WP page to appear.
    let dir = setup_temp_dir();
    write_schema_file(&dir, WP_SCHEMA);
    seed_extracted_items_f16(
        &dir, SESSION_F16,
        &[(TASK_EXTRACTED, "task_block", "Implement notifications",
           None, None, None, Some("TODO"))],
    );
    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();

    run_binary_isolated(&dir, &output_dir);

    let pages_dir = std::path::PathBuf::from(&output_dir).join("pages");
    if pages_dir.exists() {
        // Dashboard.md is legitimately generated by R15 — exclude it from the check.
        // The invariant is that no WP *item* page is auto-created.
        let item_pages: Vec<_> = fs::read_dir(&pages_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path().extension().and_then(|s| s.to_str()) == Some("md")
                    && e.file_name().to_string_lossy() != "Dashboard.md"
            })
            .collect();
        assert_eq!(
            item_pages.len(), 0,
            "No WP item page must be auto-created; found {} unexpected page(s)",
            item_pages.len()
        );
    }
}

#[test]
fn test_f16_task_and_wp_types_derived_from_schema_not_hardcoded() {
    // HP7 falsification: schema with non-standard type names.
    // "Workstream" pageType (alias "workpackage") + "action_item" blockType.
    // Extraction task with parent_item_id must render nested under the Workstream page.
    // Falsifies: hardcoded "task" or "work_package" type names in attribution logic.
    const F16_HP7_SCHEMA: &str = "schemaVersion: 1
pageTypes:
  Workstream:
    aliases: [workpackage]
blockTypes:
  action_item:
    markers:
      TODO: todo
      DONE: done
";
    const WS_ID:      &str = "30000001-0000-0000-0000-000000000001";
    const AI_ID:      &str = "30000001-0000-0000-0000-000000000002";
    const SESSION_WS: &str = "f1600000-0000-0000-0000-000000000006";
    const SESSION_AI: &str = "f1600000-0000-0000-0000-000000000007";

    let dir = setup_temp_dir();
    write_schema_file(&dir, F16_HP7_SCHEMA);

    seed_extracted_items_f16(
        &dir, SESSION_WS,
        &[(WS_ID, "workpackage", "Core Infrastructure", None, None, None, None)],
    );
    seed_extracted_items_f16(
        &dir, SESSION_AI,
        &[(AI_ID, "action_item", "Deploy monitoring stack",
           None, None, Some(WS_ID), Some("TODO"))],
    );

    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();
    run_binary_isolated(&dir, &output_dir);

    let ws_content = fs::read_to_string(page_path(&output_dir, "core-infrastructure"))
        .expect("Workstream page 'core-infrastructure' must exist");
    assert!(
        ws_content.contains("Deploy monitoring stack"),
        "action_item must render as nested block under Workstream page (no hardcoded type names), got:\n{}",
        ws_content
    );
}

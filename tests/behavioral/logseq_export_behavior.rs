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

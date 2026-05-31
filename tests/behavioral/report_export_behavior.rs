//! Behavioral tests for report_export.
//!
//! Tests verify observable outcomes: events emitted, payload shapes, file output,
//! and failure modes. No internal logic is tested.
//! All assertions reference event names from events/report_export_schema.md exactly.

use serde_json::{json, Value};
use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};
use tempfile::TempDir;

fn binary_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_report_export"))
}

fn setup_temp_dir() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("events")).unwrap();
    dir
}

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
        "payload": { "items": items_json, "item_count": items.len(), "uncertain_count": 0 }
    })).unwrap();
    writeln!(file, "{}", json!({
        "event_id": format!("seed-conf-{}", &session_id[..8]),
        "event_type": "ExtractionConfirmed",
        "timestamp": 1748000002000u64,
        "correlation_id": session_id,
        "source_module": "pm_structuring",
        "payload": { "accepted_item_ids": accepted_ids, "accepted_count": items.len() }
    })).unwrap();
    writeln!(file, "{}", json!({
        "event_id": format!("seed-inc-{}", &session_id[..8]),
        "event_type": "ItemsIncorporated",
        "timestamp": 1748000003000u64,
        "correlation_id": "00000000-0000-0000-0000-000000000001",
        "source_module": "project_state",
        "payload": { "session_id": session_id, "incorporated_count": items.len(), "total_record_size": items.len() }
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

fn read_re_events(dir: &TempDir) -> Vec<Value> {
    let path = dir.path().join("events/runtime_events.jsonl");
    if !path.exists() { return vec![]; }
    fs::read_to_string(path).unwrap()
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .filter(|e| e["source_module"].as_str() == Some("report_export"))
        .collect()
}

const SESSION_A: &str = "a4ca3a7e-61eb-4f36-b59e-f3abd166e351";
const ITEM_TASK:        &str = "c1000000-0000-0000-0000-000000000001";
const ITEM_RISK:        &str = "c1000000-0000-0000-0000-000000000002";
const ITEM_MILESTONE:   &str = "c1000000-0000-0000-0000-000000000003";
const ITEM_STAKEHOLDER: &str = "c1000000-0000-0000-0000-000000000004";

fn seed_full_record(dir: &TempDir) {
    seed_incorporated_items(dir, SESSION_A, &[
        (ITEM_TASK,        "task",        "Fix critical data loss bug"),
        (ITEM_RISK,        "risk",        "Vendor lock-in risk"),
        (ITEM_MILESTONE,   "milestone",   "Q3 release"),
        (ITEM_STAKEHOLDER, "stakeholder", "Engineering lead"),
    ]);
}

// ── Happy Path 1: stdout output ───────────────────────────────────────────────

#[test]
fn test_stdout_report_emits_requested_then_generated() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);

    run_binary(&dir, &["--type", "full"]);

    let events = read_re_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"ReportRequested"), "ReportRequested must be emitted");
    assert!(types.contains(&"ReportGenerated"), "ReportGenerated must be emitted");
    assert!(!types.contains(&"ReportFailedEmptyRecord"),   "must NOT emit EmptyRecord failure");
    assert!(!types.contains(&"ReportFailedInvalidType"),   "must NOT emit InvalidType failure");
    assert!(!types.contains(&"ReportFailedOutputNotFound"),"must NOT emit OutputNotFound failure");

    let req_pos = types.iter().position(|&t| t == "ReportRequested").unwrap();
    let gen_pos = types.iter().position(|&t| t == "ReportGenerated").unwrap();
    assert!(req_pos < gen_pos, "ReportRequested must precede ReportGenerated");
}

#[test]
fn test_stdout_report_generated_payload_shape() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);

    run_binary(&dir, &["--type", "full"]);

    let events = read_re_events(&dir);
    let generated = events.iter()
        .find(|e| e["event_type"] == "ReportGenerated")
        .expect("ReportGenerated not found");

    let p = &generated["payload"];
    assert_eq!(p["report_type"].as_str().unwrap(), "full");
    assert_eq!(p["output_destination"].as_str().unwrap(), "stdout");
    assert!(p["report_file"].is_null(), "report_file must be null when output is stdout");
    assert!(p["item_count"].as_u64().is_some(), "item_count must be an integer");
    assert!(p["item_count"].as_u64().unwrap() > 0, "item_count must be > 0");
    assert!(p["generated_at"].as_u64().is_some(), "generated_at must be a timestamp");
}

#[test]
fn test_stdout_report_writes_content_to_stdout() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);

    let output = run_binary(&dir, &["--type", "full"]);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.is_empty(), "stdout must contain report content");
    assert!(stdout.contains("# Full Project Report"), "full report must contain its heading");
}

#[test]
fn test_stdout_report_creates_no_files_in_working_directory() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);

    run_binary(&dir, &["--type", "full"]);

    let entries: Vec<_> = fs::read_dir(dir.path()).unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .collect();
    assert!(entries.is_empty(), "no files must be created in the working directory without --graph");
}

#[test]
fn test_requested_payload_contains_report_type_and_graph_path() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);

    run_binary(&dir, &["--type", "full"]);

    let events = read_re_events(&dir);
    let requested = events.iter()
        .find(|e| e["event_type"] == "ReportRequested")
        .expect("ReportRequested not found");

    let p = &requested["payload"];
    assert_eq!(p["report_type"].as_str().unwrap(), "full");
    assert!(p.get("graph_path").is_some(), "graph_path field must be present");
    assert!(p["graph_path"].is_null(), "graph_path must be null when --graph is not supplied");
}

// ── Happy Path 2: graph output ────────────────────────────────────────────────

#[test]
fn test_graph_report_writes_file_to_graph_directory() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);
    let graph_dir = tempfile::tempdir().unwrap();
    let graph_path = graph_dir.path().to_str().unwrap();

    run_binary(&dir, &["--type", "full", "--graph", graph_path]);

    let entries: Vec<_> = fs::read_dir(graph_dir.path()).unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert_eq!(entries.len(), 1, "exactly one file must be written to the graph directory");
    assert!(entries[0].file_name().to_str().unwrap().ends_with(".md"),
        "written file must be a markdown file");
}

#[test]
fn test_graph_report_generated_payload_has_file_path() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);
    let graph_dir = tempfile::tempdir().unwrap();
    let graph_path = graph_dir.path().to_str().unwrap();

    run_binary(&dir, &["--type", "full", "--graph", graph_path]);

    let events = read_re_events(&dir);
    let generated = events.iter()
        .find(|e| e["event_type"] == "ReportGenerated")
        .expect("ReportGenerated not found");

    let p = &generated["payload"];
    assert_eq!(p["output_destination"].as_str().unwrap(), graph_path);
    assert!(p["report_file"].as_str().is_some(), "report_file must be a string when output is graph");
    assert!(p["report_file"].as_str().unwrap().contains("Full Project Report"),
        "report_file path must contain the report filename");

    let file_path = p["report_file"].as_str().unwrap();
    assert!(std::path::Path::new(file_path).exists(),
        "reported report_file path must exist on disk");
}

#[test]
fn test_graph_report_is_idempotent() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);
    let graph_dir = tempfile::tempdir().unwrap();
    let graph_path = graph_dir.path().to_str().unwrap();

    run_binary(&dir, &["--type", "full", "--graph", graph_path]);
    run_binary(&dir, &["--type", "full", "--graph", graph_path]);

    let entries: Vec<_> = fs::read_dir(graph_dir.path()).unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert_eq!(entries.len(), 1, "running twice must not create a second file");
}

#[test]
fn test_graph_report_requested_payload_records_graph_path() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);
    let graph_dir = tempfile::tempdir().unwrap();
    let graph_path = graph_dir.path().to_str().unwrap();

    run_binary(&dir, &["--type", "full", "--graph", graph_path]);

    let events = read_re_events(&dir);
    let requested = events.iter()
        .find(|e| e["event_type"] == "ReportRequested")
        .expect("ReportRequested not found");

    assert_eq!(requested["payload"]["graph_path"].as_str().unwrap(), graph_path);
}

// ── Happy Path 3: weekly with no recent sessions ──────────────────────────────

#[test]
fn test_weekly_report_no_recent_sessions_emits_generated_not_failure() {
    let dir = setup_temp_dir();
    // Seed uses old timestamps (2025), far older than 7 days from test runtime
    seed_full_record(&dir);

    run_binary(&dir, &["--type", "weekly"]);

    let events = read_re_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"ReportGenerated"),
        "ReportGenerated must be emitted even when no sessions in the last 7 days");
    assert!(!types.iter().any(|t| t.starts_with("ReportFailed")),
        "no failure event must be emitted when the record has items");
}

#[test]
fn test_weekly_report_output_notes_no_recent_sessions() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);

    let output = run_binary(&dir, &["--type", "weekly"]);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("No sessions incorporated in the last 7 days"),
        "weekly report must note when no recent sessions exist");
}

#[test]
fn test_all_report_types_produce_generated_event() {
    for report_type in &["full", "risk-register", "stakeholders", "weekly"] {
        let dir = setup_temp_dir();
        seed_full_record(&dir);

        run_binary(&dir, &["--type", report_type]);

        let events = read_re_events(&dir);
        assert!(
            events.iter().any(|e| e["event_type"] == "ReportGenerated"),
            "ReportGenerated must be emitted for report type '{}'", report_type
        );
    }
}

// ── Failure Path 1: EmptyRecord ───────────────────────────────────────────────

#[test]
fn test_empty_record_emits_failure_event() {
    let dir = setup_temp_dir();
    // No items seeded

    run_binary(&dir, &["--type", "full"]);

    let events = read_re_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"ReportRequested"),         "ReportRequested must be emitted");
    assert!(types.contains(&"ReportFailedEmptyRecord"), "ReportFailedEmptyRecord must be emitted");
    assert!(!types.contains(&"ReportGenerated"),        "ReportGenerated must NOT be emitted");
}

#[test]
fn test_empty_record_failure_reason() {
    let dir = setup_temp_dir();

    run_binary(&dir, &["--type", "full"]);

    let events = read_re_events(&dir);
    let failure = events.iter()
        .find(|e| e["event_type"] == "ReportFailedEmptyRecord")
        .expect("ReportFailedEmptyRecord not found");

    assert_eq!(failure["payload"]["failure_reason"].as_str().unwrap(), "empty_record");
}

#[test]
fn test_empty_record_no_files_created() {
    let dir = setup_temp_dir();
    let graph_dir = tempfile::tempdir().unwrap();
    let graph_path = graph_dir.path().to_str().unwrap();

    run_binary(&dir, &["--type", "full", "--graph", graph_path]);

    let entries: Vec<_> = fs::read_dir(graph_dir.path()).unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert!(entries.is_empty(), "no files must be created on EmptyRecord failure");
}

// ── Failure Path 2: InvalidReportType ────────────────────────────────────────

#[test]
fn test_invalid_type_emits_failure_event() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);

    run_binary(&dir, &["--type", "summary"]);

    let events = read_re_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"ReportRequested"),          "ReportRequested must be emitted");
    assert!(types.contains(&"ReportFailedInvalidType"),  "ReportFailedInvalidType must be emitted");
    assert!(!types.contains(&"ReportGenerated"),         "ReportGenerated must NOT be emitted");
}

#[test]
fn test_invalid_type_failure_payload() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);

    run_binary(&dir, &["--type", "summary"]);

    let events = read_re_events(&dir);
    let failure = events.iter()
        .find(|e| e["event_type"] == "ReportFailedInvalidType")
        .expect("ReportFailedInvalidType not found");

    let p = &failure["payload"];
    assert_eq!(p["failure_reason"].as_str().unwrap(), "invalid_report_type");
    assert_eq!(p["report_type"].as_str().unwrap(), "summary",
        "invalid report_type value must be echoed in the failure payload");
}

#[test]
fn test_invalid_type_checked_before_empty_record() {
    let dir = setup_temp_dir();
    // No items AND invalid type → InvalidType must win

    run_binary(&dir, &["--type", "bogus"]);

    let events = read_re_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"ReportFailedInvalidType"),
        "ReportFailedInvalidType must be emitted even when record is also empty");
    assert!(!types.contains(&"ReportFailedEmptyRecord"),
        "ReportFailedEmptyRecord must NOT be emitted when type is already invalid");
}

// ── Failure Path 3: OutputNotFound ────────────────────────────────────────────

#[test]
fn test_output_not_found_emits_failure_event() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);

    run_binary(&dir, &["--type", "full", "--graph", "/tmp/does_not_exist_lucidpm_test"]);

    let events = read_re_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"ReportRequested"),             "ReportRequested must be emitted");
    assert!(types.contains(&"ReportFailedOutputNotFound"),  "ReportFailedOutputNotFound must be emitted");
    assert!(!types.contains(&"ReportGenerated"),            "ReportGenerated must NOT be emitted");
}

#[test]
fn test_output_not_found_failure_payload() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);

    let missing_path = "/tmp/does_not_exist_lucidpm_test";
    run_binary(&dir, &["--type", "full", "--graph", missing_path]);

    let events = read_re_events(&dir);
    let failure = events.iter()
        .find(|e| e["event_type"] == "ReportFailedOutputNotFound")
        .expect("ReportFailedOutputNotFound not found");

    let p = &failure["payload"];
    assert_eq!(p["failure_reason"].as_str().unwrap(), "output_not_found");
    assert_eq!(p["graph_path"].as_str().unwrap(), missing_path,
        "the missing graph_path must be echoed in the failure payload");
}

// ── Invariants ────────────────────────────────────────────────────────────────

#[test]
fn test_report_generation_does_not_modify_incorporated_item_count() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);

    // Count non-report_export events before and after
    let count_before = {
        let path = dir.path().join("events/runtime_events.jsonl");
        fs::read_to_string(&path).unwrap().lines().filter(|l| !l.is_empty()).count()
    };

    run_binary(&dir, &["--type", "full"]);

    let path = dir.path().join("events/runtime_events.jsonl");
    let all_lines: Vec<_> = fs::read_to_string(&path).unwrap()
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .collect();

    let non_report_count = all_lines.iter()
        .filter(|e| e["source_module"].as_str() != Some("report_export"))
        .count();

    assert_eq!(non_report_count, count_before,
        "report generation must not modify or add any non-report_export events");
}

#[test]
fn test_running_report_twice_emits_same_item_count() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);

    run_binary(&dir, &["--type", "full"]);
    run_binary(&dir, &["--type", "full"]);

    let events = read_re_events(&dir);
    let counts: Vec<u64> = events.iter()
        .filter(|e| e["event_type"] == "ReportGenerated")
        .map(|e| e["payload"]["item_count"].as_u64().unwrap())
        .collect();

    assert_eq!(counts.len(), 2, "must have two ReportGenerated events");
    assert_eq!(counts[0], counts[1],
        "item_count must be identical across invocations (report is read-only)");
}

// ── Telemetry ─────────────────────────────────────────────────────────────────

#[test]
fn test_all_events_have_required_base_fields() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);
    run_binary(&dir, &["--type", "full"]);

    let events = read_re_events(&dir);
    assert!(!events.is_empty());

    for event in &events {
        let t = event["event_type"].as_str().unwrap_or("unknown");
        assert!(event["event_id"].as_str().is_some(),       "{}: event_id must be a string", t);
        assert!(event["event_type"].as_str().is_some(),     "{}: event_type must be a string", t);
        assert!(event["timestamp"].as_u64().is_some(),      "{}: timestamp must be a u64", t);
        assert!(event["correlation_id"].as_str().is_some(), "{}: correlation_id must be a string", t);
        assert!(event["source_module"].as_str().is_some(),  "{}: source_module must be a string", t);
        assert!(event["payload"].is_object(),               "{}: payload must be an object", t);
        assert_eq!(event["source_module"].as_str().unwrap(), "report_export",
            "{}: source_module must be 'report_export'", t);
        assert!(event["timestamp"].as_u64().unwrap() > 0,
            "{}: timestamp must be positive", t);
    }
}

#[test]
fn test_correlation_id_consistent_within_one_invocation() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);
    run_binary(&dir, &["--type", "full"]);

    let events = read_re_events(&dir);
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
    seed_full_record(&dir);

    run_binary(&dir, &["--type", "full"]);
    run_binary(&dir, &["--type", "full"]);

    let events = read_re_events(&dir);
    let cids: Vec<&str> = events.iter()
        .filter(|e| e["event_type"] == "ReportRequested")
        .map(|e| e["correlation_id"].as_str().unwrap())
        .collect();

    assert_eq!(cids.len(), 2, "must have two ReportRequested events");
    assert_ne!(cids[0], cids[1],
        "different invocations must produce different correlation_ids");
}

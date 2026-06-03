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

// ─────────────────────────────────────────────────────────────────────────────
// R8: schema-driven section grouping, canonical labels, and unrecognized item
// exclusion — helpers and tests
// ─────────────────────────────────────────────────────────────────────────────

fn write_project_schema(dir: &TempDir, yaml: &str) {
    fs::write(dir.path().join("project-schema.yaml"), yaml).unwrap();
}

fn read_all_events(dir: &TempDir) -> Vec<Value> {
    let path = dir.path().join("events/runtime_events.jsonl");
    if !path.exists() { return vec![]; }
    fs::read_to_string(path).unwrap()
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .collect()
}

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

const SESSION_B:     &str = "b5db4b8f-72fc-5047-c60f-a4bce277f462";
const ITEM_SPRINT:   &str = "c2000000-0000-0000-0000-000000000001";
const ITEM_HAZARD:   &str = "c2000000-0000-0000-0000-000000000002";
const ITEM_PARTNER:  &str = "c2000000-0000-0000-0000-000000000003";
const ITEM_WIDGET:   &str = "c2000000-0000-0000-0000-000000000004";
const ITEM_WIDGET2:  &str = "c2000000-0000-0000-0000-000000000005";

// Standard vocab for isolated tests — mirrors the default schema types.
const BASIC_VOCAB: &str = r#"schemaVersion: 1
pageTypes:
  task:
    allowedStatuses: [todo, doing, done, cancelled]
  risk:
    allowedStatuses: [open, mitigated, accepted, closed]
  milestone:
    allowedStatuses: [pending, achieved, missed]
  stakeholder:
    allowedStatuses: [active, inactive]
  issue:
    allowedStatuses: [open, resolved, closed]
statuses:
  todo: null
  doing: null
  done: null
  cancelled: null
  open: null
  mitigated: null
  accepted: null
  closed: null
  pending: null
  achieved: null
  missed: null
  active: null
  inactive: null
  resolved: null
"#;

// Vocab with aliases for HP1/HP2 alias-resolution tests.
const ALIAS_VOCAB: &str = r#"schemaVersion: 1
pageTypes:
  task:
    aliases: [sprint]
    allowedStatuses: [todo, doing, done, cancelled]
  risk:
    aliases: [hazard]
    allowedStatuses: [open, mitigated, accepted, closed]
  stakeholder:
    aliases: [partner]
    allowedStatuses: [active, inactive]
  milestone:
    allowedStatuses: [pending, achieved, missed]
statuses:
  todo: null
  doing: null
  done: null
  cancelled: null
  open: null
  mitigated: null
  accepted: null
  closed: null
  pending: null
  achieved: null
  missed: null
  active: null
  inactive: null
"#;

// Vocab with capitalized canonical names and lowercase aliases — mirrors real
// user schemas like ~/.lucidpm/default-schema.yaml (Risk, Task, Stakeholder …).
// Required to catch any future regression where hardcoded lowercase comparisons
// would silently break fixed-scope reports without failing any existing test.
const CAPITALIZED_VOCAB: &str = r#"schemaVersion: 1
pageTypes:
  Risk:
    aliases: [risk]
    allowedStatuses: [open, mitigated, accepted, closed]
  Task:
    aliases: [task]
    allowedStatuses: [todo, doing, done, cancelled]
  Stakeholder:
    aliases: [stakeholder]
    allowedStatuses: [active, inactive]
  Milestone:
    aliases: [milestone]
    allowedStatuses: [pending, achieved, missed]
statuses:
  open: null
  mitigated: null
  accepted: null
  closed: null
  todo: null
  doing: null
  done: null
  cancelled: null
  active: null
  inactive: null
  pending: null
  achieved: null
  missed: null
"#;

// ── HP1: Full report groups items by canonical vocabulary type ────────────────

#[test]
fn test_r8_full_report_section_header_uses_canonical_type_not_alias() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, ALIAS_VOCAB);
    seed_incorporated_items(&dir, SESSION_B, &[
        (ITEM_SPRINT, "sprint", "Sprint item stored under alias"),
    ]);

    let output = run_binary_isolated(&dir, &["--type", "full"]);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("## Tasks"),
        "section header must use canonical type name 'task' (capitalized, plural)");
    assert!(!stdout.contains("## Sprints"),
        "section header must NOT use the alias 'sprint'");
}

#[test]
fn test_r8_full_report_alias_item_grouped_in_canonical_section() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, ALIAS_VOCAB);
    seed_incorporated_items(&dir, SESSION_B, &[
        (ITEM_TASK,   "task",   "Canonical task item"),
        (ITEM_SPRINT, "sprint", "Alias task item"),
    ]);

    let output = run_binary_isolated(&dir, &["--type", "full"]);

    let stdout = String::from_utf8_lossy(&output.stdout);
    let task_pos = stdout.find("## Tasks").expect("Tasks section must exist");
    assert!(stdout.find("## Sprints").is_none(), "no separate Sprints section");

    let after_tasks = &stdout[task_pos..];
    assert!(after_tasks.contains("Canonical task item"), "canonical item in Tasks section");
    assert!(after_tasks.contains("Alias task item"),     "alias item in Tasks section");
}

#[test]
fn test_r8_full_report_alias_item_counted_in_report_generated() {
    // Seed exactly one canonical item and one alias item so that item_count == 2
    // directly proves the alias item was counted, not merely that something was.
    let dir = setup_temp_dir();
    write_project_schema(&dir, ALIAS_VOCAB);
    seed_incorporated_items(&dir, SESSION_B, &[
        (ITEM_TASK,   "task",   "Canonical task item"),
        (ITEM_SPRINT, "sprint", "Alias task item"),
    ]);

    run_binary_isolated(&dir, &["--type", "full"]);

    let events = read_re_events(&dir);
    let generated = events.iter().find(|e| e["event_type"] == "ReportGenerated")
        .expect("ReportGenerated must be emitted");
    assert_eq!(generated["payload"]["item_count"].as_u64().unwrap(), 2,
        "item_count must equal canonical_count + alias_count — alias item must be counted");
}

// ── HP2: Fixed report scopes include alias-stored items ───────────────────────

#[test]
fn test_r8_risk_register_includes_alias_stored_item() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, ALIAS_VOCAB);
    seed_incorporated_items(&dir, SESSION_B, &[
        (ITEM_HAZARD, "hazard", "Hazard stored under alias"),
    ]);

    run_binary_isolated(&dir, &["--type", "risk-register"]);

    let events = read_re_events(&dir);
    let generated = events.iter().find(|e| e["event_type"] == "ReportGenerated")
        .expect("ReportGenerated must be emitted");
    assert_eq!(generated["payload"]["item_count"].as_u64().unwrap(), 1,
        "alias-stored risk item must appear in risk-register report");
}

#[test]
fn test_r8_risk_register_canonical_and_alias_items_both_included() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, ALIAS_VOCAB);
    seed_incorporated_items(&dir, SESSION_B, &[
        (ITEM_RISK,   "risk",   "Canonical risk"),
        (ITEM_HAZARD, "hazard", "Alias risk"),
    ]);

    run_binary_isolated(&dir, &["--type", "risk-register"]);

    let events = read_re_events(&dir);
    let generated = events.iter().find(|e| e["event_type"] == "ReportGenerated")
        .expect("ReportGenerated must be emitted");
    assert_eq!(generated["payload"]["item_count"].as_u64().unwrap(), 2,
        "both canonical and alias risk items must be included in risk-register");
}

#[test]
fn test_r8_stakeholders_includes_alias_stored_item() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, ALIAS_VOCAB);
    seed_incorporated_items(&dir, SESSION_B, &[
        (ITEM_PARTNER, "partner", "External partner"),
    ]);

    run_binary_isolated(&dir, &["--type", "stakeholders"]);

    let events = read_re_events(&dir);
    let generated = events.iter().find(|e| e["event_type"] == "ReportGenerated")
        .expect("ReportGenerated must be emitted");
    assert_eq!(generated["payload"]["item_count"].as_u64().unwrap(), 1,
        "alias-stored stakeholder item must appear in stakeholders report");
}

// ── HP3: Unrecognized items excluded; SchemaTypeUnknown emitted ───────────────

#[test]
fn test_r8_unrecognized_item_excluded_from_report_content() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, BASIC_VOCAB);
    seed_incorporated_items(&dir, SESSION_B, &[
        (ITEM_TASK,   "task",   "Recognized task"),
        (ITEM_WIDGET, "widget", "Unrecognized widget"),
    ]);

    run_binary_isolated(&dir, &["--type", "full"]);

    let events = read_re_events(&dir);
    let generated = events.iter().find(|e| e["event_type"] == "ReportGenerated")
        .expect("ReportGenerated must be emitted — exclusion is not a failure");
    assert_eq!(generated["payload"]["item_count"].as_u64().unwrap(), 1,
        "only the recognized item is counted");
}

#[test]
fn test_r8_schema_type_unknown_emitted_per_excluded_item() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, BASIC_VOCAB);
    seed_incorporated_items(&dir, SESSION_B, &[
        (ITEM_WIDGET,  "widget", "Unrecognized item one"),
        (ITEM_WIDGET2, "gadget", "Unrecognized item two"),
    ]);

    run_binary_isolated(&dir, &["--type", "full"]);

    let all = read_all_events(&dir);
    let unknown_count = all.iter()
        .filter(|e| e["source_module"].as_str() == Some("project_schema")
            && e["event_type"].as_str() == Some("SchemaTypeUnknown"))
        .count();
    assert_eq!(unknown_count, 2,
        "exactly one SchemaTypeUnknown per excluded item");
}

#[test]
fn test_r8_schema_type_unknown_carries_item_id_and_unknown_type() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, BASIC_VOCAB);
    seed_incorporated_items(&dir, SESSION_B, &[
        (ITEM_WIDGET, "widget", "Unrecognized item"),
    ]);

    run_binary_isolated(&dir, &["--type", "full"]);

    let all = read_all_events(&dir);
    let unknown = all.iter()
        .find(|e| e["source_module"].as_str() == Some("project_schema")
            && e["event_type"].as_str() == Some("SchemaTypeUnknown"))
        .expect("SchemaTypeUnknown must be emitted");

    assert_eq!(unknown["payload"]["item_id"].as_str().unwrap(), ITEM_WIDGET,
        "SchemaTypeUnknown must carry the excluded item_id");
    assert_eq!(unknown["payload"]["unknown_type"].as_str().unwrap(), "widget",
        "SchemaTypeUnknown must carry the unrecognized type string");
}

#[test]
fn test_r8_schema_type_unknown_shares_correlation_id_with_report_events() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, BASIC_VOCAB);
    seed_incorporated_items(&dir, SESSION_B, &[
        (ITEM_TASK,   "task",   "Recognized"),
        (ITEM_WIDGET, "widget", "Unrecognized"),
    ]);

    run_binary_isolated(&dir, &["--type", "full"]);

    let all = read_all_events(&dir);
    let report_cid = all.iter()
        .find(|e| e["source_module"].as_str() == Some("report_export")
            && e["event_type"].as_str() == Some("ReportRequested"))
        .and_then(|e| e["correlation_id"].as_str())
        .expect("ReportRequested must be present");

    let unknown = all.iter()
        .find(|e| e["source_module"].as_str() == Some("project_schema")
            && e["event_type"].as_str() == Some("SchemaTypeUnknown"))
        .expect("SchemaTypeUnknown must be emitted");

    assert_eq!(unknown["correlation_id"].as_str().unwrap(), report_cid,
        "SchemaTypeUnknown must share the correlation_id of the report invocation");
}

#[test]
fn test_r8_unrecognized_exclusion_does_not_trigger_empty_record() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, BASIC_VOCAB);
    seed_incorporated_items(&dir, SESSION_B, &[
        (ITEM_TASK,   "task",   "Recognized"),
        (ITEM_WIDGET, "widget", "Unrecognized"),
    ]);

    run_binary_isolated(&dir, &["--type", "full"]);

    let events = read_re_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();
    assert!(!types.contains(&"ReportFailedEmptyRecord"),
        "exclusion of unrecognized items must NOT trigger EmptyRecord");
    assert!(types.contains(&"ReportGenerated"),
        "command must complete successfully");
}

// ── HP4: Section omitted when no recognized items exist for that type ─────────

#[test]
fn test_r8_full_report_omits_section_for_empty_canonical_type() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, BASIC_VOCAB);
    seed_incorporated_items(&dir, SESSION_B, &[
        (ITEM_TASK, "task", "Only a task — no risks seeded"),
    ]);

    let output = run_binary_isolated(&dir, &["--type", "full"]);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("## Tasks"),     "Tasks section must be present");
    assert!(!stdout.contains("## Risks"),    "Risks section must be omitted when no risks exist");
    assert!(!stdout.contains("## Milestones"), "Milestones section must be omitted");
}

#[test]
fn test_r8_partial_sections_does_not_cause_failure() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, BASIC_VOCAB);
    seed_incorporated_items(&dir, SESSION_B, &[
        (ITEM_TASK, "task", "Only a task"),
    ]);

    run_binary_isolated(&dir, &["--type", "full"]);

    let events = read_re_events(&dir);
    assert!(events.iter().any(|e| e["event_type"] == "ReportGenerated"),
        "ReportGenerated must be emitted when some sections are omitted");
    assert!(!events.iter().any(|e| {
        e["event_type"].as_str().map(|t| t.starts_with("ReportFailed")).unwrap_or(false)
    }), "no failure event must be emitted for omitted sections");
}

// ── HP5: All items excluded → empty report, not EmptyRecord ──────────────────

#[test]
fn test_r8_all_items_unrecognized_produces_empty_report_not_empty_record_failure() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, BASIC_VOCAB);
    seed_incorporated_items(&dir, SESSION_B, &[
        (ITEM_WIDGET,  "widget", "Unrecognized one"),
        (ITEM_WIDGET2, "gadget", "Unrecognized two"),
    ]);

    run_binary_isolated(&dir, &["--type", "full"]);

    let events = read_re_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();
    assert!(!types.contains(&"ReportFailedEmptyRecord"),
        "EmptyRecord must NOT fire — the record has items, they are just unrecognized");
    assert!(types.contains(&"ReportGenerated"),
        "ReportGenerated must be emitted even when all items are excluded");
}

#[test]
fn test_r8_all_items_excluded_item_count_is_zero() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, BASIC_VOCAB);
    seed_incorporated_items(&dir, SESSION_B, &[
        (ITEM_WIDGET,  "widget", "Unrecognized"),
        (ITEM_WIDGET2, "gadget", "Also unrecognized"),
    ]);

    run_binary_isolated(&dir, &["--type", "full"]);

    let events = read_re_events(&dir);
    let generated = events.iter().find(|e| e["event_type"] == "ReportGenerated")
        .expect("ReportGenerated must be emitted");
    assert_eq!(generated["payload"]["item_count"].as_u64().unwrap(), 0,
        "item_count must be 0 when all items are excluded by the vocabulary");
}

#[test]
fn test_r8_all_items_excluded_schema_type_unknown_emitted_per_item() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, BASIC_VOCAB);
    seed_incorporated_items(&dir, SESSION_B, &[
        (ITEM_WIDGET,  "widget", "Unrecognized one"),
        (ITEM_WIDGET2, "gadget", "Unrecognized two"),
    ]);

    run_binary_isolated(&dir, &["--type", "full"]);

    let all = read_all_events(&dir);
    let unknown_count = all.iter()
        .filter(|e| e["source_module"].as_str() == Some("project_schema")
            && e["event_type"].as_str() == Some("SchemaTypeUnknown"))
        .count();
    assert_eq!(unknown_count, 2,
        "exactly one SchemaTypeUnknown per excluded item, even when all items are excluded");
}

// ── FP1: SchemaInvalid — vocabulary load failure ──────────────────────────────

#[test]
fn test_r8_schema_invalid_report_requested_not_emitted() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, "this: is: not: valid: yaml: [[[");
    seed_full_record(&dir);

    run_binary(&dir, &["--type", "full"]);

    let events = read_re_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();
    assert!(!types.contains(&"ReportRequested"),
        "ReportRequested must NOT be emitted when vocabulary load fails");
    assert!(!types.contains(&"ReportGenerated"),
        "ReportGenerated must NOT be emitted when vocabulary load fails");
}

#[test]
fn test_r8_schema_invalid_emits_project_schema_failure_event() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, "this: is: not: valid: yaml: [[[");
    seed_full_record(&dir);

    run_binary(&dir, &["--type", "full"]);

    let all = read_all_events(&dir);
    let schema_failures: Vec<&Value> = all.iter()
        .filter(|e| e["source_module"].as_str() == Some("project_schema")
            && matches!(e["event_type"].as_str(),
                Some("SchemaParseError") | Some("SchemaValidationFailed")))
        .collect();
    assert!(!schema_failures.is_empty(),
        "project_schema must emit a failure event when the vocabulary file is invalid");
}

#[test]
fn test_r8_schema_invalid_no_stdout_output() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, "this: is: not: valid: yaml: [[[");
    seed_full_record(&dir);

    let output = run_binary(&dir, &["--type", "full"]);

    assert!(output.stdout.is_empty(),
        "no report content must be written to stdout when vocabulary load fails");
}

#[test]
fn test_r8_schema_invalid_no_output_file_created() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, "this: is: not: valid: yaml: [[[");
    seed_full_record(&dir);
    let graph_dir = tempfile::tempdir().unwrap();
    let graph_path = graph_dir.path().to_str().unwrap();

    run_binary(&dir, &["--type", "full", "--graph", graph_path]);

    let entries: Vec<_> = fs::read_dir(graph_dir.path()).unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert!(entries.is_empty(),
        "no report file must be created in --graph directory when vocabulary load fails");
}

// ── Regression: capitalized canonical names (real-schema casing) ─────────────
//
// The default user schema defines canonical names with capital initials (Risk,
// Task, Stakeholder) and lowercase as aliases. Fixed-scope reports must resolve
// their target type through the schema rather than comparing against hardcoded
// lowercase strings. These tests use CAPITALIZED_VOCAB to ensure the fix holds.

#[test]
fn test_regression_risk_register_with_capitalized_canonical() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, CAPITALIZED_VOCAB);
    // Item stored under lowercase alias "risk" → canonical is "Risk"
    seed_incorporated_items(&dir, SESSION_B, &[
        (ITEM_RISK, "risk", "Alias-stored risk item"),
    ]);

    run_binary_isolated(&dir, &["--type", "risk-register"]);

    let events = read_re_events(&dir);
    let generated = events.iter().find(|e| e["event_type"] == "ReportGenerated")
        .expect("ReportGenerated must be emitted");
    assert_eq!(generated["payload"]["item_count"].as_u64().unwrap(), 1,
        "risk-register must include items stored under the lowercase alias of a capitalized canonical");
}

#[test]
fn test_regression_stakeholders_with_capitalized_canonical() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, CAPITALIZED_VOCAB);
    seed_incorporated_items(&dir, SESSION_B, &[
        (ITEM_STAKEHOLDER, "stakeholder", "Alias-stored stakeholder"),
    ]);

    run_binary_isolated(&dir, &["--type", "stakeholders"]);

    let events = read_re_events(&dir);
    let generated = events.iter().find(|e| e["event_type"] == "ReportGenerated")
        .expect("ReportGenerated must be emitted");
    assert_eq!(generated["payload"]["item_count"].as_u64().unwrap(), 1,
        "stakeholders must include items stored under the lowercase alias of a capitalized canonical");
}

#[test]
fn test_regression_weekly_includes_task_risk_milestone_with_capitalized_canonicals() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, CAPITALIZED_VOCAB);
    seed_incorporated_items(&dir, SESSION_B, &[
        (ITEM_TASK,      "task",      "Alias task"),
        (ITEM_RISK,      "risk",      "Alias risk"),
        (ITEM_MILESTONE, "milestone", "Alias milestone"),
    ]);

    let output = run_binary_isolated(&dir, &["--type", "weekly"]);

    let stdout = String::from_utf8_lossy(&output.stdout);
    // All three fixed scopes must populate when items are stored under lowercase aliases
    assert!(!stdout.contains("_No open tasks._"),
        "weekly Open Tasks section must include alias-stored task items");
    assert!(!stdout.contains("_No open risks._"),
        "weekly Open Risks section must include alias-stored risk items");
    assert!(!stdout.contains("_No milestones._"),
        "weekly Milestones section must include alias-stored milestone items");
}

#[test]
fn test_regression_full_report_capitalized_canonical_section_headers() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, CAPITALIZED_VOCAB);
    seed_incorporated_items(&dir, SESSION_B, &[
        (ITEM_RISK,        "risk",        "Risk via alias"),
        (ITEM_STAKEHOLDER, "stakeholder", "Stakeholder via alias"),
    ]);

    let output = run_binary_isolated(&dir, &["--type", "full"]);

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Section headers use the capitalized canonical name from the vocabulary
    assert!(stdout.contains("## Risks"),        "full report must use canonical 'Risk' as section header");
    assert!(stdout.contains("## Stakeholders"), "full report must use canonical 'Stakeholder' as section header");
}

// ── R8 behavioral amendment: vocabulary gate before ReportRequested ───────────

#[test]
fn test_r8_successful_vocabulary_load_allows_report_requested() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, BASIC_VOCAB);
    seed_incorporated_items(&dir, SESSION_B, &[
        (ITEM_TASK, "task", "A task item"),
    ]);

    run_binary_isolated(&dir, &["--type", "full"]);

    let all = read_all_events(&dir);
    let schema_failures: Vec<&Value> = all.iter()
        .filter(|e| e["source_module"].as_str() == Some("project_schema")
            && matches!(e["event_type"].as_str(),
                Some("SchemaParseError") | Some("SchemaValidationFailed") | Some("SchemaNotFound")))
        .collect();
    assert!(schema_failures.is_empty(),
        "no schema failure events when vocabulary loads successfully");

    let events = read_re_events(&dir);
    assert!(events.iter().any(|e| e["event_type"] == "ReportRequested"),
        "ReportRequested must be emitted after successful vocabulary load");
}

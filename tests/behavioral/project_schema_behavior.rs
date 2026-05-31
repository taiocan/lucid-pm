//! Behavioral tests for project_schema.
//!
//! Tests verify observable outcomes: validate/show command exit codes,
//! events emitted with correct types and payloads, and library API behavior.
//! All event type names reference events/project_schema_schema.md exactly.

use project_schema::{
    emit_type_unknown, is_valid_status, load_schema_str, logseq_forward_label,
    logseq_inverse_label, logseq_property_key, marker_to_status, resolve_type, validate,
};
use serde_json::Value;
use std::fs;
use std::process::Command;
use tempfile::TempDir;

// ─── Test helpers ─────────────────────────────────────────────────────────────

fn binary_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_project_schema"))
}

fn setup_project_dir() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("events")).unwrap();
    dir
}

fn setup_fake_home_with_default(default_yaml: &str) -> TempDir {
    let home = tempfile::tempdir().unwrap();
    let lucidpm = home.path().join(".lucidpm");
    fs::create_dir_all(&lucidpm).unwrap();
    fs::write(lucidpm.join("default-schema.yaml"), default_yaml).unwrap();
    home
}

fn run_cmd(
    args: &[&str],
    _project_dir: &std::path::Path,
    fake_home: &std::path::Path,
) -> std::process::Output {
    Command::new(binary_path())
        .args(args)
        .env("HOME", fake_home)
        .output()
        .unwrap()
}

fn read_events(project_dir: &std::path::Path) -> Vec<Value> {
    let path = project_dir.join("events/runtime_events.jsonl");
    if !path.exists() {
        return vec![];
    }
    fs::read_to_string(&path)
        .unwrap()
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

fn write_project_schema(project_dir: &std::path::Path, yaml: &str) {
    fs::write(project_dir.join("project-schema.yaml"), yaml).unwrap();
}

const MINIMAL_VALID_SCHEMA: &str = r#"schemaVersion: 1
properties:
  status:
    type: enum
statuses:
  active:
pageTypes:
  WorkPackage:
    uses:
      - status
relations: {}
"#;

/// Schema used for library unit tests. No file I/O or HOME needed.
const UNIT_TEST_SCHEMA: &str = r#"schemaVersion: 1
properties:
  status:
    type: enum
  deadline:
    type: date
statuses:
  active:
  done:
pageTypes:
  WorkPackage:
    uses:
      - status
      - deadline
    aliases:
      - wp
      - workpackage
  Issue:
    uses:
      - status
blockTypes:
  Task:
    markers:
      TODO: active
      DOING: active
      DONE: done
    uses:
      - deadline
relations:
  blocks:
    source:
      - WorkPackage
    target:
      - WorkPackage
  relatedTo:
    source:
      - any
    target:
      - any
renderers:
  logseq:
    relations:
      blocks:
        forwardLabel: "Blocks"
        inverseLabel: "Blocked By"
      relatedTo:
        forwardLabel: "Related To"
        inverseLabel: "Related To"
    properties:
      status: status
      deadline: deadline
"#;

// ─── Happy Path: validate command ─────────────────────────────────────────────

/// Contract: Happy Path 1 — valid vocabulary loaded, command succeeds.
#[test]
fn test_validate_succeeds_with_valid_schema() {
    let project_dir = setup_project_dir();
    let fake_home = tempfile::tempdir().unwrap();
    write_project_schema(project_dir.path(), MINIMAL_VALID_SCHEMA);

    let output = run_cmd(
        &["--project-dir", project_dir.path().to_str().unwrap(), "validate"],
        project_dir.path(),
        fake_home.path(),
    );

    assert!(
        output.status.success(),
        "validate should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("Schema OK"),
        "stdout should report Schema OK"
    );
    assert!(
        read_events(project_dir.path()).is_empty(),
        "no events emitted on successful validate"
    );
}

/// Contract: Happy Path 3 — no project schema, global default used.
#[test]
fn test_validate_falls_back_to_global_default() {
    let project_dir = setup_project_dir();
    let fake_home = setup_fake_home_with_default(MINIMAL_VALID_SCHEMA);
    // No project-schema.yaml written

    let output = run_cmd(
        &["--project-dir", project_dir.path().to_str().unwrap(), "validate"],
        project_dir.path(),
        fake_home.path(),
    );

    assert!(
        output.status.success(),
        "validate should succeed using global default: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("Schema OK"));
}

/// Contract: Happy Path 2 — project schema merges with global default (maps merge recursively).
#[test]
fn test_project_schema_merges_with_global_default() {
    let project_dir = setup_project_dir();
    // Default has WorkPackage and status 'active'
    let fake_home = setup_fake_home_with_default(MINIMAL_VALID_SCHEMA);
    // Project adds Milestone and status 'done' (both merged on top of default)
    write_project_schema(
        project_dir.path(),
        r#"schemaVersion: 1
properties:
  status:
    type: enum
statuses:
  done:
pageTypes:
  Milestone:
    uses:
      - status
relations: {}
"#,
    );

    let output = run_cmd(
        &["--project-dir", project_dir.path().to_str().unwrap(), "show"],
        project_dir.path(),
        fake_home.path(),
    );

    assert!(output.status.success());
    let yaml = String::from_utf8_lossy(&output.stdout);
    assert!(yaml.contains("Milestone"), "project type Milestone should appear");
    assert!(yaml.contains("WorkPackage"), "default type WorkPackage retained (maps merge)");
    assert!(yaml.contains("done"), "project status 'done' should appear");
    assert!(yaml.contains("active"), "default status 'active' retained (maps merge)");
}

// ─── Failure Mode Tests ───────────────────────────────────────────────────────

/// Contract: Failure Path 1 — SchemaNotFound.
#[test]
fn test_schema_not_found_emits_failure_event() {
    let project_dir = setup_project_dir();
    let fake_home = tempfile::tempdir().unwrap(); // empty home, no .lucidpm/

    let output = run_cmd(
        &["--project-dir", project_dir.path().to_str().unwrap(), "validate"],
        project_dir.path(),
        fake_home.path(),
    );

    assert!(!output.status.success(), "command should fail");

    let events = read_events(project_dir.path());
    assert_eq!(events.len(), 1, "exactly one failure event expected");
    assert_eq!(events[0]["event_type"], "SchemaNotFound");
    assert_eq!(events[0]["source_module"], "project_schema");
    assert_eq!(events[0]["payload"]["failure_reason"], "schema_not_found");
    assert!(
        events[0]["payload"]["searched_locations"].is_array(),
        "searched_locations must be an array"
    );
}

/// Contract: Failure Path 2 — SchemaParseError.
#[test]
fn test_parse_error_emits_schema_parse_error_event() {
    let project_dir = setup_project_dir();
    let fake_home = tempfile::tempdir().unwrap();
    write_project_schema(project_dir.path(), "invalid: yaml: [unclosed bracket");

    let output = run_cmd(
        &["--project-dir", project_dir.path().to_str().unwrap(), "validate"],
        project_dir.path(),
        fake_home.path(),
    );

    assert!(!output.status.success());

    let events = read_events(project_dir.path());
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["event_type"], "SchemaParseError");
    assert_eq!(events[0]["source_module"], "project_schema");
    assert_eq!(events[0]["payload"]["failure_reason"], "schema_parse_error");
    assert!(
        events[0]["payload"]["detail"].as_str().is_some_and(|s| !s.is_empty()),
        "detail must describe the parse failure"
    );
}

/// Contract: Failure Path 3 — SchemaValidationFailed (undefined property ref).
#[test]
fn test_undefined_property_ref_emits_schema_validation_failed() {
    let project_dir = setup_project_dir();
    let fake_home = tempfile::tempdir().unwrap();
    write_project_schema(
        project_dir.path(),
        r#"schemaVersion: 1
properties:
  status:
    type: enum
statuses:
  active:
pageTypes:
  WorkPackage:
    uses:
      - nonexistent_prop
"#,
    );

    let output = run_cmd(
        &["--project-dir", project_dir.path().to_str().unwrap(), "validate"],
        project_dir.path(),
        fake_home.path(),
    );

    assert!(!output.status.success());

    let events = read_events(project_dir.path());
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["event_type"], "SchemaValidationFailed");
    assert_eq!(events[0]["payload"]["failure_reason"], "schema_validation_failed");
    assert_eq!(events[0]["payload"]["violated_rule"], "undefined_property_ref");
    assert!(events[0]["payload"]["detail"].as_str().is_some());
}

/// Contract: Failure Path 3 — SchemaValidationFailed (undefined relation in renderer).
#[test]
fn test_undefined_renderer_relation_emits_schema_validation_failed() {
    let project_dir = setup_project_dir();
    let fake_home = tempfile::tempdir().unwrap();
    write_project_schema(
        project_dir.path(),
        r#"schemaVersion: 1
properties:
  status:
    type: enum
statuses:
  active:
pageTypes:
  WorkPackage:
    uses: [status]
relations: {}
renderers:
  logseq:
    relations:
      ghostRelation:
        forwardLabel: "Ghost"
        inverseLabel: "Haunted By"
"#,
    );

    let output = run_cmd(
        &["--project-dir", project_dir.path().to_str().unwrap(), "validate"],
        project_dir.path(),
        fake_home.path(),
    );

    assert!(!output.status.success());
    let events = read_events(project_dir.path());
    assert_eq!(events[0]["event_type"], "SchemaValidationFailed");
    assert_eq!(events[0]["payload"]["violated_rule"], "undefined_relation_ref");
}

/// Contract: Failure Path 4 — AliasCollision.
#[test]
fn test_alias_collision_emits_schema_alias_collision_detected() {
    let project_dir = setup_project_dir();
    let fake_home = tempfile::tempdir().unwrap();
    write_project_schema(
        project_dir.path(),
        r#"schemaVersion: 1
properties:
  status:
    type: enum
statuses:
  active:
pageTypes:
  WorkPackage:
    uses: [status]
  Feature:
    uses: [status]
    aliases:
      - WorkPackage
"#,
    );

    let output = run_cmd(
        &["--project-dir", project_dir.path().to_str().unwrap(), "validate"],
        project_dir.path(),
        fake_home.path(),
    );

    assert!(!output.status.success());

    let events = read_events(project_dir.path());
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["event_type"], "SchemaAliasCollisionDetected");
    assert_eq!(events[0]["source_module"], "project_schema");
    assert_eq!(events[0]["payload"]["failure_reason"], "alias_collision");
    // Two-phase validation: canonical names registered first, then aliases checked.
    // Feature's alias "WorkPackage" collides with the canonical pageType "WorkPackage".
    assert_eq!(events[0]["payload"]["alias_value"], "WorkPackage");
    assert_eq!(events[0]["payload"]["collides_with"], "pageType 'WorkPackage'");
}

// ─── Non-aborting condition: SchemaTypeUnknown ────────────────────────────────

/// Contract: Non-aborting — SchemaTypeUnknown emitted per unrecognized item type.
#[test]
fn test_schema_type_unknown_event_emitted_by_library() {
    let project_dir = setup_project_dir();
    let events_file = project_dir.path().join("events/runtime_events.jsonl");

    emit_type_unknown(&events_file, "item-abc-123", "LegacyWorkstream", "corr-test-001");

    let events = read_events(project_dir.path());
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["event_type"], "SchemaTypeUnknown");
    assert_eq!(events[0]["source_module"], "project_schema");
    assert_eq!(events[0]["payload"]["item_id"], "item-abc-123");
    assert_eq!(events[0]["payload"]["unknown_type"], "LegacyWorkstream");
    // Command continues: no exit code test needed — this is a library call
}

// ─── Telemetry Tests ──────────────────────────────────────────────────────────

/// Contract invariant: all emitted events carry required base fields.
#[test]
fn test_failure_events_have_all_required_base_fields() {
    let project_dir = setup_project_dir();
    let fake_home = tempfile::tempdir().unwrap();
    // Trigger SchemaNotFound to get an event
    let _ = run_cmd(
        &["--project-dir", project_dir.path().to_str().unwrap(), "validate"],
        project_dir.path(),
        fake_home.path(),
    );

    let events = read_events(project_dir.path());
    assert!(!events.is_empty(), "at least one event must be emitted");

    for event in &events {
        assert!(
            event["event_id"].as_str().is_some_and(|s| !s.is_empty()),
            "event_id must be present and non-empty"
        );
        assert!(
            event["event_type"].as_str().is_some_and(|s| !s.is_empty()),
            "event_type must be present and non-empty"
        );
        assert!(event["timestamp"].as_u64().is_some(), "timestamp must be a u64");
        assert!(
            event["correlation_id"].as_str().is_some_and(|s| !s.is_empty()),
            "correlation_id must be present and non-empty"
        );
        assert_eq!(event["source_module"], "project_schema", "source_module must be project_schema");
        assert!(event["payload"].is_object(), "payload must be an object");
    }
}

#[test]
fn test_schema_type_unknown_has_all_required_base_fields() {
    let project_dir = setup_project_dir();
    let events_file = project_dir.path().join("events/runtime_events.jsonl");

    emit_type_unknown(&events_file, "item-xyz", "OldType", "corr-telemetry-test");

    let events = read_events(project_dir.path());
    let event = &events[0];
    assert!(event["event_id"].as_str().is_some_and(|s| !s.is_empty()));
    assert_eq!(event["event_type"], "SchemaTypeUnknown");
    assert!(event["timestamp"].as_u64().is_some());
    assert!(event["correlation_id"].as_str().is_some_and(|s| !s.is_empty()));
    assert_eq!(event["source_module"], "project_schema");
}

// ─── Library Unit Tests — Contract: Happy Path 4 (alias resolution) ──────────

fn unit_schema() -> project_schema::ProjectSchema {
    load_schema_str(UNIT_TEST_SCHEMA).expect("unit test schema must be valid")
}

/// Contract: Happy Path 4 — renamed type (alias) → existing items accessible.
#[test]
fn test_resolve_type_returns_canonical_name() {
    let schema = unit_schema();
    assert_eq!(resolve_type(&schema, "WorkPackage"), Some("WorkPackage"));
    assert_eq!(resolve_type(&schema, "Issue"), Some("Issue"));
    assert_eq!(resolve_type(&schema, "Task"), Some("Task")); // block type
}

#[test]
fn test_resolve_type_follows_alias_to_canonical() {
    let schema = unit_schema();
    assert_eq!(resolve_type(&schema, "wp"), Some("WorkPackage"));
    assert_eq!(resolve_type(&schema, "workpackage"), Some("WorkPackage"));
}

#[test]
fn test_resolve_type_returns_none_for_unknown_type() {
    let schema = unit_schema();
    assert_eq!(resolve_type(&schema, "UnknownType"), None);
    assert_eq!(resolve_type(&schema, ""), None);
}

// ─── Library Unit Tests — Contract: Happy Path 5 (marker normalization) ───────

#[test]
fn test_marker_to_status_maps_todo_to_active() {
    let schema = unit_schema();
    assert_eq!(marker_to_status(&schema, "TODO"), Some("active"));
    assert_eq!(marker_to_status(&schema, "DOING"), Some("active"));
}

#[test]
fn test_marker_to_status_maps_done() {
    let schema = unit_schema();
    assert_eq!(marker_to_status(&schema, "DONE"), Some("done"));
}

#[test]
fn test_marker_to_status_returns_none_for_unknown_marker() {
    let schema = unit_schema();
    assert_eq!(marker_to_status(&schema, "UNKNOWN"), None);
    assert_eq!(marker_to_status(&schema, ""), None);
}

// ─── Library Unit Tests — Status validation ───────────────────────────────────

#[test]
fn test_is_valid_status_accepts_global_vocabulary() {
    let schema = unit_schema();
    assert!(is_valid_status(&schema, "WorkPackage", "active"));
    assert!(is_valid_status(&schema, "WorkPackage", "done"));
}

#[test]
fn test_is_valid_status_rejects_unknown_status() {
    let schema = unit_schema();
    assert!(!is_valid_status(&schema, "WorkPackage", "open")); // not in schema
    assert!(!is_valid_status(&schema, "WorkPackage", ""));
}

// ─── Library Unit Tests — Renderer labels ─────────────────────────────────────

#[test]
fn test_logseq_forward_and_inverse_labels_from_schema() {
    let schema = unit_schema();
    assert_eq!(logseq_forward_label(&schema, "blocks"), "Blocks");
    assert_eq!(logseq_inverse_label(&schema, "blocks"), "Blocked By");
    assert_eq!(logseq_forward_label(&schema, "relatedTo"), "Related To");
    assert_eq!(logseq_inverse_label(&schema, "relatedTo"), "Related To");
}

#[test]
fn test_logseq_label_falls_back_to_relation_name_for_unknown() {
    let schema = unit_schema();
    assert_eq!(logseq_forward_label(&schema, "unknownRel"), "unknownRel");
    assert_eq!(logseq_inverse_label(&schema, "unknownRel"), "unknownRel");
}

#[test]
fn test_logseq_property_key_mapping() {
    let schema = unit_schema();
    assert_eq!(logseq_property_key(&schema, "status"), "status");
    assert_eq!(logseq_property_key(&schema, "deadline"), "deadline");
}

#[test]
fn test_logseq_property_key_falls_back_for_unknown() {
    let schema = unit_schema();
    assert_eq!(logseq_property_key(&schema, "unknownProp"), "unknownProp");
}

// ─── Library Unit Tests — validate() ─────────────────────────────────────────

#[test]
fn test_validate_passes_for_correct_schema() {
    let schema = unit_schema();
    assert!(validate(&schema).is_ok());
}

#[test]
fn test_validate_detects_undefined_property_ref_in_uses() {
    let schema = load_schema_str(r#"schemaVersion: 1
properties:
  status:
    type: enum
statuses:
  active:
pageTypes:
  WorkPackage:
    uses:
      - ghost_property
"#).unwrap();
    let err = validate(&schema).unwrap_err();
    assert!(matches!(err, project_schema::SchemaError::ValidationFailed { .. }));
}

#[test]
fn test_validate_detects_alias_collision() {
    let schema = load_schema_str(r#"schemaVersion: 1
properties:
  status:
    type: enum
statuses:
  active:
pageTypes:
  WorkPackage:
    uses: [status]
  Feature:
    uses: [status]
    aliases:
      - WorkPackage
"#).unwrap();
    let err = validate(&schema).unwrap_err();
    match err {
        project_schema::SchemaError::AliasCollision { alias_value, collides_with } => {
            assert_eq!(alias_value, "WorkPackage");
            assert_eq!(collides_with, "pageType 'WorkPackage'");
        }
        other => panic!("expected AliasCollision, got {:?}", other),
    }
}

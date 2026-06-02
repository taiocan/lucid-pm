//! Behavioral tests for item_links and item_links_schema_integration.
//!
//! Tests verify observable outcomes: events emitted, payload shapes, link
//! visibility after add/remove, failure modes, and schema integration invariants.
//! All assertions reference event names from events/item_links_schema.md and
//! events/item_links_schema_integration_schema.md exactly.

use serde_json::{json, Value};
use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};
use tempfile::TempDir;

fn binary_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_item_links"))
}

fn setup_temp_dir() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("events")).unwrap();
    dir
}

fn write_project_schema(dir: &TempDir, yaml: &str) {
    fs::write(dir.path().join("project-schema.yaml"), yaml).unwrap();
}

fn seed_incorporated_items(dir: &TempDir, session_id: &str, items: &[(&str, &str, &str)]) {
    let items_json: Vec<Value> = items.iter().map(|(id, typ, desc)| json!({
        "item_id": id, "item_type": typ, "description": desc,
        "uncertain": false, "uncertainty_reason": null,
        "proposed_status": null, "proposed_priority": null,
    })).collect();
    let accepted_ids: Vec<&str> = items.iter().map(|(id, _, _)| *id).collect();
    let path = dir.path().join("events/runtime_events.jsonl");
    let mut file = fs::OpenOptions::new().create(true).append(true).open(&path).unwrap();
    writeln!(file, "{}", json!({
        "event_id": format!("seed-ext-{}", &session_id[..8]),
        "event_type": "ItemsExtracted", "timestamp": 1748000001000u64,
        "correlation_id": session_id, "source_module": "pm_structuring",
        "payload": { "items": items_json, "item_count": items.len(), "uncertain_count": 0 }
    })).unwrap();
    writeln!(file, "{}", json!({
        "event_id": format!("seed-conf-{}", &session_id[..8]),
        "event_type": "ExtractionConfirmed", "timestamp": 1748000002000u64,
        "correlation_id": session_id, "source_module": "pm_structuring",
        "payload": { "accepted_item_ids": accepted_ids, "accepted_count": items.len() }
    })).unwrap();
    writeln!(file, "{}", json!({
        "event_id": format!("seed-inc-{}", &session_id[..8]),
        "event_type": "ItemsIncorporated", "timestamp": 1748000003000u64,
        "correlation_id": "00000000-0000-0000-0000-000000000001",
        "source_module": "project_state",
        "payload": { "session_id": session_id, "incorporated_count": items.len(), "total_record_size": items.len() }
    })).unwrap();
}

fn seed_item_linked(dir: &TempDir, source_id: &str, link_type: &str, target_id: &str) {
    let path = dir.path().join("events/runtime_events.jsonl");
    let mut file = fs::OpenOptions::new().create(true).append(true).open(&path).unwrap();
    writeln!(file, "{}", json!({
        "event_id": format!("seed-link-{}", &source_id[..8]),
        "event_type": "ItemLinked", "timestamp": 1748000010000u64,
        "correlation_id": "00000000-0000-0000-0000-000000000002",
        "source_module": "item_links",
        "payload": {
            "source_id": source_id, "source_type": "task",
            "link_type": link_type,
            "target_id": target_id, "target_type": "milestone"
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

/// Events from item_links module only.
fn read_il_events(dir: &TempDir) -> Vec<Value> {
    let path = dir.path().join("events/runtime_events.jsonl");
    if !path.exists() { return vec![]; }
    fs::read_to_string(path).unwrap()
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .filter(|e| e["source_module"].as_str() == Some("item_links"))
        .collect()
}

/// All events, including cross-module (project_schema, etc.).
fn read_all_events(dir: &TempDir) -> Vec<Value> {
    let path = dir.path().join("events/runtime_events.jsonl");
    if !path.exists() { return vec![]; }
    fs::read_to_string(path).unwrap()
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .collect()
}

const SESSION_A:      &str = "a4ca3a7e-61eb-4f36-b59e-f3abd166e351";
const ITEM_TASK:      &str = "d1000000-0000-0000-0000-000000000001";
const ITEM_MILESTONE: &str = "d1000000-0000-0000-0000-000000000002";
const ITEM_RISK:      &str = "d1000000-0000-0000-0000-000000000003";
const ITEM_HOLDER:    &str = "d1000000-0000-0000-0000-000000000004";
const ITEM_ISSUE:     &str = "d1000000-0000-0000-0000-000000000005";

fn seed_full_record(dir: &TempDir) {
    seed_incorporated_items(dir, SESSION_A, &[
        (ITEM_TASK,      "task",        "Fix critical bug"),
        (ITEM_MILESTONE, "milestone",   "Q3 release"),
        (ITEM_RISK,      "risk",        "Vendor lock-in risk"),
        (ITEM_HOLDER,    "stakeholder", "Engineering lead"),
        (ITEM_ISSUE,     "issue",       "Login page is slow"),
    ]);
}

// ── Happy Path 1: Add a link ──────────────────────────────────────────────────

#[test]
fn test_add_link_emits_requested_then_linked() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);

    run_binary(&dir, &["add", ITEM_TASK, "blocks", ITEM_MILESTONE]);

    let events = read_il_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"LinkAddRequested"), "LinkAddRequested must be emitted");
    assert!(types.contains(&"ItemLinked"),       "ItemLinked must be emitted");
    assert!(!types.iter().any(|t| t.starts_with("LinkFailed")),
        "no failure event must be emitted on a valid add");

    let req_pos = types.iter().position(|&t| t == "LinkAddRequested").unwrap();
    let lnk_pos = types.iter().position(|&t| t == "ItemLinked").unwrap();
    assert!(req_pos < lnk_pos, "LinkAddRequested must precede ItemLinked");
}

#[test]
fn test_add_link_requested_payload_shape() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);

    run_binary(&dir, &["add", ITEM_TASK, "blocks", ITEM_MILESTONE]);

    let events = read_il_events(&dir);
    let req = events.iter().find(|e| e["event_type"] == "LinkAddRequested").unwrap();
    let p = &req["payload"];

    assert_eq!(p["source_id"].as_str().unwrap(), ITEM_TASK);
    assert_eq!(p["link_type"].as_str().unwrap(), "blocks");
    assert_eq!(p["target_id"].as_str().unwrap(), ITEM_MILESTONE);
}

#[test]
fn test_add_link_itemlinked_payload_shape() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);

    run_binary(&dir, &["add", ITEM_TASK, "blocks", ITEM_MILESTONE]);

    let events = read_il_events(&dir);
    let linked = events.iter().find(|e| e["event_type"] == "ItemLinked").unwrap();
    let p = &linked["payload"];

    assert_eq!(p["source_id"].as_str().unwrap(),  ITEM_TASK);
    assert_eq!(p["source_type"].as_str().unwrap(), "task");
    assert_eq!(p["link_type"].as_str().unwrap(),   "blocks");
    assert_eq!(p["target_id"].as_str().unwrap(),   ITEM_MILESTONE);
    assert_eq!(p["target_type"].as_str().unwrap(), "milestone");
}

#[test]
fn test_add_link_then_list_shows_link() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);

    run_binary(&dir, &["add", ITEM_TASK, "blocks", ITEM_MILESTONE]);
    run_binary(&dir, &["list"]);

    let events = read_il_events(&dir);
    let returned = events.iter().find(|e| e["event_type"] == "LinkListReturned").unwrap();
    let links = returned["payload"]["links"].as_array().unwrap();

    assert_eq!(returned["payload"]["link_count"].as_u64().unwrap(), 1);
    assert!(links.iter().any(|l|
        l["source_id"].as_str() == Some(ITEM_TASK)
        && l["link_type"].as_str() == Some("blocks")
        && l["target_id"].as_str() == Some(ITEM_MILESTONE)
    ), "added link must appear in list");
}

// ── Happy Path 2: Labels from vocabulary ──────────────────────────────────────

#[test]
fn test_list_labels_come_from_vocabulary_not_hardcoded() {
    // Custom schema with non-default labels for 'blocks'
    const CUSTOM_SCHEMA: &str = r#"schemaVersion: 1
properties:
  status:
    type: enum
statuses:
  active:
pageTypes:
  Task:
    uses: [status]
    aliases: [task]
  Milestone:
    uses: [status]
    aliases: [milestone]
relations:
  blocks:
    source: []
    target: []
renderers:
  logseq:
    relations:
      blocks:
        forwardLabel: "Precedes"
        inverseLabel: "Follows"
"#;
    let dir = setup_temp_dir();
    write_project_schema(&dir, CUSTOM_SCHEMA);
    seed_full_record(&dir);

    run_binary(&dir, &["add", ITEM_TASK, "blocks", ITEM_MILESTONE]);

    run_binary(&dir, &["list", "--item", ITEM_TASK]);
    let events = read_il_events(&dir);
    let returned = events.iter().find(|e| e["event_type"] == "LinkListReturned").unwrap();
    let links = returned["payload"]["links"].as_array().unwrap();

    assert_eq!(links.len(), 1);
    assert_eq!(links[0]["display_label"].as_str().unwrap(), "Precedes",
        "forward label must come from vocabulary, not hardcoded 'Blocks'");
}

#[test]
fn test_list_inverse_label_comes_from_vocabulary() {
    const CUSTOM_SCHEMA: &str = r#"schemaVersion: 1
properties:
  status:
    type: enum
statuses:
  active:
pageTypes:
  Task:
    uses: [status]
    aliases: [task]
  Milestone:
    uses: [status]
    aliases: [milestone]
relations:
  blocks:
    source: []
    target: []
renderers:
  logseq:
    relations:
      blocks:
        forwardLabel: "Precedes"
        inverseLabel: "Follows"
"#;
    let dir = setup_temp_dir();
    write_project_schema(&dir, CUSTOM_SCHEMA);
    seed_full_record(&dir);

    run_binary(&dir, &["add", ITEM_TASK, "blocks", ITEM_MILESTONE]);
    run_binary(&dir, &["list", "--item", ITEM_MILESTONE]);

    let events = read_il_events(&dir);
    let returned = events.iter().find(|e| e["event_type"] == "LinkListReturned").unwrap();
    let links = returned["payload"]["links"].as_array().unwrap();

    assert_eq!(links.len(), 1);
    assert_eq!(links[0]["display_label"].as_str().unwrap(), "Follows",
        "inverse label must come from vocabulary, not hardcoded 'Blocked By'");
}

// ── Happy Path 3: Unrecognized relation type excluded with LinkRelationTypeUnknown

#[test]
fn test_link_with_unrecognized_relation_emits_link_relation_type_unknown() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);
    // Directly seed an ItemLinked event with a relation type not in the default schema
    seed_item_linked(&dir, ITEM_TASK, "obsolete_link_type", ITEM_MILESTONE);

    run_binary(&dir, &["list"]);

    let events = read_il_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"LinkRelationTypeUnknown"),
        "LinkRelationTypeUnknown must be emitted for each excluded link");
    assert!(types.contains(&"LinkListReturned"),
        "LinkListReturned must still be emitted");
}

#[test]
fn test_link_relation_type_unknown_payload() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);
    seed_item_linked(&dir, ITEM_TASK, "obsolete_link_type", ITEM_MILESTONE);

    run_binary(&dir, &["list"]);

    let events = read_il_events(&dir);
    let unknown = events.iter().find(|e| e["event_type"] == "LinkRelationTypeUnknown").unwrap();
    let p = &unknown["payload"];

    assert_eq!(p["source_id"].as_str().unwrap(), ITEM_TASK);
    assert_eq!(p["link_type"].as_str().unwrap(), "obsolete_link_type");
    assert_eq!(p["target_id"].as_str().unwrap(), ITEM_MILESTONE);
}

#[test]
fn test_excluded_link_absent_from_list_returned() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);
    seed_item_linked(&dir, ITEM_TASK, "obsolete_link_type", ITEM_MILESTONE);

    run_binary(&dir, &["list"]);

    let events = read_il_events(&dir);
    let returned = events.iter().find(|e| e["event_type"] == "LinkListReturned").unwrap();
    let links = returned["payload"]["links"].as_array().unwrap();

    assert!(
        !links.iter().any(|l| l["link_type"].as_str() == Some("obsolete_link_type")),
        "excluded link must not appear in the links array"
    );
}

#[test]
fn test_links_excluded_relation_unknown_count_in_payload() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);
    seed_item_linked(&dir, ITEM_TASK,      "obsolete_link_type", ITEM_MILESTONE);
    seed_item_linked(&dir, ITEM_MILESTONE, "another_old_type",   ITEM_RISK);

    run_binary(&dir, &["list"]);

    let events = read_il_events(&dir);
    let returned = events.iter().find(|e| e["event_type"] == "LinkListReturned").unwrap();
    let excluded = returned["payload"]["links_excluded_relation_unknown"].as_u64().unwrap();

    assert_eq!(excluded, 2,
        "links_excluded_relation_unknown must count each excluded link");
}

#[test]
fn test_links_excluded_relation_unknown_zero_when_all_recognized() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);
    run_binary(&dir, &["add", ITEM_TASK, "blocks", ITEM_MILESTONE]);
    run_binary(&dir, &["list"]);

    let events = read_il_events(&dir);
    let returned = events.iter().find(|e| e["event_type"] == "LinkListReturned").unwrap();
    let excluded = returned["payload"]["links_excluded_relation_unknown"].as_u64().unwrap();

    assert_eq!(excluded, 0,
        "links_excluded_relation_unknown must be 0 when all relation types are recognized");
}

#[test]
fn test_excluded_link_remains_in_record_and_can_be_removed() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);
    seed_item_linked(&dir, ITEM_TASK, "obsolete_link_type", ITEM_MILESTONE);

    // Removal must succeed even though 'obsolete_link_type' is not in vocabulary
    run_binary(&dir, &["remove", ITEM_TASK, "obsolete_link_type", ITEM_MILESTONE]);

    let events = read_il_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"ItemUnlinked"),
        "ItemUnlinked must be emitted — vocabulary evolution must not prevent removal");
    assert!(!types.contains(&"LinkFailedLinkNotFound"),
        "link must be found and removed");
}

// ── Failure Path 1: SchemaInvalid ─────────────────────────────────────────────

#[test]
fn test_schema_invalid_aborts_add_before_link_add_requested() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);
    write_project_schema(&dir, "this: is: not: valid: yaml: [[[");

    run_binary(&dir, &["add", ITEM_TASK, "blocks", ITEM_MILESTONE]);

    let events = read_il_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(!types.contains(&"LinkAddRequested"),
        "LinkAddRequested must NOT be emitted when schema fails to load");
    assert!(!types.contains(&"ItemLinked"),
        "ItemLinked must NOT be emitted when schema fails to load");
}

#[test]
fn test_schema_invalid_aborts_list_before_link_list_requested() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);
    write_project_schema(&dir, "this: is: not: valid: yaml: [[[");

    run_binary(&dir, &["list"]);

    let events = read_il_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(!types.contains(&"LinkListRequested"),
        "LinkListRequested must NOT be emitted when schema fails to load");
    assert!(!types.contains(&"LinkListReturned"),
        "LinkListReturned must NOT be emitted when schema fails to load");
}

#[test]
fn test_schema_invalid_emits_cross_module_failure_event() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);
    write_project_schema(&dir, "this: is: not: valid: yaml: [[[");

    run_binary(&dir, &["add", ITEM_TASK, "blocks", ITEM_MILESTONE]);

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
    seed_full_record(&dir);
    let events_before = read_all_events(&dir).len();

    write_project_schema(&dir, "this: is: not: valid: yaml: [[[");
    run_binary(&dir, &["add", ITEM_TASK, "blocks", ITEM_MILESTONE]);

    let all_after = read_all_events(&dir);
    let il_events: Vec<&Value> = all_after.iter()
        .filter(|e| e["source_module"].as_str() == Some("item_links"))
        .collect();

    assert!(il_events.is_empty(),
        "no item_links events must be appended when schema fails");
    // Only the schema failure event was appended
    assert_eq!(all_after.len(), events_before + 1,
        "only one new event (schema failure) should be in the log");
}

// ── Failure Path 2: InvalidRelationType ──────────────────────────────────────

#[test]
fn test_invalid_relation_type_emits_link_failed_relation_type_unrecognized() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);

    run_binary(&dir, &["add", ITEM_TASK, "owns", ITEM_MILESTONE]);

    let events = read_il_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"LinkAddRequested"),
        "LinkAddRequested must be emitted");
    assert!(types.contains(&"LinkFailedRelationTypeUnrecognized"),
        "LinkFailedRelationTypeUnrecognized must be emitted for unknown relation type");
    assert!(!types.contains(&"ItemLinked"),
        "ItemLinked must NOT be emitted");
}

#[test]
fn test_invalid_relation_type_payload() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);

    run_binary(&dir, &["add", ITEM_TASK, "owns", ITEM_MILESTONE]);

    let events = read_il_events(&dir);
    let failure = events.iter()
        .find(|e| e["event_type"] == "LinkFailedRelationTypeUnrecognized")
        .unwrap();
    let p = &failure["payload"];

    assert_eq!(p["failure_reason"].as_str().unwrap(), "relation_type_unrecognized");
    assert_eq!(p["relation_type"].as_str().unwrap(), "owns",
        "relation_type payload must identify the unrecognized relation type");
}

#[test]
fn test_invalid_relation_type_project_record_unchanged() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);

    run_binary(&dir, &["add", ITEM_TASK, "owns", ITEM_MILESTONE]);
    run_binary(&dir, &["list"]);

    let events = read_il_events(&dir);
    let returned = events.iter().find(|e| e["event_type"] == "LinkListReturned").unwrap();

    assert_eq!(returned["payload"]["link_count"].as_u64().unwrap(), 0,
        "no link must be recorded after InvalidRelationType failure");
}

// ── Failure Path 3: ItemTypeUnrecognized ──────────────────────────────────────

#[test]
fn test_source_item_type_unrecognized_emits_failure() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[
        (ITEM_TASK, "AlienType", "Item with unknown type"),
        (ITEM_MILESTONE, "milestone", "Q3 release"),
    ]);

    run_binary(&dir, &["add", ITEM_TASK, "blocks", ITEM_MILESTONE]);

    let events = read_il_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"LinkFailedItemTypeUnrecognized"),
        "LinkFailedItemTypeUnrecognized must be emitted when source entity type is unrecognized");
    assert!(!types.contains(&"ItemLinked"),
        "ItemLinked must NOT be emitted");
}

#[test]
fn test_target_item_type_unrecognized_emits_failure() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[
        (ITEM_TASK,      "task",      "Fix critical bug"),
        (ITEM_MILESTONE, "AlienType", "Unknown target"),
    ]);

    run_binary(&dir, &["add", ITEM_TASK, "blocks", ITEM_MILESTONE]);

    let events = read_il_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"LinkFailedItemTypeUnrecognized"),
        "LinkFailedItemTypeUnrecognized must be emitted when target entity type is unrecognized");
    assert!(!types.contains(&"ItemLinked"),
        "ItemLinked must NOT be emitted");
}

#[test]
fn test_item_type_unrecognized_source_payload() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[
        (ITEM_TASK,      "AlienType", "Unknown source"),
        (ITEM_MILESTONE, "milestone", "Q3 release"),
    ]);

    run_binary(&dir, &["add", ITEM_TASK, "blocks", ITEM_MILESTONE]);

    let events = read_il_events(&dir);
    let failure = events.iter()
        .find(|e| e["event_type"] == "LinkFailedItemTypeUnrecognized")
        .unwrap();
    let p = &failure["payload"];

    assert_eq!(p["failure_reason"].as_str().unwrap(), "item_type_unrecognized");
    assert_eq!(p["item_id"].as_str().unwrap(),    ITEM_TASK);
    assert_eq!(p["item_type"].as_str().unwrap(),  "AlienType");
    assert_eq!(p["role"].as_str().unwrap(),       "source");
}

#[test]
fn test_item_type_unrecognized_target_payload() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[
        (ITEM_TASK,      "task",      "Fix critical bug"),
        (ITEM_MILESTONE, "AlienType", "Unknown target"),
    ]);

    run_binary(&dir, &["add", ITEM_TASK, "blocks", ITEM_MILESTONE]);

    let events = read_il_events(&dir);
    let failure = events.iter()
        .find(|e| e["event_type"] == "LinkFailedItemTypeUnrecognized")
        .unwrap();
    let p = &failure["payload"];

    assert_eq!(p["failure_reason"].as_str().unwrap(), "item_type_unrecognized");
    assert_eq!(p["item_id"].as_str().unwrap(),    ITEM_MILESTONE);
    assert_eq!(p["item_type"].as_str().unwrap(),  "AlienType");
    assert_eq!(p["role"].as_str().unwrap(),       "target");
}

#[test]
fn test_canonical_precedence_item_type_checked_before_relation_type() {
    // When both item type and relation type are unrecognized, item type is checked first.
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[
        (ITEM_TASK,      "AlienType", "Unknown source"),
        (ITEM_MILESTONE, "milestone", "Q3 release"),
    ]);

    run_binary(&dir, &["add", ITEM_TASK, "completely_unknown_relation", ITEM_MILESTONE]);

    let events = read_il_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"LinkFailedItemTypeUnrecognized"),
        "ItemTypeUnrecognized must be emitted (source checked before relation type)");
    assert!(!types.contains(&"LinkFailedRelationTypeUnrecognized"),
        "LinkFailedRelationTypeUnrecognized must NOT be reached in the same invocation");
}

// ── Remove a link ─────────────────────────────────────────────────────────────

#[test]
fn test_remove_link_emits_requested_then_unlinked() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);

    run_binary(&dir, &["add",    ITEM_TASK, "blocks", ITEM_MILESTONE]);
    run_binary(&dir, &["remove", ITEM_TASK, "blocks", ITEM_MILESTONE]);

    let events = read_il_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"LinkRemoveRequested"), "LinkRemoveRequested must be emitted");
    assert!(types.contains(&"ItemUnlinked"),        "ItemUnlinked must be emitted");
    assert!(!types.contains(&"LinkFailedLinkNotFound"), "must NOT emit LinkNotFound on valid remove");

    let req_pos = types.iter().position(|&t| t == "LinkRemoveRequested").unwrap();
    let unl_pos = types.iter().position(|&t| t == "ItemUnlinked").unwrap();
    assert!(req_pos < unl_pos, "LinkRemoveRequested must precede ItemUnlinked");
}

#[test]
fn test_remove_link_no_longer_visible_in_list() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);

    run_binary(&dir, &["add",    ITEM_TASK, "blocks", ITEM_MILESTONE]);
    run_binary(&dir, &["remove", ITEM_TASK, "blocks", ITEM_MILESTONE]);
    run_binary(&dir, &["list"]);

    let events = read_il_events(&dir);
    let returned = events.iter().find(|e| e["event_type"] == "LinkListReturned").unwrap();

    assert_eq!(returned["payload"]["link_count"].as_u64().unwrap(), 0,
        "link must not appear after removal");
}

#[test]
fn test_vocabulary_evolution_never_prevents_removal() {
    // Even items with unrecognized entity types can have their links removed.
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[
        (ITEM_TASK,      "AlienType",   "Unknown source"),
        (ITEM_MILESTONE, "AnotherAlien","Unknown target"),
    ]);
    seed_item_linked(&dir, ITEM_TASK, "obsolete_link_type", ITEM_MILESTONE);

    run_binary(&dir, &["remove", ITEM_TASK, "obsolete_link_type", ITEM_MILESTONE]);

    let events = read_il_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"ItemUnlinked"),
        "ItemUnlinked must be emitted — neither unrecognized entity type nor \
         unrecognized relation type prevents removal");
}

// ── List all links ────────────────────────────────────────────────────────────

#[test]
fn test_list_all_emits_requested_then_returned() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);
    run_binary(&dir, &["add", ITEM_TASK, "blocks",  ITEM_MILESTONE]);
    run_binary(&dir, &["add", ITEM_RISK, "affects", ITEM_MILESTONE]);

    run_binary(&dir, &["list"]);

    let events = read_il_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"LinkListRequested"), "LinkListRequested must be emitted");
    assert!(types.contains(&"LinkListReturned"),  "LinkListReturned must be emitted");
}

#[test]
fn test_list_all_link_count_matches_array_length() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);
    run_binary(&dir, &["add", ITEM_TASK, "blocks",  ITEM_MILESTONE]);
    run_binary(&dir, &["add", ITEM_RISK, "affects", ITEM_MILESTONE]);

    run_binary(&dir, &["list"]);

    let events = read_il_events(&dir);
    let returned = events.iter().find(|e| e["event_type"] == "LinkListReturned").unwrap();
    let p = &returned["payload"];

    let count = p["link_count"].as_u64().unwrap() as usize;
    let array_len = p["links"].as_array().unwrap().len();
    assert_eq!(count, array_len, "link_count must equal links array length");
    assert_eq!(count, 2, "both added links must appear");
}

#[test]
fn test_list_all_shows_only_forward_direction() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);
    run_binary(&dir, &["add", ITEM_TASK, "blocks", ITEM_MILESTONE]);

    run_binary(&dir, &["list"]);

    let events = read_il_events(&dir);
    let returned = events.iter().find(|e| e["event_type"] == "LinkListReturned").unwrap();
    let links = returned["payload"]["links"].as_array().unwrap();

    assert_eq!(links.len(), 1, "list all must show only forward links, not synthetic inverses");
    assert_eq!(links[0]["direction"].as_str().unwrap(), "outgoing");
    assert_eq!(links[0]["display_label"].as_str().unwrap(), "Blocks");
}

#[test]
fn test_list_requested_item_id_is_null_for_all_listing() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);
    run_binary(&dir, &["add", ITEM_TASK, "blocks", ITEM_MILESTONE]);
    run_binary(&dir, &["list"]);

    let events = read_il_events(&dir);
    let requested = events.iter().find(|e| e["event_type"] == "LinkListRequested").unwrap();

    assert!(requested["payload"]["item_id"].is_null(),
        "item_id must be null when listing all");
}

// ── List links for a specific item ────────────────────────────────────────────

#[test]
fn test_list_item_shows_outgoing_with_forward_label() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);
    run_binary(&dir, &["add", ITEM_TASK, "blocks", ITEM_MILESTONE]);

    run_binary(&dir, &["list", "--item", ITEM_TASK]);

    let events = read_il_events(&dir);
    let returned = events.iter().find(|e| e["event_type"] == "LinkListReturned").unwrap();
    let links = returned["payload"]["links"].as_array().unwrap();

    assert_eq!(links.len(), 1);
    assert_eq!(links[0]["direction"].as_str().unwrap(),     "outgoing");
    assert_eq!(links[0]["display_label"].as_str().unwrap(), "Blocks");
    assert_eq!(links[0]["source_id"].as_str().unwrap(),     ITEM_TASK);
    assert_eq!(links[0]["target_id"].as_str().unwrap(),     ITEM_MILESTONE);
}

#[test]
fn test_list_item_shows_incoming_with_inverse_label() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);
    run_binary(&dir, &["add", ITEM_TASK, "blocks", ITEM_MILESTONE]);

    run_binary(&dir, &["list", "--item", ITEM_MILESTONE]);

    let events = read_il_events(&dir);
    let returned = events.iter().find(|e| e["event_type"] == "LinkListReturned").unwrap();
    let links = returned["payload"]["links"].as_array().unwrap();

    assert_eq!(links.len(), 1);
    assert_eq!(links[0]["direction"].as_str().unwrap(),     "incoming");
    assert_eq!(links[0]["display_label"].as_str().unwrap(), "Blocked By");
    assert_eq!(links[0]["source_id"].as_str().unwrap(),     ITEM_TASK);
    assert_eq!(links[0]["target_id"].as_str().unwrap(),     ITEM_MILESTONE);
}

#[test]
fn test_list_item_shows_only_links_involving_that_item() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);
    run_binary(&dir, &["add", ITEM_TASK, "blocks",  ITEM_MILESTONE]);
    run_binary(&dir, &["add", ITEM_RISK, "affects", ITEM_MILESTONE]);

    run_binary(&dir, &["list", "--item", ITEM_RISK]);

    let events = read_il_events(&dir);
    let returned = events.iter().find(|e| e["event_type"] == "LinkListReturned").unwrap();
    let links = returned["payload"]["links"].as_array().unwrap();

    assert_eq!(links.len(), 1);
    assert_eq!(links[0]["link_type"].as_str().unwrap(), "affects");
    assert!(links.iter().all(|l|
        l["source_id"].as_str() == Some(ITEM_RISK)
        || l["target_id"].as_str() == Some(ITEM_RISK)
    ), "all returned links must involve the queried item");
}

#[test]
fn test_list_item_shows_both_outgoing_and_incoming() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);
    run_binary(&dir, &["add", ITEM_TASK, "blocks",  ITEM_MILESTONE]);
    run_binary(&dir, &["add", ITEM_RISK, "affects", ITEM_MILESTONE]);

    run_binary(&dir, &["list", "--item", ITEM_MILESTONE]);

    let events = read_il_events(&dir);
    let returned = events.iter().find(|e| e["event_type"] == "LinkListReturned").unwrap();
    let links = returned["payload"]["links"].as_array().unwrap();

    assert_eq!(links.len(), 2, "milestone is target of 2 links — both must appear");
    assert!(links.iter().all(|l| l["direction"].as_str() == Some("incoming")));
}

#[test]
fn test_list_item_with_no_links_returns_empty_not_failure() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);

    run_binary(&dir, &["list", "--item", ITEM_TASK]);

    let events = read_il_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"LinkListReturned"), "LinkListReturned must be emitted");
    assert!(!types.iter().any(|t| t.starts_with("LinkFailed")),
        "no failure must be emitted when item exists but has no links");

    let returned = events.iter().find(|e| e["event_type"] == "LinkListReturned").unwrap();
    assert_eq!(returned["payload"]["link_count"].as_u64().unwrap(), 0);
    assert!(returned["payload"]["links"].as_array().unwrap().is_empty());
}

// ── Failure: ItemNotFound ─────────────────────────────────────────────────────

#[test]
fn test_item_not_found_source_emits_failure_on_add() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);

    run_binary(&dir, &["add", "00000000-0000-0000-0000-nonexistent1", "blocks", ITEM_MILESTONE]);

    let events = read_il_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"LinkAddRequested"),       "LinkAddRequested must be emitted");
    assert!(types.contains(&"LinkFailedItemNotFound"), "LinkFailedItemNotFound must be emitted");
    assert!(!types.contains(&"ItemLinked"),             "ItemLinked must NOT be emitted");
}

#[test]
fn test_item_not_found_target_emits_failure_on_add() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);

    run_binary(&dir, &["add", ITEM_TASK, "blocks", "00000000-0000-0000-0000-nonexistent2"]);

    let events = read_il_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"LinkFailedItemNotFound"), "LinkFailedItemNotFound must be emitted");
    assert!(!types.contains(&"ItemLinked"),             "ItemLinked must NOT be emitted");
}

#[test]
fn test_item_not_found_emits_failure_on_remove() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);

    run_binary(&dir, &["remove", "00000000-0000-0000-0000-nonexistent1", "blocks", ITEM_MILESTONE]);

    let events = read_il_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"LinkRemoveRequested"),    "LinkRemoveRequested must be emitted");
    assert!(types.contains(&"LinkFailedItemNotFound"), "LinkFailedItemNotFound must be emitted");
    assert!(!types.contains(&"ItemUnlinked"),           "ItemUnlinked must NOT be emitted");
}

#[test]
fn test_item_not_found_payload_identifies_missing_id() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);
    let missing = "00000000-0000-0000-0000-nonexistent1";

    run_binary(&dir, &["add", missing, "blocks", ITEM_MILESTONE]);

    let events = read_il_events(&dir);
    let failure = events.iter()
        .find(|e| e["event_type"] == "LinkFailedItemNotFound")
        .unwrap();
    let p = &failure["payload"];

    assert_eq!(p["failure_reason"].as_str().unwrap(), "item_not_found");
    assert_eq!(p["operation"].as_str().unwrap(), "add");
    assert_eq!(p["missing_item_id"].as_str().unwrap(), missing);
}

// ── Failure: DuplicateLink ────────────────────────────────────────────────────

#[test]
fn test_duplicate_link_emits_failure() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);

    run_binary(&dir, &["add", ITEM_TASK, "blocks", ITEM_MILESTONE]);
    run_binary(&dir, &["add", ITEM_TASK, "blocks", ITEM_MILESTONE]);

    let events = read_il_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"LinkFailedDuplicateLink"), "LinkFailedDuplicateLink must be emitted");
    assert_eq!(
        types.iter().filter(|&&t| t == "ItemLinked").count(), 1,
        "exactly one ItemLinked must be emitted despite two add attempts"
    );
}

#[test]
fn test_duplicate_link_payload() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);

    run_binary(&dir, &["add", ITEM_TASK, "blocks", ITEM_MILESTONE]);
    run_binary(&dir, &["add", ITEM_TASK, "blocks", ITEM_MILESTONE]);

    let events = read_il_events(&dir);
    let failure = events.iter()
        .find(|e| e["event_type"] == "LinkFailedDuplicateLink")
        .unwrap();
    let p = &failure["payload"];

    assert_eq!(p["failure_reason"].as_str().unwrap(), "duplicate_link");
    assert_eq!(p["source_id"].as_str().unwrap(), ITEM_TASK);
    assert_eq!(p["link_type"].as_str().unwrap(), "blocks");
    assert_eq!(p["target_id"].as_str().unwrap(), ITEM_MILESTONE);
}

// ── Failure: LinkNotFound ─────────────────────────────────────────────────────

#[test]
fn test_link_not_found_emits_failure_on_remove() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);

    run_binary(&dir, &["remove", ITEM_TASK, "blocks", ITEM_MILESTONE]);

    let events = read_il_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"LinkRemoveRequested"),   "LinkRemoveRequested must be emitted");
    assert!(types.contains(&"LinkFailedLinkNotFound"),"LinkFailedLinkNotFound must be emitted");
    assert!(!types.contains(&"ItemUnlinked"),          "ItemUnlinked must NOT be emitted");
}

#[test]
fn test_link_not_found_payload() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);

    run_binary(&dir, &["remove", ITEM_TASK, "blocks", ITEM_MILESTONE]);

    let events = read_il_events(&dir);
    let failure = events.iter()
        .find(|e| e["event_type"] == "LinkFailedLinkNotFound")
        .unwrap();
    let p = &failure["payload"];

    assert_eq!(p["failure_reason"].as_str().unwrap(), "link_not_found");
    assert_eq!(p["source_id"].as_str().unwrap(), ITEM_TASK);
    assert_eq!(p["link_type"].as_str().unwrap(), "blocks");
    assert_eq!(p["target_id"].as_str().unwrap(), ITEM_MILESTONE);
}

// ── Invariants ────────────────────────────────────────────────────────────────

#[test]
fn test_add_does_not_modify_item_events() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);

    let before = {
        let path = dir.path().join("events/runtime_events.jsonl");
        fs::read_to_string(&path).unwrap().lines().filter(|l| !l.is_empty()).count()
    };

    run_binary(&dir, &["add", ITEM_TASK, "blocks", ITEM_MILESTONE]);

    let path = dir.path().join("events/runtime_events.jsonl");
    let all: Vec<Value> = fs::read_to_string(&path).unwrap()
        .lines().filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let non_il = all.iter().filter(|e| e["source_module"].as_str() != Some("item_links")).count();

    assert_eq!(non_il, before, "add must not modify any existing events");
}

#[test]
fn test_directionality_preserved_related_to() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);

    // relatedTo is the schema-defined name (camelCase)
    run_binary(&dir, &["add", ITEM_TASK,      "relatedTo", ITEM_MILESTONE]);
    run_binary(&dir, &["add", ITEM_MILESTONE, "relatedTo", ITEM_TASK]);

    run_binary(&dir, &["list"]);

    let events = read_il_events(&dir);
    let returned = events.iter().find(|e| e["event_type"] == "LinkListReturned").unwrap();

    assert_eq!(returned["payload"]["link_count"].as_u64().unwrap(), 2,
        "A→B and B→A relatedTo links are distinct and both must appear");
}

#[test]
fn test_related_to_shows_same_label_on_both_sides() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);
    run_binary(&dir, &["add", ITEM_TASK, "relatedTo", ITEM_MILESTONE]);

    run_binary(&dir, &["list", "--item", ITEM_TASK]);
    run_binary(&dir, &["list", "--item", ITEM_MILESTONE]);

    let events = read_il_events(&dir);
    let returned_events: Vec<&Value> = events.iter()
        .filter(|e| e["event_type"] == "LinkListReturned")
        .collect();

    assert_eq!(returned_events.len(), 2);
    for ret in &returned_events {
        let links = ret["payload"]["links"].as_array().unwrap();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0]["display_label"].as_str().unwrap(), "Related To",
            "relatedTo must show 'Related To' label on both source and target side");
    }
}

// ── Telemetry ─────────────────────────────────────────────────────────────────

#[test]
fn test_all_events_have_required_base_fields() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);
    run_binary(&dir, &["add", ITEM_TASK, "blocks", ITEM_MILESTONE]);

    let events = read_il_events(&dir);
    assert!(!events.is_empty());

    for event in &events {
        let t = event["event_type"].as_str().unwrap_or("unknown");
        assert!(event["event_id"].as_str().is_some(),       "{}: event_id must be a string", t);
        assert!(event["event_type"].as_str().is_some(),     "{}: event_type must be a string", t);
        assert!(event["timestamp"].as_u64().is_some(),      "{}: timestamp must be a u64", t);
        assert!(event["correlation_id"].as_str().is_some(), "{}: correlation_id must be a string", t);
        assert!(event["source_module"].as_str().is_some(),  "{}: source_module must be a string", t);
        assert!(event["payload"].is_object(),               "{}: payload must be an object", t);
        assert_eq!(event["source_module"].as_str().unwrap(), "item_links",
            "{}: source_module must be 'item_links'", t);
    }
}

#[test]
fn test_correlation_id_consistent_within_invocation() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);
    run_binary(&dir, &["add", ITEM_TASK, "blocks", ITEM_MILESTONE]);

    let events = read_il_events(&dir);
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

    run_binary(&dir, &["add", ITEM_TASK, "blocks",  ITEM_MILESTONE]);
    run_binary(&dir, &["add", ITEM_RISK, "affects", ITEM_MILESTONE]);

    let events = read_il_events(&dir);
    let cids: Vec<&str> = events.iter()
        .filter(|e| e["event_type"] == "LinkAddRequested")
        .map(|e| e["correlation_id"].as_str().unwrap())
        .collect();

    assert_eq!(cids.len(), 2);
    assert_ne!(cids[0], cids[1], "different invocations must have different correlation_ids");
}

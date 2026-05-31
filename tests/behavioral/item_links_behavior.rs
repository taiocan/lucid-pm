//! Behavioral tests for item_links.
//!
//! Tests verify observable outcomes: events emitted, payload shapes, link
//! visibility after add/remove, failure modes, and invariants.
//! All assertions reference event names from events/item_links_schema.md exactly.

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

    assert_eq!(p["source_id"].as_str().unwrap(),   ITEM_TASK);
    assert_eq!(p["source_type"].as_str().unwrap(),  "task");
    assert_eq!(p["link_type"].as_str().unwrap(),    "blocks");
    assert_eq!(p["target_id"].as_str().unwrap(),    ITEM_MILESTONE);
    assert_eq!(p["target_type"].as_str().unwrap(),  "milestone");
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

// ── Happy Path 2: Remove a link ───────────────────────────────────────────────

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

// ── Happy Path 3: List all links ──────────────────────────────────────────────

#[test]
fn test_list_all_emits_requested_then_returned() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);
    run_binary(&dir, &["add", ITEM_TASK, "blocks",     ITEM_MILESTONE]);
    run_binary(&dir, &["add", ITEM_RISK, "affects",    ITEM_MILESTONE]);

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

// ── Happy Path 4: List links for a specific item ──────────────────────────────

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

    // Query ITEM_RISK — must only show the affects link, not the blocks link
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

    // ITEM_MILESTONE is the target of both links → 2 incoming entries
    run_binary(&dir, &["list", "--item", ITEM_MILESTONE]);

    let events = read_il_events(&dir);
    let returned = events.iter().find(|e| e["event_type"] == "LinkListReturned").unwrap();
    let links = returned["payload"]["links"].as_array().unwrap();

    assert_eq!(links.len(), 2, "milestone is target of 2 links — both must appear");
    assert!(links.iter().all(|l| l["direction"].as_str() == Some("incoming")));
}

// ── Happy Path 5: List item with no links ─────────────────────────────────────

#[test]
fn test_list_item_with_no_links_returns_empty_not_failure() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);
    // No links added

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

// ── Failure Path 1: ItemNotFound ──────────────────────────────────────────────

#[test]
fn test_item_not_found_source_emits_failure_on_add() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);

    run_binary(&dir, &["add", "00000000-0000-0000-0000-nonexistent1", "blocks", ITEM_MILESTONE]);

    let events = read_il_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"LinkAddRequested"),      "LinkAddRequested must be emitted");
    assert!(types.contains(&"LinkFailedItemNotFound"),"LinkFailedItemNotFound must be emitted");
    assert!(!types.contains(&"ItemLinked"),            "ItemLinked must NOT be emitted");
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

    assert!(types.contains(&"LinkRemoveRequested"),   "LinkRemoveRequested must be emitted");
    assert!(types.contains(&"LinkFailedItemNotFound"),"LinkFailedItemNotFound must be emitted");
    assert!(!types.contains(&"ItemUnlinked"),          "ItemUnlinked must NOT be emitted");
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
    assert_eq!(p["missing_item_id"].as_str().unwrap(), missing,
        "missing_item_id must identify which item was not found");
}

// ── Failure Path 2: InvalidLinkType ──────────────────────────────────────────

#[test]
fn test_invalid_link_type_unknown_emits_failure() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);

    run_binary(&dir, &["add", ITEM_TASK, "owns", ITEM_MILESTONE]);

    let events = read_il_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"LinkAddRequested"),         "LinkAddRequested must be emitted");
    assert!(types.contains(&"LinkFailedInvalidLinkType"),"LinkFailedInvalidLinkType must be emitted");
    assert!(!types.contains(&"ItemLinked"),               "ItemLinked must NOT be emitted");
}

#[test]
fn test_invalid_link_type_wrong_type_pair_emits_failure() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);

    // mitigated_by requires source=risk — task is not valid
    run_binary(&dir, &["add", ITEM_TASK, "mitigated_by", ITEM_MILESTONE]);

    let events = read_il_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"LinkFailedInvalidLinkType"),
        "LinkFailedInvalidLinkType must be emitted for a valid type used on wrong item type pair");
    assert!(!types.contains(&"ItemLinked"), "ItemLinked must NOT be emitted");
}

#[test]
fn test_invalid_link_type_payload_identifies_type_and_pair() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);

    run_binary(&dir, &["add", ITEM_TASK, "owns", ITEM_MILESTONE]);

    let events = read_il_events(&dir);
    let failure = events.iter()
        .find(|e| e["event_type"] == "LinkFailedInvalidLinkType")
        .unwrap();

    let p = &failure["payload"];
    assert_eq!(p["failure_reason"].as_str().unwrap(), "invalid_link_type");
    assert_eq!(p["link_type"].as_str().unwrap(),      "owns");
    assert_eq!(p["source_type"].as_str().unwrap(),    "task");
    assert_eq!(p["target_type"].as_str().unwrap(),    "milestone");
}

// ── Failure Path 3: DuplicateLink ────────────────────────────────────────────

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

// ── Failure Path 4: LinkNotFound ──────────────────────────────────────────────

#[test]
fn test_link_not_found_emits_failure_on_remove() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);
    // No link added

    run_binary(&dir, &["remove", ITEM_TASK, "blocks", ITEM_MILESTONE]);

    let events = read_il_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"LinkRemoveRequested"),  "LinkRemoveRequested must be emitted");
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

    // related_to is valid in both directions for any types
    run_binary(&dir, &["add", ITEM_TASK,      "related_to", ITEM_MILESTONE]);
    run_binary(&dir, &["add", ITEM_MILESTONE, "related_to", ITEM_TASK]);

    run_binary(&dir, &["list"]);

    let events = read_il_events(&dir);
    let returned = events.iter().find(|e| e["event_type"] == "LinkListReturned").unwrap();

    assert_eq!(returned["payload"]["link_count"].as_u64().unwrap(), 2,
        "A→B and B→A related_to links are distinct and both must appear");
}

#[test]
fn test_related_to_shows_same_label_on_both_sides() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);
    run_binary(&dir, &["add", ITEM_TASK, "related_to", ITEM_MILESTONE]);

    // Query source
    run_binary(&dir, &["list", "--item", ITEM_TASK]);
    // Query target
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
            "related_to must show 'Related To' label on both source and target side");
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

    run_binary(&dir, &["add", ITEM_TASK,      "blocks",  ITEM_MILESTONE]);
    run_binary(&dir, &["add", ITEM_RISK, "affects", ITEM_MILESTONE]);

    let events = read_il_events(&dir);
    let cids: Vec<&str> = events.iter()
        .filter(|e| e["event_type"] == "LinkAddRequested")
        .map(|e| e["correlation_id"].as_str().unwrap())
        .collect();

    assert_eq!(cids.len(), 2);
    assert_ne!(cids[0], cids[1], "different invocations must have different correlation_ids");
}

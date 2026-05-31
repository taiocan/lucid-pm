//! Behavioral tests for ontology_suggest.
//!
//! Tests verify observable outcomes: events emitted, payload shapes, delegated
//! events, failure modes, and confirm-time validation skips.
//! All assertions reference event names from events/ontology_suggest_schema.md exactly.
//!
//! The propose path requires a live LLM and is tested via failure modes only.
//! The confirm path is fully tested by seeding OntologyReviewProposed events directly.

use serde_json::{json, Value};
use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};
use tempfile::TempDir;

fn binary_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_ontology_suggest"))
}

fn setup_temp_dir() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("events")).unwrap();
    dir
}

fn seed_incorporated_items(dir: &TempDir, session_id: &str, items: &[(&str, &str, &str)]) {
    let items_json: Vec<Value> = items
        .iter()
        .map(|(id, typ, desc)| {
            json!({
                "item_id": id, "item_type": typ, "description": desc,
                "uncertain": false, "uncertainty_reason": null,
                "proposed_status": null, "proposed_priority": null,
            })
        })
        .collect();
    let accepted_ids: Vec<&str> = items.iter().map(|(id, _, _)| *id).collect();
    let path = dir.path().join("events/runtime_events.jsonl");
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .unwrap();
    writeln!(
        file,
        "{}",
        json!({
            "event_id": format!("seed-ext-{}", &session_id[..8]),
            "event_type": "ItemsExtracted", "timestamp": 1748000001000u64,
            "correlation_id": session_id, "source_module": "pm_structuring",
            "payload": { "items": items_json, "item_count": items.len(), "uncertain_count": 0 }
        })
    )
    .unwrap();
    writeln!(
        file,
        "{}",
        json!({
            "event_id": format!("seed-conf-{}", &session_id[..8]),
            "event_type": "ExtractionConfirmed", "timestamp": 1748000002000u64,
            "correlation_id": session_id, "source_module": "pm_structuring",
            "payload": { "accepted_item_ids": accepted_ids, "accepted_count": items.len() }
        })
    )
    .unwrap();
    writeln!(
        file,
        "{}",
        json!({
            "event_id": format!("seed-inc-{}", &session_id[..8]),
            "event_type": "ItemsIncorporated", "timestamp": 1748000003000u64,
            "correlation_id": "00000000-0000-0000-0000-000000000001",
            "source_module": "project_state",
            "payload": {
                "session_id": session_id,
                "incorporated_count": items.len(),
                "total_record_size": items.len()
            }
        })
    )
    .unwrap();
}

fn seed_review_proposed(dir: &TempDir, review_id: &str, proposals: Vec<Value>) {
    let path = dir.path().join("events/runtime_events.jsonl");
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .unwrap();
    writeln!(
        file,
        "{}",
        json!({
            "event_id": format!("seed-rev-{}", review_id.replace('-', "").chars().take(8).collect::<String>()),
            "event_type": "OntologyReviewProposed",
            "timestamp": 1748100000000u64,
            "correlation_id": format!("seed-corr-{}", &review_id[..8]),
            "source_module": "ontology_suggest",
            "payload": {
                "review_id": review_id,
                "proposal_count": proposals.len(),
                "proposals": proposals,
            }
        })
    )
    .unwrap();
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

fn run_binary_no_llm_key(dir: &TempDir, args: &[&str]) -> std::process::Output {
    Command::new(binary_path())
        .current_dir(dir.path())
        .args(args)
        .env_remove("GEMINI_API_KEY_PMCLI")
        .env_remove("GEMINI_API_KEY")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("Failed to run binary")
}

fn read_all_events(dir: &TempDir) -> Vec<Value> {
    let path = dir.path().join("events/runtime_events.jsonl");
    if !path.exists() {
        return vec![];
    }
    fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .collect()
}

fn read_os_events(dir: &TempDir) -> Vec<Value> {
    read_all_events(dir)
        .into_iter()
        .filter(|e| e["source_module"].as_str() == Some("ontology_suggest"))
        .collect()
}

const SESSION_A: &str = "aabb0000-0000-0000-0000-000000000001";
const REVIEW_ID_1: &str = "aarev001-0000-0000-0000-000000000001";
const REVIEW_ID_2: &str = "aarev002-0000-0000-0000-000000000002";
const ITEM_TASK: &str = "os000000-0000-0000-0000-000000000001";
const ITEM_MILESTONE: &str = "os000000-0000-0000-0000-000000000002";
const ITEM_RISK: &str = "os000000-0000-0000-0000-000000000003";
const ITEM_HOLDER: &str = "os000000-0000-0000-0000-000000000004";
const ITEM_ISSUE: &str = "os000000-0000-0000-0000-000000000005";

fn seed_full_record(dir: &TempDir) {
    seed_incorporated_items(
        dir,
        SESSION_A,
        &[
            (ITEM_TASK, "task", "Fix critical bug"),
            (ITEM_MILESTONE, "milestone", "Q3 release"),
            (ITEM_RISK, "risk", "Vendor dependency risk"),
            (ITEM_HOLDER, "stakeholder", "Engineering lead"),
            (ITEM_ISSUE, "issue", "Login page slow"),
        ],
    );
}

fn link_proposal(pid: &str, src: &str, lt: &str, tgt: &str) -> Value {
    json!({
        "proposal_id": pid,
        "type": "link",
        "source_id": src,
        "source_type": "task",
        "link_type": lt,
        "target_id": tgt,
        "target_type": "milestone",
        "rationale": "test rationale"
    })
}

fn status_proposal(pid: &str, item_id: &str, proposed: &str) -> Value {
    json!({
        "proposal_id": pid,
        "type": "status",
        "item_id": item_id,
        "current_status": null,
        "proposed_status": proposed,
        "rationale": "test rationale"
    })
}

fn priority_proposal(pid: &str, item_id: &str, proposed: &str) -> Value {
    json!({
        "proposal_id": pid,
        "type": "priority",
        "item_id": item_id,
        "current_priority": null,
        "proposed_priority": proposed,
        "rationale": "test rationale"
    })
}

// ── Failure Path 1: EmptyProjectRecord ───────────────────────────────────────

#[test]
fn test_empty_record_emits_failed_empty_record() {
    let dir = setup_temp_dir();
    // No items seeded

    run_binary_no_llm_key(&dir, &["propose"]);

    let events = read_os_events(&dir);
    let types: Vec<&str> = events
        .iter()
        .map(|e| e["event_type"].as_str().unwrap())
        .collect();

    assert!(
        types.contains(&"OntologyReviewFailedEmptyRecord"),
        "OntologyReviewFailedEmptyRecord must be emitted"
    );
    assert!(
        !types.contains(&"OntologyReviewRequested"),
        "OntologyReviewRequested must NOT be emitted when record is empty"
    );
    assert!(
        !types.contains(&"OntologyReviewProposed"),
        "OntologyReviewProposed must NOT be emitted"
    );
}

#[test]
fn test_empty_record_failure_payload() {
    let dir = setup_temp_dir();

    run_binary_no_llm_key(&dir, &["propose"]);

    let events = read_os_events(&dir);
    let failure = events
        .iter()
        .find(|e| e["event_type"] == "OntologyReviewFailedEmptyRecord")
        .unwrap();

    assert_eq!(
        failure["payload"]["failure_reason"].as_str().unwrap(),
        "empty_project_record"
    );
}

// ── Failure Path 2: LLMUnavailable ───────────────────────────────────────────

#[test]
fn test_llm_unavailable_emits_review_requested_then_failed() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);

    run_binary_no_llm_key(&dir, &["propose"]);

    let events = read_os_events(&dir);
    let types: Vec<&str> = events
        .iter()
        .map(|e| e["event_type"].as_str().unwrap())
        .collect();

    assert!(
        types.contains(&"OntologyReviewRequested"),
        "OntologyReviewRequested must be emitted before LLM failure"
    );
    assert!(
        types.contains(&"OntologyReviewFailedLLMUnavailable"),
        "OntologyReviewFailedLLMUnavailable must be emitted"
    );
    assert!(
        !types.contains(&"OntologyReviewProposed"),
        "OntologyReviewProposed must NOT be emitted when LLM fails"
    );

    let req_pos = types
        .iter()
        .position(|&t| t == "OntologyReviewRequested")
        .unwrap();
    let fail_pos = types
        .iter()
        .position(|&t| t == "OntologyReviewFailedLLMUnavailable")
        .unwrap();
    assert!(
        req_pos < fail_pos,
        "OntologyReviewRequested must precede OntologyReviewFailedLLMUnavailable"
    );
}

#[test]
fn test_llm_unavailable_failure_payload() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);

    run_binary_no_llm_key(&dir, &["propose"]);

    let events = read_os_events(&dir);
    let failure = events
        .iter()
        .find(|e| e["event_type"] == "OntologyReviewFailedLLMUnavailable")
        .unwrap();

    let p = &failure["payload"];
    assert_eq!(p["failure_reason"].as_str().unwrap(), "llm_unavailable");
    assert!(
        p["error_detail"].as_str().is_some(),
        "error_detail must be a non-null string"
    );
    assert!(
        !p["error_detail"].as_str().unwrap().is_empty(),
        "error_detail must be non-empty"
    );
}

#[test]
fn test_review_requested_payload_has_item_count() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);

    run_binary_no_llm_key(&dir, &["propose"]);

    let events = read_os_events(&dir);
    let requested = events
        .iter()
        .find(|e| e["event_type"] == "OntologyReviewRequested")
        .unwrap();

    let item_count = requested["payload"]["item_count"].as_u64().unwrap();
    assert_eq!(item_count, 5, "item_count must equal the 5 seeded items");
}

// ── Failure Path 3: ReviewNotFound ────────────────────────────────────────────

#[test]
fn test_review_not_found_emits_correct_failure() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);

    run_binary(
        &dir,
        &["confirm", "nonexistent-review-id", "--accept", "p-001"],
    );

    let events = read_os_events(&dir);
    let types: Vec<&str> = events
        .iter()
        .map(|e| e["event_type"].as_str().unwrap())
        .collect();

    assert!(
        types.contains(&"OntologyConfirmFailedReviewNotFound"),
        "OntologyConfirmFailedReviewNotFound must be emitted"
    );
    assert!(
        !types.contains(&"OntologyReviewConfirmed"),
        "OntologyReviewConfirmed must NOT be emitted when review not found"
    );
}

#[test]
fn test_review_not_found_payload() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);
    let missing_id = "00000000-dead-beef-0000-000000000000";

    run_binary(&dir, &["confirm", missing_id, "--accept", "p-001"]);

    let events = read_os_events(&dir);
    let failure = events
        .iter()
        .find(|e| e["event_type"] == "OntologyConfirmFailedReviewNotFound")
        .unwrap();

    let p = &failure["payload"];
    assert_eq!(p["failure_reason"].as_str().unwrap(), "review_not_found");
    assert_eq!(p["review_id"].as_str().unwrap(), missing_id);
}

// ── Happy Path 2: Link proposal accepted ─────────────────────────────────────

#[test]
fn test_accept_link_proposal_emits_confirm_requested_and_item_linked() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);
    seed_review_proposed(
        &dir,
        REVIEW_ID_1,
        vec![link_proposal("p-001", ITEM_TASK, "blocks", ITEM_MILESTONE)],
    );

    run_binary(&dir, &["confirm", REVIEW_ID_1, "--accept", "p-001"]);

    let os_events = read_os_events(&dir);
    let all_events = read_all_events(&dir);
    let os_types: Vec<&str> = os_events
        .iter()
        .map(|e| e["event_type"].as_str().unwrap())
        .collect();

    assert!(
        os_types.contains(&"OntologyConfirmRequested"),
        "OntologyConfirmRequested must be emitted"
    );
    assert!(
        os_types.contains(&"OntologyReviewConfirmed"),
        "OntologyReviewConfirmed must be emitted"
    );

    // Delegated event: ItemLinked with source_module=item_links
    let item_linked = all_events
        .iter()
        .find(|e| {
            e["event_type"] == "ItemLinked" && e["source_module"].as_str() == Some("item_links")
        })
        .expect("ItemLinked with source_module=item_links must be emitted");

    let p = &item_linked["payload"];
    assert_eq!(p["source_id"].as_str().unwrap(), ITEM_TASK);
    assert_eq!(p["link_type"].as_str().unwrap(), "blocks");
    assert_eq!(p["target_id"].as_str().unwrap(), ITEM_MILESTONE);
    assert_eq!(p["source_type"].as_str().unwrap(), "task");
    assert_eq!(p["target_type"].as_str().unwrap(), "milestone");
}

#[test]
fn test_accept_link_proposal_confirmed_counts() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);
    seed_review_proposed(
        &dir,
        REVIEW_ID_1,
        vec![link_proposal("p-001", ITEM_TASK, "blocks", ITEM_MILESTONE)],
    );

    run_binary(&dir, &["confirm", REVIEW_ID_1, "--accept", "p-001"]);

    let events = read_os_events(&dir);
    let confirmed = events
        .iter()
        .find(|e| e["event_type"] == "OntologyReviewConfirmed")
        .unwrap();

    let p = &confirmed["payload"];
    assert_eq!(p["accepted_count"].as_u64().unwrap(), 1);
    assert_eq!(p["rejected_count"].as_u64().unwrap(), 0);
    assert_eq!(p["skipped_count"].as_u64().unwrap(), 0);
    assert!(
        p["accepted_ids"].as_array().unwrap().contains(&json!("p-001")),
        "accepted_ids must contain p-001"
    );
    assert_eq!(p["review_id"].as_str().unwrap(), REVIEW_ID_1);
}

// ── Happy Path 3: Status proposal accepted ───────────────────────────────────

#[test]
fn test_accept_status_proposal_emits_item_status_updated() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);
    seed_review_proposed(
        &dir,
        REVIEW_ID_1,
        vec![status_proposal("p-001", ITEM_TASK, "doing")],
    );

    run_binary(&dir, &["confirm", REVIEW_ID_1, "--accept", "p-001"]);

    let all_events = read_all_events(&dir);
    let updated = all_events
        .iter()
        .find(|e| {
            e["event_type"] == "ItemStatusUpdated"
                && e["source_module"].as_str() == Some("item_status")
        })
        .expect("ItemStatusUpdated with source_module=item_status must be emitted");

    let p = &updated["payload"];
    assert_eq!(p["item_id"].as_str().unwrap(), ITEM_TASK);
    assert_eq!(p["new_status"].as_str().unwrap(), "doing");
    assert_eq!(p["item_type"].as_str().unwrap(), "task");
}

#[test]
fn test_accept_status_proposal_confirmed_count() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);
    seed_review_proposed(
        &dir,
        REVIEW_ID_1,
        vec![status_proposal("p-001", ITEM_TASK, "doing")],
    );

    run_binary(&dir, &["confirm", REVIEW_ID_1, "--accept", "p-001"]);

    let events = read_os_events(&dir);
    let confirmed = events
        .iter()
        .find(|e| e["event_type"] == "OntologyReviewConfirmed")
        .unwrap();

    assert_eq!(
        confirmed["payload"]["accepted_count"].as_u64().unwrap(),
        1
    );
    assert_eq!(
        confirmed["payload"]["skipped_count"].as_u64().unwrap(),
        0
    );
}

// ── Happy Path 4: Priority proposal accepted ──────────────────────────────────

#[test]
fn test_accept_priority_proposal_emits_item_priority_updated() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);
    seed_review_proposed(
        &dir,
        REVIEW_ID_1,
        vec![priority_proposal("p-001", ITEM_TASK, "high")],
    );

    run_binary(&dir, &["confirm", REVIEW_ID_1, "--accept", "p-001"]);

    let all_events = read_all_events(&dir);
    let updated = all_events
        .iter()
        .find(|e| {
            e["event_type"] == "ItemPriorityUpdated"
                && e["source_module"].as_str() == Some("item_status")
        })
        .expect("ItemPriorityUpdated with source_module=item_status must be emitted");

    let p = &updated["payload"];
    assert_eq!(p["item_id"].as_str().unwrap(), ITEM_TASK);
    assert_eq!(p["new_priority"].as_str().unwrap(), "high");
}

#[test]
fn test_accept_priority_proposal_confirmed_count() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);
    seed_review_proposed(
        &dir,
        REVIEW_ID_1,
        vec![priority_proposal("p-001", ITEM_TASK, "high")],
    );

    run_binary(&dir, &["confirm", REVIEW_ID_1, "--accept", "p-001"]);

    let events = read_os_events(&dir);
    let confirmed = events
        .iter()
        .find(|e| e["event_type"] == "OntologyReviewConfirmed")
        .unwrap();

    assert_eq!(
        confirmed["payload"]["accepted_count"].as_u64().unwrap(),
        1
    );
    assert_eq!(
        confirmed["payload"]["skipped_count"].as_u64().unwrap(),
        0
    );
}

// ── Happy Path 5: All proposals rejected ─────────────────────────────────────

#[test]
fn test_all_rejected_emits_confirmed_with_zero_applied() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);
    seed_review_proposed(
        &dir,
        REVIEW_ID_1,
        vec![
            link_proposal("p-001", ITEM_TASK, "blocks", ITEM_MILESTONE),
            status_proposal("p-002", ITEM_TASK, "doing"),
        ],
    );

    run_binary(
        &dir,
        &["confirm", REVIEW_ID_1, "--reject", "p-001", "--reject", "p-002"],
    );

    let all_events = read_all_events(&dir);
    let os_events: Vec<&Value> = all_events
        .iter()
        .filter(|e| e["source_module"].as_str() == Some("ontology_suggest"))
        .collect();

    // No delegated behavioral events
    assert!(
        !all_events.iter().any(|e| e["source_module"].as_str() == Some("item_links")),
        "no ItemLinked must be emitted when all rejected"
    );
    assert!(
        !all_events.iter().any(|e| {
            e["event_type"] == "ItemStatusUpdated"
                && e["source_module"].as_str() == Some("item_status")
        }),
        "no ItemStatusUpdated must be emitted when all rejected"
    );

    let confirmed = os_events
        .iter()
        .find(|e| e["event_type"] == "OntologyReviewConfirmed")
        .unwrap();

    let p = &confirmed["payload"];
    assert_eq!(p["accepted_count"].as_u64().unwrap(), 0);
    assert_eq!(p["rejected_count"].as_u64().unwrap(), 2);
    assert_eq!(p["skipped_count"].as_u64().unwrap(), 0);
    assert!(p["rejected_ids"]
        .as_array()
        .unwrap()
        .contains(&json!("p-001")));
    assert!(p["rejected_ids"]
        .as_array()
        .unwrap()
        .contains(&json!("p-002")));
}

// ── Happy Path 6: Partial acceptance ─────────────────────────────────────────

#[test]
fn test_partial_acceptance_applies_only_accepted() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);
    seed_review_proposed(
        &dir,
        REVIEW_ID_1,
        vec![
            link_proposal("p-001", ITEM_TASK, "blocks", ITEM_MILESTONE),
            status_proposal("p-002", ITEM_TASK, "doing"),
        ],
    );

    run_binary(
        &dir,
        &["confirm", REVIEW_ID_1, "--accept", "p-001", "--reject", "p-002"],
    );

    let all_events = read_all_events(&dir);

    // p-001 (link) applied → ItemLinked
    assert!(
        all_events.iter().any(|e| e["event_type"] == "ItemLinked"
            && e["source_module"].as_str() == Some("item_links")),
        "ItemLinked must be emitted for accepted link proposal"
    );

    // p-002 (status) rejected → no ItemStatusUpdated
    assert!(
        !all_events.iter().any(|e| e["event_type"] == "ItemStatusUpdated"
            && e["source_module"].as_str() == Some("item_status")),
        "ItemStatusUpdated must NOT be emitted for rejected status proposal"
    );

    let os_events: Vec<&Value> = all_events
        .iter()
        .filter(|e| e["source_module"].as_str() == Some("ontology_suggest"))
        .collect();

    let confirmed = os_events
        .iter()
        .find(|e| e["event_type"] == "OntologyReviewConfirmed")
        .unwrap();

    let p = &confirmed["payload"];
    assert_eq!(p["accepted_count"].as_u64().unwrap(), 1);
    assert_eq!(p["rejected_count"].as_u64().unwrap(), 1);
    assert_eq!(p["skipped_count"].as_u64().unwrap(), 0);
}

// ── Happy Path 7: Zero proposals ─────────────────────────────────────────────

#[test]
fn test_zero_proposals_confirm_emits_confirmed_with_all_zeros() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);
    seed_review_proposed(&dir, REVIEW_ID_1, vec![]);

    run_binary(&dir, &["confirm", REVIEW_ID_1]);

    let events = read_os_events(&dir);
    let confirmed = events
        .iter()
        .find(|e| e["event_type"] == "OntologyReviewConfirmed")
        .unwrap();

    let p = &confirmed["payload"];
    assert_eq!(p["accepted_count"].as_u64().unwrap(), 0);
    assert_eq!(p["rejected_count"].as_u64().unwrap(), 0);
    assert_eq!(p["skipped_count"].as_u64().unwrap(), 0);
    assert!(p["accepted_ids"].as_array().unwrap().is_empty());
    assert!(p["rejected_ids"].as_array().unwrap().is_empty());
    assert!(p["skipped_ids"].as_array().unwrap().is_empty());
}

// ── Happy Path 8: Confirm old review ─────────────────────────────────────────

#[test]
fn test_confirm_old_review_applies_only_its_proposals() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);

    // review_1: task blocks milestone
    seed_review_proposed(
        &dir,
        REVIEW_ID_1,
        vec![link_proposal("p-001", ITEM_TASK, "blocks", ITEM_MILESTONE)],
    );
    // review_2: risk affects milestone (more recent)
    seed_review_proposed(
        &dir,
        REVIEW_ID_2,
        vec![link_proposal("p-001", ITEM_RISK, "affects", ITEM_MILESTONE)],
    );

    // Confirm only the first review
    run_binary(&dir, &["confirm", REVIEW_ID_1, "--accept", "p-001"]);

    let all_events = read_all_events(&dir);
    let linked_events: Vec<&Value> = all_events
        .iter()
        .filter(|e| {
            e["event_type"] == "ItemLinked" && e["source_module"].as_str() == Some("item_links")
        })
        .collect();

    assert_eq!(linked_events.len(), 1, "exactly one ItemLinked must be emitted");
    assert_eq!(
        linked_events[0]["payload"]["source_id"].as_str().unwrap(),
        ITEM_TASK,
        "only the proposal from review_1 (task blocks milestone) must be applied"
    );
    assert_eq!(
        linked_events[0]["payload"]["link_type"].as_str().unwrap(),
        "blocks"
    );

    let os_events = read_os_events(&dir);
    let confirmed = os_events
        .iter()
        .find(|e| e["event_type"] == "OntologyReviewConfirmed")
        .unwrap();
    assert_eq!(
        confirmed["payload"]["review_id"].as_str().unwrap(),
        REVIEW_ID_1
    );
}

// ── Confirm-time validation: skip invalid proposals ───────────────────────────

#[test]
fn test_skips_link_proposal_when_link_already_exists() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);

    // Seed an existing ItemLinked event (task blocks milestone already in place)
    let path = dir.path().join("events/runtime_events.jsonl");
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .unwrap();
    writeln!(
        file,
        "{}",
        json!({
            "event_id": "existing-link",
            "event_type": "ItemLinked",
            "timestamp": 1748050000000u64,
            "correlation_id": "pre-existing",
            "source_module": "item_links",
            "payload": {
                "source_id": ITEM_TASK,
                "source_type": "task",
                "link_type": "blocks",
                "target_id": ITEM_MILESTONE,
                "target_type": "milestone"
            }
        })
    )
    .unwrap();

    // Propose the same link again
    seed_review_proposed(
        &dir,
        REVIEW_ID_1,
        vec![link_proposal("p-001", ITEM_TASK, "blocks", ITEM_MILESTONE)],
    );

    run_binary(&dir, &["confirm", REVIEW_ID_1, "--accept", "p-001"]);

    let os_events = read_os_events(&dir);
    let confirmed = os_events
        .iter()
        .find(|e| e["event_type"] == "OntologyReviewConfirmed")
        .unwrap();

    let p = &confirmed["payload"];
    assert_eq!(p["accepted_count"].as_u64().unwrap(), 0);
    assert_eq!(p["skipped_count"].as_u64().unwrap(), 1);
    assert!(p["skipped_ids"].as_array().unwrap().contains(&json!("p-001")));

    // No new ItemLinked emitted (only the pre-existing one)
    let all_events = read_all_events(&dir);
    let link_count = all_events
        .iter()
        .filter(|e| e["event_type"] == "ItemLinked")
        .count();
    assert_eq!(link_count, 1, "no new ItemLinked must be emitted for a skipped proposal");
}

// ── OntologyConfirmRequested: accepted_ids and rejected_ids recorded ───────────

#[test]
fn test_confirm_requested_records_accept_and_reject_ids() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);
    seed_review_proposed(
        &dir,
        REVIEW_ID_1,
        vec![
            link_proposal("p-001", ITEM_TASK, "blocks", ITEM_MILESTONE),
            status_proposal("p-002", ITEM_TASK, "doing"),
        ],
    );

    run_binary(
        &dir,
        &["confirm", REVIEW_ID_1, "--accept", "p-001", "--reject", "p-002"],
    );

    let events = read_os_events(&dir);
    let requested = events
        .iter()
        .find(|e| e["event_type"] == "OntologyConfirmRequested")
        .unwrap();

    let p = &requested["payload"];
    assert_eq!(p["review_id"].as_str().unwrap(), REVIEW_ID_1);
    assert!(p["accepted_ids"].as_array().unwrap().contains(&json!("p-001")));
    assert!(p["rejected_ids"].as_array().unwrap().contains(&json!("p-002")));
}

// ── Delegated events have correct source_module ───────────────────────────────

#[test]
fn test_item_linked_from_confirmed_proposal_has_item_links_source_module() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);
    seed_review_proposed(
        &dir,
        REVIEW_ID_1,
        vec![link_proposal("p-001", ITEM_TASK, "blocks", ITEM_MILESTONE)],
    );

    run_binary(&dir, &["confirm", REVIEW_ID_1, "--accept", "p-001"]);

    let all_events = read_all_events(&dir);
    let linked = all_events
        .iter()
        .find(|e| e["event_type"] == "ItemLinked")
        .unwrap();

    assert_eq!(
        linked["source_module"].as_str().unwrap(),
        "item_links",
        "ItemLinked emitted by ontology_suggest confirm must have source_module=item_links"
    );
}

#[test]
fn test_item_status_updated_from_confirmed_proposal_has_item_status_source_module() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);
    seed_review_proposed(
        &dir,
        REVIEW_ID_1,
        vec![status_proposal("p-001", ITEM_TASK, "doing")],
    );

    run_binary(&dir, &["confirm", REVIEW_ID_1, "--accept", "p-001"]);

    let all_events = read_all_events(&dir);
    let updated = all_events
        .iter()
        .find(|e| e["event_type"] == "ItemStatusUpdated")
        .unwrap();

    assert_eq!(
        updated["source_module"].as_str().unwrap(),
        "item_status",
        "ItemStatusUpdated emitted by ontology_suggest confirm must have source_module=item_status"
    );
}

#[test]
fn test_item_priority_updated_from_confirmed_proposal_has_item_status_source_module() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);
    seed_review_proposed(
        &dir,
        REVIEW_ID_1,
        vec![priority_proposal("p-001", ITEM_TASK, "high")],
    );

    run_binary(&dir, &["confirm", REVIEW_ID_1, "--accept", "p-001"]);

    let all_events = read_all_events(&dir);
    let updated = all_events
        .iter()
        .find(|e| e["event_type"] == "ItemPriorityUpdated")
        .unwrap();

    assert_eq!(
        updated["source_module"].as_str().unwrap(),
        "item_status",
        "ItemPriorityUpdated emitted by ontology_suggest confirm must have source_module=item_status"
    );
}

// ── Telemetry ─────────────────────────────────────────────────────────────────

#[test]
fn test_all_os_events_have_required_base_fields() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);
    seed_review_proposed(
        &dir,
        REVIEW_ID_1,
        vec![link_proposal("p-001", ITEM_TASK, "blocks", ITEM_MILESTONE)],
    );

    run_binary(&dir, &["confirm", REVIEW_ID_1, "--accept", "p-001"]);

    let events = read_os_events(&dir);
    assert!(!events.is_empty());

    for event in &events {
        let t = event["event_type"].as_str().unwrap_or("unknown");
        assert!(
            event["event_id"].as_str().is_some(),
            "{}: event_id must be a string",
            t
        );
        assert!(
            event["event_type"].as_str().is_some(),
            "{}: event_type must be a string",
            t
        );
        assert!(
            event["timestamp"].as_u64().is_some(),
            "{}: timestamp must be a u64",
            t
        );
        assert!(
            event["correlation_id"].as_str().is_some(),
            "{}: correlation_id must be a string",
            t
        );
        assert!(
            event["source_module"].as_str().is_some(),
            "{}: source_module must be a string",
            t
        );
        assert!(
            event["payload"].is_object(),
            "{}: payload must be an object",
            t
        );
        assert_eq!(
            event["source_module"].as_str().unwrap(),
            "ontology_suggest",
            "{}: source_module must be ontology_suggest for own events",
            t
        );
    }
}

#[test]
fn test_confirm_events_share_correlation_id() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);
    seed_review_proposed(
        &dir,
        REVIEW_ID_1,
        vec![link_proposal("p-001", ITEM_TASK, "blocks", ITEM_MILESTONE)],
    );

    run_binary(&dir, &["confirm", REVIEW_ID_1, "--accept", "p-001"]);

    let all_events = read_all_events(&dir);
    // All events emitted by this invocation (OntologyConfirmRequested, ItemLinked,
    // OntologyReviewConfirmed) should share a correlation_id.
    let emitted: Vec<&Value> = all_events
        .iter()
        .filter(|e| {
            matches!(
                e["event_type"].as_str(),
                Some("OntologyConfirmRequested")
                    | Some("ItemLinked")
                    | Some("OntologyReviewConfirmed")
            )
        })
        .collect();

    assert!(emitted.len() >= 2);
    let cid = emitted[0]["correlation_id"].as_str().unwrap();
    for e in &emitted {
        assert_eq!(
            e["correlation_id"].as_str().unwrap(),
            cid,
            "all events from one confirm invocation must share correlation_id"
        );
    }
}

#[test]
fn test_accept_all_flag_accepts_all_proposals() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);
    seed_review_proposed(
        &dir,
        REVIEW_ID_1,
        vec![
            status_proposal("p-001", ITEM_TASK, "doing"),
            priority_proposal("p-002", ITEM_RISK, "high"),
        ],
    );

    run_binary(&dir, &["confirm", REVIEW_ID_1, "--accept-all"]);

    let os_events = read_os_events(&dir);
    let confirmed = os_events
        .iter()
        .find(|e| e["event_type"] == "OntologyReviewConfirmed")
        .unwrap();

    assert_eq!(
        confirmed["payload"]["accepted_count"].as_u64().unwrap(),
        2,
        "--accept-all must accept all proposals"
    );
    assert_eq!(
        confirmed["payload"]["skipped_count"].as_u64().unwrap(),
        0
    );
}

#[test]
fn test_accept_all_populates_confirm_requested_accepted_ids() {
    let dir = setup_temp_dir();
    seed_full_record(&dir);
    seed_review_proposed(
        &dir,
        REVIEW_ID_1,
        vec![
            status_proposal("p-001", ITEM_TASK, "doing"),
            priority_proposal("p-002", ITEM_RISK, "high"),
        ],
    );

    run_binary(&dir, &["confirm", REVIEW_ID_1, "--accept-all"]);

    let os_events = read_os_events(&dir);
    let requested = os_events
        .iter()
        .find(|e| e["event_type"] == "OntologyConfirmRequested")
        .unwrap();

    let accepted_ids = requested["payload"]["accepted_ids"].as_array().unwrap();
    assert_eq!(
        accepted_ids.len(),
        2,
        "OntologyConfirmRequested.accepted_ids must list all proposal IDs when --accept-all is used"
    );
    assert!(accepted_ids.contains(&serde_json::json!("p-001")));
    assert!(accepted_ids.contains(&serde_json::json!("p-002")));
}

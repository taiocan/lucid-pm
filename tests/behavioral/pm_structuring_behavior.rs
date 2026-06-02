//! Behavioral tests for pm_structuring.
//!
//! Tests verify observable outcomes: events emitted, event fields, ordering.
//! No internal logic is tested. All assertions reference event names from
//! events/pm_structuring_schema.md exactly.
//!
//! Tests marked with `if !gemini_key_available() { return; }` require
//! GEMINI_API_KEY_PMCLI or GEMINI_API_KEY to be set.

use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};
use serde_json::{json, Value};
use tempfile::TempDir;

fn binary_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_pm_structuring"))
}

fn setup_temp_dir() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("events")).unwrap();
    dir
}

fn run_binary(dir: &TempDir, stdin_bytes: &[u8]) -> std::process::Output {
    run_binary_with_args(dir, stdin_bytes, &[])
}

fn run_binary_with_args(dir: &TempDir, stdin_bytes: &[u8], args: &[&str]) -> std::process::Output {
    let mut child = Command::new(binary_path())
        .current_dir(dir.path())
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn binary");

    child.stdin.as_mut().unwrap().write_all(stdin_bytes).unwrap();
    child.wait_with_output().unwrap()
}

fn read_events(dir: &TempDir) -> Vec<Value> {
    let path = dir.path().join("events/runtime_events.jsonl");
    if !path.exists() {
        return vec![];
    }
    fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

fn gemini_key_available() -> bool {
    std::env::var("GEMINI_API_KEY_PMCLI").is_ok() || std::env::var("GEMINI_API_KEY").is_ok()
}

// ── Happy Path: Full Extraction with PM Confirmation (Refinement 1) ───────────

#[test]
fn test_confirmed_path_emits_extraction_confirmed() {
    if !gemini_key_available() { return; }
    let dir = setup_temp_dir();
    run_binary_with_args(
        &dir,
        b"Deploy the release by end of week. Sarah is the release manager. Risk: vendor delays.\n",
        &["--yes"],
    );

    let events = read_events(&dir);
    let types: Vec<&str> = events.iter()
        .map(|e| e["event_type"].as_str().unwrap())
        .collect();

    assert!(types.contains(&"TextSubmitted"),      "TextSubmitted must be emitted");
    assert!(types.contains(&"ItemsExtracted"),     "ItemsExtracted must be emitted");
    assert!(types.contains(&"ExtractionConfirmed"),"ExtractionConfirmed must be emitted with --yes");
    assert!(!types.contains(&"ExtractionRejected"),"ExtractionRejected must NOT be emitted on confirm");
}

#[test]
fn test_confirmed_event_order() {
    if !gemini_key_available() { return; }
    let dir = setup_temp_dir();
    run_binary_with_args(
        &dir,
        b"Deliver prototype by end of Q2. Alice is the project owner.\n",
        &["--yes"],
    );

    let events = read_events(&dir);
    let types: Vec<&str> = events.iter()
        .map(|e| e["event_type"].as_str().unwrap())
        .collect();

    let submitted  = types.iter().position(|&t| t == "TextSubmitted").unwrap();
    let extracted  = types.iter().position(|&t| t == "ItemsExtracted").unwrap();
    let confirmed  = types.iter().position(|&t| t == "ExtractionConfirmed").unwrap();

    assert!(submitted < extracted, "TextSubmitted must precede ItemsExtracted");
    assert!(extracted < confirmed, "ItemsExtracted must precede ExtractionConfirmed");
}

#[test]
fn test_confirmed_item_ids_match_extracted() {
    if !gemini_key_available() { return; }
    let dir = setup_temp_dir();
    run_binary_with_args(
        &dir,
        b"Set up CI pipeline by Friday. Bob is the lead developer.\n",
        &["--yes"],
    );

    let events = read_events(&dir);
    let extracted = events.iter().find(|e| e["event_type"] == "ItemsExtracted").unwrap();
    let confirmed  = events.iter().find(|e| e["event_type"] == "ExtractionConfirmed").unwrap();

    let extracted_ids: Vec<&str> = extracted["payload"]["items"].as_array().unwrap()
        .iter().map(|i| i["item_id"].as_str().unwrap()).collect();
    let accepted_ids: Vec<&str>  = confirmed["payload"]["accepted_item_ids"].as_array().unwrap()
        .iter().map(|i| i.as_str().unwrap()).collect();

    assert_eq!(extracted_ids, accepted_ids,
        "accepted_item_ids must match item_ids from ItemsExtracted");
}

// ── Happy Path: items extracted (rejection path verifies up to ItemsExtracted) ─

#[test]
fn test_happy_path_emits_items_extracted() {
    if !gemini_key_available() { return; }
    let dir = setup_temp_dir();
    run_binary(&dir, b"Deliver the prototype by end of Q2. Alice is the project owner. Risk: budget may be cut.\n");

    let events = read_events(&dir);
    let types: Vec<&str> = events.iter()
        .map(|e| e["event_type"].as_str().unwrap())
        .collect();

    assert!(types.contains(&"TextSubmitted"),  "TextSubmitted must be emitted");
    assert!(types.contains(&"ItemsExtracted"), "ItemsExtracted must be emitted for PM text");
}

#[test]
fn test_items_extracted_payload_shape() {
    if !gemini_key_available() { return; }
    let dir = setup_temp_dir();
    run_binary(&dir, b"Set up CI pipeline by Friday. Bob is the release manager.\n");

    let events = read_events(&dir);
    if let Some(extracted) = events.iter().find(|e| e["event_type"] == "ItemsExtracted") {
        let items        = extracted["payload"]["items"].as_array().expect("items must be an array");
        let item_count   = extracted["payload"]["item_count"].as_u64().expect("item_count must be present");
        let uncertain_count = extracted["payload"]["uncertain_count"].as_u64().expect("uncertain_count must be present");

        assert_eq!(items.len() as u64, item_count);
        let actual_uncertain = items.iter().filter(|i| i["uncertain"].as_bool().unwrap_or(false)).count() as u64;
        assert_eq!(actual_uncertain, uncertain_count);

        // R6: "unknown" is now a valid item_type (unrecognized vocabulary prediction)
        let valid_types = ["task", "milestone", "risk", "issue", "stakeholder", "unknown"];
        for item in items {
            assert!(item.get("item_id").is_some(),    "item must have item_id");
            assert!(item.get("item_type").is_some(),  "item must have item_type");
            assert!(item.get("description").is_some(),"item must have description");
            assert!(item.get("uncertain").is_some(),  "item must have uncertain");

            let item_type = item["item_type"].as_str().unwrap();
            assert!(valid_types.contains(&item_type),
                "item_type '{}' must be vocabulary-recognized or 'unknown'", item_type);

            if item["uncertain"].as_bool().unwrap_or(false) {
                assert!(!item["uncertainty_reason"].is_null(),
                    "uncertainty_reason must not be null when uncertain is true");
            }
        }
    }
}

// ── Happy Path Variant: Uncertainty (Refinement 3) ────────────────────────────

#[test]
fn test_uncertain_items_visible_before_confirmation() {
    if !gemini_key_available() { return; }
    let dir = setup_temp_dir();
    // Deliberately vague input designed to force uncertain item extraction
    run_binary_with_args(
        &dir,
        b"Someone should probably look at the thing with the deployment before it becomes more of a problem. Also, there might be some risk around the upcoming thing.\n",
        &["--yes"],
    );

    let events = read_events(&dir);
    // ItemsExtracted must appear BEFORE ExtractionConfirmed — uncertainty visible before confirmation
    if let Some(extracted) = events.iter().find(|e| e["event_type"] == "ItemsExtracted") {
        let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();
        let extracted_pos = types.iter().position(|&t| t == "ItemsExtracted").unwrap();
        if let Some(confirmed_pos) = types.iter().position(|&t| t == "ExtractionConfirmed") {
            assert!(extracted_pos < confirmed_pos,
                "ItemsExtracted (with uncertainty info) must precede ExtractionConfirmed");
        }

        let items = extracted["payload"]["items"].as_array().unwrap();
        for item in items {
            assert!(item.get("uncertain").is_some(), "Each item must have 'uncertain' field");
            if item["uncertain"].as_bool().unwrap_or(false) {
                assert!(!item["uncertainty_reason"].is_null(),
                    "uncertainty_reason must not be null when uncertain is true");
                assert!(!item["uncertainty_reason"].as_str().unwrap_or("").is_empty(),
                    "uncertainty_reason must not be empty");
            }
        }

        let actual_uncertain = items.iter()
            .filter(|i| i["uncertain"].as_bool().unwrap_or(false))
            .count() as u64;
        let stated_uncertain = extracted["payload"]["uncertain_count"].as_u64().unwrap();
        assert_eq!(actual_uncertain, stated_uncertain,
            "uncertain_count must match actual items with uncertain=true");
    }
}

// ── Happy Path Variant: Proposed Status and Priority (R1) ────────────────────

const VALID_STATUSES_BY_TYPE: &[(&str, &[&str])] = &[
    ("task",        &["todo", "doing", "done", "waiting", "cancelled"]),
    ("milestone",   &["pending", "achieved", "missed"]),
    ("risk",        &["open", "mitigated", "accepted", "closed"]),
    ("issue",       &["open", "in_progress", "resolved", "closed"]),
    ("stakeholder", &["active", "inactive"]),
];

const VALID_PRIORITIES: &[&str] = &["high", "medium", "low"];

fn valid_statuses_for(item_type: &str) -> &'static [&'static str] {
    VALID_STATUSES_BY_TYPE
        .iter()
        .find(|(t, _)| *t == item_type)
        .map(|(_, v)| *v)
        .unwrap_or(&[])
}

#[test]
fn test_items_extracted_items_have_proposed_fields() {
    if !gemini_key_available() { return; }
    let dir = setup_temp_dir();
    run_binary_with_args(
        &dir,
        b"Deploy the new release by end of week. Risk: vendor delays may block the build.\n",
        &["--yes"],
    );

    let events = read_events(&dir);
    if let Some(extracted) = events.iter().find(|e| e["event_type"] == "ItemsExtracted") {
        let items = extracted["payload"]["items"].as_array().unwrap();
        assert!(!items.is_empty(), "ItemsExtracted must contain items");
        for item in items {
            assert!(item.get("proposed_status").is_some(),
                "Each item must have a proposed_status field (may be null)");
            assert!(item.get("proposed_priority").is_some(),
                "Each item must have a proposed_priority field (may be null)");
        }
    }
}

#[test]
fn test_proposed_status_values_are_schema_valid() {
    if !gemini_key_available() { return; }
    let dir = setup_temp_dir();
    run_binary_with_args(
        &dir,
        b"Deploy the release by end of week. Bug: login page crashes on mobile.\n",
        &["--yes"],
    );

    let events = read_events(&dir);
    if let Some(extracted) = events.iter().find(|e| e["event_type"] == "ItemsExtracted") {
        let items = extracted["payload"]["items"].as_array().unwrap();
        for item in items {
            if let Some(status) = item["proposed_status"].as_str() {
                let item_type = item["item_type"].as_str().unwrap();
                let valid = valid_statuses_for(item_type);
                assert!(valid.contains(&status),
                    "proposed_status '{}' is not valid for item_type '{}'", status, item_type);
            }
            // null is always valid — no assertion needed for null case
        }
    }
}

#[test]
fn test_proposed_priority_values_are_schema_valid() {
    if !gemini_key_available() { return; }
    let dir = setup_temp_dir();
    run_binary_with_args(
        &dir,
        b"Critical issue: payment processing is down. Urgent fix needed immediately.\n",
        &["--yes"],
    );

    let events = read_events(&dir);
    if let Some(extracted) = events.iter().find(|e| e["event_type"] == "ItemsExtracted") {
        let items = extracted["payload"]["items"].as_array().unwrap();
        for item in items {
            if let Some(priority) = item["proposed_priority"].as_str() {
                assert!(VALID_PRIORITIES.contains(&priority),
                    "proposed_priority '{}' must be one of: high, medium, low", priority);
            }
        }
    }
}

#[test]
fn test_proposed_fields_present_on_rejection_path() {
    if !gemini_key_available() { return; }
    let dir = setup_temp_dir();
    // EOF stdin → ExtractionRejected; proposed fields must still be present in ItemsExtracted
    run_binary(
        &dir,
        b"Deploy the release by end of week. Risk: vendor delays.\n",
    );

    let events = read_events(&dir);
    if let Some(extracted) = events.iter().find(|e| e["event_type"] == "ItemsExtracted") {
        let items = extracted["payload"]["items"].as_array().unwrap();
        for item in items {
            assert!(item.get("proposed_status").is_some(),
                "proposed_status must be present even on the rejection path");
            assert!(item.get("proposed_priority").is_some(),
                "proposed_priority must be present even on the rejection path");
        }
    }
}

// ── Failure Path 1: EmptyInput ────────────────────────────────────────────────

#[test]
fn test_empty_input_emits_text_submitted_then_failure() {
    let dir = setup_temp_dir();
    run_binary(&dir, b"");

    let events = read_events(&dir);
    let types: Vec<&str> = events.iter()
        .map(|e| e["event_type"].as_str().unwrap())
        .collect();

    assert!(types.contains(&"TextSubmitted"),             "TextSubmitted must be emitted even for empty input");
    assert!(types.contains(&"ExtractionFailedEmptyInput"),"ExtractionFailedEmptyInput must be emitted");
    assert!(!types.contains(&"ItemsExtracted"),           "ItemsExtracted must NOT be emitted on empty input");
    assert!(!types.contains(&"ExtractionConfirmed"),      "ExtractionConfirmed must NOT be emitted on empty input");
}

#[test]
fn test_empty_input_failure_reason_is_empty_input() {
    let dir = setup_temp_dir();
    run_binary(&dir, b"");

    let events = read_events(&dir);
    let failure = events.iter()
        .find(|e| e["event_type"] == "ExtractionFailedEmptyInput")
        .expect("ExtractionFailedEmptyInput event not found");

    assert_eq!(failure["payload"]["failure_reason"].as_str().unwrap(), "empty_input");
}

#[test]
fn test_whitespace_only_input_treated_as_empty() {
    let dir = setup_temp_dir();
    run_binary(&dir, b"   \n\n   \n");

    let events = read_events(&dir);
    let types: Vec<&str> = events.iter()
        .map(|e| e["event_type"].as_str().unwrap())
        .collect();

    assert!(types.contains(&"ExtractionFailedEmptyInput"),
        "Whitespace-only input must trigger EmptyInput failure");
}

// ── Failure Path 2: NoExtractableContent ─────────────────────────────────────

#[test]
fn test_no_content_failure_reason_and_source_length() {
    if !gemini_key_available() { return; }
    let dir = setup_temp_dir();
    run_binary(&dir, b"The weather today is sunny with a light breeze. A great day for a walk.\n");

    let events = read_events(&dir);
    if let Some(failure) = events.iter().find(|e| e["event_type"] == "ExtractionFailedNoContent") {
        assert_eq!(failure["payload"]["failure_reason"].as_str().unwrap(), "no_extractable_content");
        assert!(failure["payload"]["source_text_length"].as_u64().unwrap() > 0);
    }
}

// ── Failure Path 3: PMRejectedExtraction ─────────────────────────────────────

#[test]
fn test_pm_rejection_emits_extraction_rejected() {
    if !gemini_key_available() { return; }
    let dir = setup_temp_dir();
    // EOF on stdin → decision = "" → rejection
    run_binary(&dir, b"Deploy the new release by end of week. Sarah is the release manager.\n");

    let events = read_events(&dir);
    let types: Vec<&str> = events.iter()
        .map(|e| e["event_type"].as_str().unwrap())
        .collect();

    assert!(types.contains(&"ExtractionRejected"),  "ExtractionRejected must be emitted on PM rejection");
    assert!(!types.contains(&"ExtractionConfirmed"),"ExtractionConfirmed must NOT be emitted on rejection");
}

#[test]
fn test_pm_rejection_items_extracted_event_precedes_rejected() {
    if !gemini_key_available() { return; }
    let dir = setup_temp_dir();
    run_binary(&dir, b"Finish the API integration by Thursday. John is the tech lead.\n");

    let events = read_events(&dir);
    let types: Vec<&str> = events.iter()
        .map(|e| e["event_type"].as_str().unwrap())
        .collect();

    if types.contains(&"ItemsExtracted") {
        let extracted_pos = types.iter().position(|&t| t == "ItemsExtracted").unwrap();
        let rejected_pos  = types.iter().position(|&t| t == "ExtractionRejected").unwrap();
        assert!(extracted_pos < rejected_pos, "ItemsExtracted must precede ExtractionRejected");
    }
}

// ── Failure Path 4: ApiRequestFailed (Refinement 2) ──────────────────────────

#[test]
fn test_api_failure_emits_terminal_event_not_orphaned_chain() {
    let dir = setup_temp_dir();
    // Run without any API key set — forces API failure path
    let mut child = Command::new(binary_path())
        .current_dir(dir.path())
        .env_remove("GEMINI_API_KEY_PMCLI")
        .env_remove("GEMINI_API_KEY")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn binary");

    child.stdin.as_mut().unwrap()
        .write_all(b"Deploy the release by end of week.\n")
        .unwrap();
    child.wait_with_output().unwrap();

    let events = read_events(&dir);
    let types: Vec<&str> = events.iter()
        .map(|e| e["event_type"].as_str().unwrap())
        .collect();

    assert!(types.contains(&"TextSubmitted"), "TextSubmitted must be emitted");
    assert!(types.contains(&"ExtractionFailedApiRequest"),
        "ExtractionFailedApiRequest must be emitted — chain must not be orphaned");

    let failure = events.iter()
        .find(|e| e["event_type"] == "ExtractionFailedApiRequest")
        .unwrap();
    assert_eq!(failure["payload"]["failure_reason"].as_str().unwrap(), "api_request_failed");
    assert!(failure["payload"]["error_detail"].as_str().is_some(),
        "error_detail must be present");
}

// ── Telemetry: required base fields ──────────────────────────────────────────

#[test]
fn test_all_events_have_required_base_fields() {
    let dir = setup_temp_dir();
    run_binary(&dir, b"");

    let events = read_events(&dir);
    assert!(!events.is_empty(), "At least one event must be emitted");

    for event in &events {
        let t = event["event_type"].as_str().unwrap_or("unknown");
        assert!(!event["event_id"].is_null(),       "{}: event_id must be present", t);
        assert!(!event["event_type"].is_null(),     "{}: event_type must be present", t);
        assert!(!event["timestamp"].is_null(),      "{}: timestamp must be present", t);
        assert!(!event["correlation_id"].is_null(), "{}: correlation_id must be present", t);
        assert!(!event["source_module"].is_null(),  "{}: source_module must be present", t);
        assert!(!event["payload"].is_null(),        "{}: payload must be present", t);
        assert_eq!(event["source_module"].as_str().unwrap(), "pm_structuring",
            "{}: source_module must be 'pm_structuring'", t);
        assert!(event["timestamp"].as_u64().unwrap() > 0,
            "{}: timestamp must be a positive integer", t);
    }
}

#[test]
fn test_correlation_id_is_same_across_all_events_in_one_run() {
    let dir = setup_temp_dir();
    run_binary(&dir, b"");

    let events = read_events(&dir);
    assert!(events.len() >= 2);

    let first_id = events[0]["correlation_id"].as_str().unwrap();
    for event in &events {
        assert_eq!(event["correlation_id"].as_str().unwrap(), first_id,
            "All events in one run must share the same correlation_id");
    }
}

#[test]
fn test_correlation_id_is_uuid_v4_format() {
    let dir = setup_temp_dir();
    run_binary(&dir, b"");

    let events = read_events(&dir);
    let corr_id = events[0]["correlation_id"].as_str().unwrap();

    assert_eq!(corr_id.len(), 36);
    assert_eq!(corr_id.chars().filter(|&c| c == '-').count(), 4);
    let hex_only: String = corr_id.chars().filter(|&c| c != '-').collect();
    assert!(hex_only.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn test_source_text_preserved_in_text_submitted_event() {
    let dir = setup_temp_dir();
    run_binary(&dir, b"");

    let events = read_events(&dir);
    let submitted = events.iter()
        .find(|e| e["event_type"] == "TextSubmitted")
        .expect("TextSubmitted event not found");

    assert!(!submitted["payload"]["source_text"].is_null());
    assert!(submitted["payload"]["input_length"].as_u64().is_some());
}

// ── R2: Folder Ingestion ──────────────────────────────────────────────────────

fn create_journal_file(dir: &TempDir, filename: &str, content: &str) {
    let journal_dir = dir.path().join("journal");
    fs::create_dir_all(&journal_dir).unwrap();
    fs::write(journal_dir.join(filename), content).unwrap();
}

fn seed_source_file_processed(dir: &TempDir, filename: &str) {
    let events_path = dir.path().join("events/runtime_events.jsonl");
    let event = json!({
        "event_id": "seeded-event-id-0001",
        "event_type": "ItemsExtracted",
        "timestamp": 1748200001000u64,
        "correlation_id": "seeded-corr-id-0001",
        "source_module": "pm_structuring",
        "payload": {
            "items": [],
            "item_count": 0,
            "uncertain_count": 0,
            "source_file": filename
        }
    });
    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&events_path)
        .unwrap();
    writeln!(f, "{}", event).unwrap();
}

fn run_folder(dir: &TempDir, folder_rel_path: &str, extra_args: &[&str]) -> std::process::Output {
    let mut args = vec!["--folder", folder_rel_path];
    args.extend_from_slice(extra_args);
    run_binary_with_args(dir, b"", &args)
}

// ── R2: FolderNotFound failure path ──────────────────────────────────────────

#[test]
fn test_folder_not_found_emits_failure_and_exits_nonzero() {
    let dir = setup_temp_dir();
    let output = run_folder(&dir, "nonexistent_folder", &[]);

    assert!(!output.status.success(), "Must exit nonzero when folder not found");

    let events = read_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"FolderScanRequested"),
        "FolderScanRequested must be emitted before the check");
    assert!(types.contains(&"ExtractionFailedFolderNotFound"),
        "ExtractionFailedFolderNotFound must be emitted");
    assert!(!types.contains(&"FolderScanCompleted"),
        "FolderScanCompleted must NOT be emitted on FolderNotFound");
}

#[test]
fn test_folder_not_found_failure_reason() {
    let dir = setup_temp_dir();
    run_folder(&dir, "nonexistent_folder", &[]);

    let events = read_events(&dir);
    let failure = events.iter()
        .find(|e| e["event_type"] == "ExtractionFailedFolderNotFound")
        .expect("ExtractionFailedFolderNotFound not found");

    assert_eq!(failure["payload"]["failure_reason"].as_str().unwrap(), "folder_not_found");
    assert!(failure["payload"]["folder_path"].as_str().is_some(), "folder_path must be present");
}

#[test]
fn test_folder_not_found_scan_requested_precedes_failure() {
    let dir = setup_temp_dir();
    run_folder(&dir, "nonexistent_folder", &[]);

    let events = read_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    let req_pos     = types.iter().position(|&t| t == "FolderScanRequested").unwrap();
    let failure_pos = types.iter().position(|&t| t == "ExtractionFailedFolderNotFound").unwrap();
    assert!(req_pos < failure_pos, "FolderScanRequested must precede ExtractionFailedFolderNotFound");
}

// ── R2: Empty folder / non-eligible files ────────────────────────────────────

#[test]
fn test_empty_folder_emits_scan_requested_and_completed() {
    let dir = setup_temp_dir();
    fs::create_dir_all(dir.path().join("journal")).unwrap();

    run_folder(&dir, "journal", &[]);

    let events = read_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"FolderScanRequested"), "FolderScanRequested must be emitted");
    assert!(types.contains(&"FolderScanCompleted"),  "FolderScanCompleted must be emitted");
    assert!(!types.contains(&"ExtractionFailedFolderNotFound"),
        "No failure event expected for empty folder");
}

#[test]
fn test_empty_folder_scan_completed_counts_are_zero() {
    let dir = setup_temp_dir();
    fs::create_dir_all(dir.path().join("journal")).unwrap();

    run_folder(&dir, "journal", &[]);

    let events = read_events(&dir);
    let completed = events.iter()
        .find(|e| e["event_type"] == "FolderScanCompleted")
        .expect("FolderScanCompleted not found");

    assert_eq!(completed["payload"]["files_found"].as_u64().unwrap(), 0);
    assert_eq!(completed["payload"]["files_skipped"].as_u64().unwrap(), 0);
    assert_eq!(completed["payload"]["files_processed"].as_u64().unwrap(), 0);
}

#[test]
fn test_folder_non_eligible_files_not_counted_as_found() {
    let dir = setup_temp_dir();
    let journal_dir = dir.path().join("journal");
    fs::create_dir_all(&journal_dir).unwrap();
    fs::write(journal_dir.join("notes.bak"), "backup content").unwrap();
    fs::write(journal_dir.join("data.json"), "{}").unwrap();

    run_folder(&dir, "journal", &[]);

    let events = read_events(&dir);
    let completed = events.iter()
        .find(|e| e["event_type"] == "FolderScanCompleted")
        .expect("FolderScanCompleted not found");

    assert_eq!(completed["payload"]["files_found"].as_u64().unwrap(), 0,
        "Non-.txt/.md files must not count as eligible");
    assert!(!events.iter().any(|e| e["event_type"] == "TextSubmitted"),
        "TextSubmitted must not be emitted for non-eligible files");
}

// ── R2: Scan event ordering and correlation ───────────────────────────────────

#[test]
fn test_folder_scan_requested_precedes_completed() {
    let dir = setup_temp_dir();
    fs::create_dir_all(dir.path().join("journal")).unwrap();

    run_folder(&dir, "journal", &[]);

    let events = read_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    let req_pos = types.iter().position(|&t| t == "FolderScanRequested").unwrap();
    let cmp_pos = types.iter().position(|&t| t == "FolderScanCompleted").unwrap();
    assert!(req_pos < cmp_pos, "FolderScanRequested must precede FolderScanCompleted");
}

#[test]
fn test_folder_scan_events_share_correlation_id() {
    let dir = setup_temp_dir();
    fs::create_dir_all(dir.path().join("journal")).unwrap();

    run_folder(&dir, "journal", &[]);

    let events = read_events(&dir);
    let req = events.iter().find(|e| e["event_type"] == "FolderScanRequested").unwrap();
    let cmp = events.iter().find(|e| e["event_type"] == "FolderScanCompleted").unwrap();

    assert_eq!(
        req["correlation_id"].as_str().unwrap(),
        cmp["correlation_id"].as_str().unwrap(),
        "FolderScanRequested and FolderScanCompleted must share correlation_id"
    );
}

// ── R2: FolderScanRequested payload ──────────────────────────────────────────

#[test]
fn test_folder_scan_requested_auto_confirm_false_without_yes() {
    let dir = setup_temp_dir();
    fs::create_dir_all(dir.path().join("journal")).unwrap();

    run_folder(&dir, "journal", &[]);

    let events = read_events(&dir);
    let req = events.iter().find(|e| e["event_type"] == "FolderScanRequested").unwrap();

    assert_eq!(req["payload"]["auto_confirm"].as_bool().unwrap(), false);
    assert!(req["payload"]["folder_path"].as_str().is_some(), "folder_path must be present");
}

#[test]
fn test_folder_scan_requested_auto_confirm_true_with_yes() {
    let dir = setup_temp_dir();
    fs::create_dir_all(dir.path().join("journal")).unwrap();

    run_folder(&dir, "journal", &["--yes"]);

    let events = read_events(&dir);
    let req = events.iter().find(|e| e["event_type"] == "FolderScanRequested").unwrap();

    assert_eq!(req["payload"]["auto_confirm"].as_bool().unwrap(), true);
}

// ── R2: Deduplication ────────────────────────────────────────────────────────

#[test]
fn test_folder_deduplication_skips_already_processed_file() {
    let dir = setup_temp_dir();
    create_journal_file(&dir, "2026-05-28-notes.md", "Deploy by Friday.");
    seed_source_file_processed(&dir, "2026-05-28-notes.md");

    run_folder(&dir, "journal", &[]);

    // No new TextSubmitted: the file was recognised as already processed
    let events = read_events(&dir);
    let new_text_submitted: Vec<&Value> = events.iter()
        .filter(|e| e["event_type"] == "TextSubmitted")
        .collect();
    assert!(new_text_submitted.is_empty(),
        "Already-processed file must not produce a new TextSubmitted event");
}

#[test]
fn test_folder_deduplication_files_skipped_count() {
    let dir = setup_temp_dir();
    create_journal_file(&dir, "2026-05-28-notes.md", "Deploy by Friday.");
    seed_source_file_processed(&dir, "2026-05-28-notes.md");

    run_folder(&dir, "journal", &[]);

    let events = read_events(&dir);
    let completed = events.iter()
        .find(|e| e["event_type"] == "FolderScanCompleted")
        .expect("FolderScanCompleted not found");

    assert_eq!(completed["payload"]["files_found"].as_u64().unwrap(), 1);
    assert_eq!(completed["payload"]["files_skipped"].as_u64().unwrap(), 1);
    assert_eq!(completed["payload"]["files_processed"].as_u64().unwrap(), 0);
}

// ── R2: Live processing (requires Gemini key) ─────────────────────────────────

#[test]
fn test_folder_processes_new_file_full_event_spine() {
    if !gemini_key_available() { return; }
    let dir = setup_temp_dir();
    create_journal_file(&dir, "2026-05-28-notes.md",
        "Deploy the release by end of week. Sarah is release manager. Risk: vendor delays.\n");

    run_folder(&dir, "journal", &["--yes"]);

    let events = read_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"FolderScanRequested"), "FolderScanRequested must be emitted");
    assert!(types.contains(&"TextSubmitted"),        "TextSubmitted must be emitted per file");
    assert!(types.contains(&"ItemsExtracted"),       "ItemsExtracted must be emitted per file");
    assert!(types.contains(&"FolderScanCompleted"),  "FolderScanCompleted must be emitted");
}

#[test]
fn test_folder_items_extracted_source_file_matches_filename() {
    if !gemini_key_available() { return; }
    let dir = setup_temp_dir();
    create_journal_file(&dir, "2026-05-28-notes.md",
        "Deploy the release by end of week. Sarah is release manager.\n");

    run_folder(&dir, "journal", &["--yes"]);

    let events = read_events(&dir);
    let extracted = events.iter()
        .find(|e| e["event_type"] == "ItemsExtracted")
        .expect("ItemsExtracted not found");

    assert_eq!(
        extracted["payload"]["source_file"].as_str().unwrap(),
        "2026-05-28-notes.md",
        "source_file must be the filename of the processed file"
    );
}

#[test]
fn test_folder_scan_completed_files_processed_count() {
    if !gemini_key_available() { return; }
    let dir = setup_temp_dir();
    create_journal_file(&dir, "2026-05-28-notes.md",
        "Deploy the release by end of week. Sarah is release manager.\n");

    run_folder(&dir, "journal", &["--yes"]);

    let events = read_events(&dir);
    let completed = events.iter()
        .find(|e| e["event_type"] == "FolderScanCompleted")
        .expect("FolderScanCompleted not found");

    assert_eq!(completed["payload"]["files_found"].as_u64().unwrap(), 1);
    assert_eq!(completed["payload"]["files_processed"].as_u64().unwrap(), 1);
    assert_eq!(completed["payload"]["files_skipped"].as_u64().unwrap(), 0);
}

#[test]
fn test_folder_partial_skip_processes_only_new_files() {
    if !gemini_key_available() { return; }
    let dir = setup_temp_dir();
    create_journal_file(&dir, "2026-05-27-old.md",
        "Completed the API integration yesterday.\n");
    create_journal_file(&dir, "2026-05-28-new.md",
        "Deploy the release by end of week. Sarah is release manager.\n");
    seed_source_file_processed(&dir, "2026-05-27-old.md");

    run_folder(&dir, "journal", &["--yes"]);

    let events = read_events(&dir);
    let completed = events.iter()
        .find(|e| e["event_type"] == "FolderScanCompleted")
        .expect("FolderScanCompleted not found");

    assert_eq!(completed["payload"]["files_found"].as_u64().unwrap(), 2);
    assert_eq!(completed["payload"]["files_skipped"].as_u64().unwrap(), 1);
    assert_eq!(completed["payload"]["files_processed"].as_u64().unwrap(), 1);

    let new_run_extracted: Vec<&Value> = events.iter()
        .filter(|e| e["event_type"] == "ItemsExtracted"
            && e["payload"]["source_file"].as_str() == Some("2026-05-28-new.md"))
        .collect();
    assert!(!new_run_extracted.is_empty(), "New file must be processed and source_file set");
}

#[test]
fn test_stdin_items_extracted_source_file_is_null() {
    if !gemini_key_available() { return; }
    let dir = setup_temp_dir();
    run_binary_with_args(
        &dir,
        b"Deploy the release by end of week. Sarah is release manager.\n",
        &["--yes"],
    );

    let events = read_events(&dir);
    if let Some(extracted) = events.iter().find(|e| e["event_type"] == "ItemsExtracted") {
        assert!(extracted["payload"]["source_file"].is_null(),
            "source_file must be null for stdin sessions");
    }
}

// ── R6: item_type now includes "unknown" as a valid value ─────────────────────
// Update the live extraction payload shape test to accept "unknown" alongside
// the five standard types — it can appear when the LLM produces a type not
// recognized by the active vocabulary.

#[test]
fn test_items_extracted_item_type_valid_includes_unknown() {
    if !gemini_key_available() { return; }
    let dir = setup_temp_dir();
    run_binary(&dir, b"Set up CI pipeline by Friday. Bob is the release manager.\n");

    let events = read_events(&dir);
    if let Some(extracted) = events.iter().find(|e| e["event_type"] == "ItemsExtracted") {
        let items = extracted["payload"]["items"].as_array().expect("items must be an array");
        let valid_types = ["task", "milestone", "risk", "issue", "stakeholder", "unknown"];
        for item in items {
            let item_type = item["item_type"].as_str().unwrap();
            assert!(valid_types.contains(&item_type),
                "item_type '{}' must be vocabulary-recognized or 'unknown'", item_type);
        }
    }
}

// ── R6: HP5 — Proposed status null when outside vocabulary status set ─────────

#[test]
fn test_r6_proposed_status_null_when_outside_vocabulary_set() {
    if !gemini_key_available() { return; }
    let dir = setup_temp_dir();
    // Vocabulary with a single recognized type and a single valid status ("scheduled").
    // Any other status the LLM might propose (e.g., "todo", "open") is not in the
    // vocabulary status set for this type and must be stored as null (HP5: silently set
    // to null, no failure signal, item not marked uncertain).
    write_project_schema(&dir, r#"schemaVersion: 1
statuses:
  scheduled:
pageTypes:
  Action:
    allowedStatuses: [scheduled]
    aliases: [action]
"#);

    run_binary_isolated(
        &dir,
        b"Deploy the new release. Fix the login bug. Review the architecture plan.\n",
        &["--yes"],
    );

    let events = read_events(&dir);
    if let Some(extracted) = events.iter().find(|e| e["event_type"] == "ItemsExtracted") {
        let items = extracted["payload"]["items"].as_array().unwrap();
        for item in items {
            let item_type = item["item_type"].as_str().unwrap();
            if item_type == "unknown" { continue; }
            // HP5: proposed_status must be either the single valid value or null
            if let Some(status) = item["proposed_status"].as_str() {
                assert_eq!(status, "scheduled",
                    "proposed_status '{}' for recognized type '{}' must be 'scheduled' (only valid \
                    status in vocabulary) — out-of-vocabulary values must be null, not recorded (HP5)",
                    status, item_type);
            }
            // HP5: an item is not marked uncertain solely due to out-of-vocab proposed_status
            if item_type == "action" || item_type == "Action" {
                assert!(!item["uncertain"].as_bool().unwrap_or(false),
                    "HP5: out-of-vocabulary proposed_status must not mark the item as uncertain");
            }
        }
    }
}

// ── R6: FP1 — SchemaInvalid aborts extraction (stdin mode) ───────────────────

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

#[test]
fn test_r6_schema_invalid_stdin_text_submitted_not_emitted() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, "this: is: not: valid: yaml: [[[");

    run_binary(&dir, b"Deploy the release by end of week.\n");

    let events = read_all_events(&dir);
    let pms_events: Vec<&Value> = events.iter()
        .filter(|e| e["source_module"].as_str() == Some("pm_structuring"))
        .collect();

    assert!(pms_events.is_empty(),
        "No pm_structuring events must be emitted when schema is invalid — TextSubmitted must not appear");
}

#[test]
fn test_r6_schema_invalid_stdin_emits_project_schema_failure_event() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, "this: is: not: valid: yaml: [[[");

    run_binary(&dir, b"Deploy the release by end of week.\n");

    let events = read_all_events(&dir);
    let failures: Vec<&Value> = events.iter()
        .filter(|e| e["source_module"].as_str() == Some("project_schema"))
        .filter(|e| matches!(e["event_type"].as_str(),
            Some("SchemaParseError") | Some("SchemaValidationFailed")))
        .collect();

    assert!(!failures.is_empty(),
        "project_schema module must emit a failure event when schema is invalid");
}

#[test]
fn test_r6_schema_invalid_stdin_no_extraction_events_in_record() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, "this: is: not: valid: yaml: [[[");

    run_binary(&dir, b"Deploy the release by end of week.\n");

    let events = read_all_events(&dir);
    let extraction_events: Vec<&Value> = events.iter()
        .filter(|e| matches!(e["event_type"].as_str(),
            Some("TextSubmitted") | Some("ItemsExtracted") |
            Some("ExtractionConfirmed") | Some("ExtractionRejected")))
        .collect();

    assert!(extraction_events.is_empty(),
        "No extraction events must be written when schema is invalid — project record is unchanged");
}

#[test]
fn test_r6_schema_invalid_project_schema_failure_has_required_fields() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, "this: is: not: valid: yaml: [[[");

    run_binary(&dir, b"Deploy the release by end of week.\n");

    let events = read_all_events(&dir);
    let failure = events.iter()
        .find(|e| e["source_module"].as_str() == Some("project_schema")
            && matches!(e["event_type"].as_str(),
                Some("SchemaParseError") | Some("SchemaValidationFailed")))
        .expect("project_schema failure event must be present");

    assert!(failure["event_id"].as_str().is_some(),       "event_id must be a string");
    assert!(failure["event_type"].as_str().is_some(),     "event_type must be a string");
    assert!(failure["timestamp"].as_u64().is_some(),      "timestamp must be a u64");
    assert!(failure["correlation_id"].as_str().is_some(), "correlation_id must be a string");
    assert!(failure["source_module"].as_str().is_some(),  "source_module must be a string");
    assert!(failure["payload"].is_object(),               "payload must be an object");
}

// ── R6: FP1 — SchemaInvalid aborts extraction (folder mode) ──────────────────
// Schema failure aborts before FolderScanRequested per the event schema flow.

#[test]
fn test_r6_schema_invalid_folder_scan_requested_not_emitted() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, "this: is: not: valid: yaml: [[[");
    fs::create_dir_all(dir.path().join("journal")).unwrap();

    run_folder(&dir, "journal", &[]);

    let events = read_all_events(&dir);
    let types: Vec<&str> = events.iter()
        .filter(|e| e["source_module"].as_str() == Some("pm_structuring"))
        .map(|e| e["event_type"].as_str().unwrap())
        .collect();

    assert!(!types.contains(&"FolderScanRequested"),
        "FolderScanRequested must NOT be emitted when schema is invalid — abort is before folder dispatch");
    assert!(!types.contains(&"FolderScanCompleted"),
        "FolderScanCompleted must NOT be emitted when schema is invalid");
}

#[test]
fn test_r6_schema_invalid_folder_emits_project_schema_failure_event() {
    let dir = setup_temp_dir();
    write_project_schema(&dir, "this: is: not: valid: yaml: [[[");
    fs::create_dir_all(dir.path().join("journal")).unwrap();

    run_folder(&dir, "journal", &[]);

    let events = read_all_events(&dir);
    let failures: Vec<&Value> = events.iter()
        .filter(|e| e["source_module"].as_str() == Some("project_schema"))
        .filter(|e| matches!(e["event_type"].as_str(),
            Some("SchemaParseError") | Some("SchemaValidationFailed")))
        .collect();

    assert!(!failures.is_empty(),
        "project_schema module must emit a failure event when schema is invalid in folder mode");
}

// ── R6: HP1 — Custom vocabulary governs type classification ──────────────────
// These tests isolate the project schema by removing HOME from the child
// process environment, which prevents load_and_validate from merging with
// ~/.lucidpm/default-schema.yaml. The only recognized types are those
// defined in the project-schema.yaml written to the temp dir.

const CUSTOM_VOCAB_SCHEMA: &str = r#"schemaVersion: 1
statuses:
  pending:
  active:
  completed:
  open:
  resolved:
  inactive:
pageTypes:
  Action:
    allowedStatuses: [pending, active, completed]
    aliases: [action]
  Person:
    allowedStatuses: [active, inactive]
    aliases: [person]
  Problem:
    allowedStatuses: [open, resolved]
    aliases: [problem]
"#;

// Like run_binary_with_args but removes HOME to prevent default schema merge.
fn run_binary_isolated(dir: &TempDir, stdin_bytes: &[u8], args: &[&str]) -> std::process::Output {
    let mut child = Command::new(binary_path())
        .current_dir(dir.path())
        .args(args)
        .env_remove("HOME")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn binary");
    child.stdin.as_mut().unwrap().write_all(stdin_bytes).unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn test_r6_custom_vocabulary_item_types_are_vocabulary_recognized_or_unknown() {
    if !gemini_key_available() { return; }
    let dir = setup_temp_dir();
    write_project_schema(&dir, CUSTOM_VOCAB_SCHEMA);

    // HOME removed: only CUSTOM_VOCAB_SCHEMA types are valid; standard types are unrecognized.
    run_binary_isolated(
        &dir,
        b"Deploy the new release by end of week. Sarah is the release manager. Risk: vendor delays.\n",
        &["--yes"],
    );

    let events = read_events(&dir);
    if let Some(extracted) = events.iter().find(|e| e["event_type"] == "ItemsExtracted") {
        let items = extracted["payload"]["items"].as_array().unwrap();
        let recognized = ["action", "Action", "person", "Person", "problem", "Problem", "unknown"];
        for item in items {
            let item_type = item["item_type"].as_str().unwrap();
            assert!(recognized.contains(&item_type),
                "item_type '{}' must be vocabulary-recognized ('action', 'person', 'problem') or \
                'unknown' — hardcoded legacy types are not in the active vocabulary",
                item_type);
        }
    }
}

#[test]
fn test_r6_custom_vocabulary_proposed_status_from_vocabulary_status_set() {
    if !gemini_key_available() { return; }
    let dir = setup_temp_dir();
    write_project_schema(&dir, CUSTOM_VOCAB_SCHEMA);

    run_binary_isolated(
        &dir,
        b"Deploy the new release by end of week. Sarah is the release manager. Risk: vendor delays.\n",
        &["--yes"],
    );

    let events = read_events(&dir);
    if let Some(extracted) = events.iter().find(|e| e["event_type"] == "ItemsExtracted") {
        let items = extracted["payload"]["items"].as_array().unwrap();
        let valid_statuses = ["pending", "active", "completed", "open", "resolved", "inactive"];
        for item in items {
            let item_type = item["item_type"].as_str().unwrap();
            if item_type == "unknown" {
                // HP4: unrecognized type → proposed_status must be null
                assert!(item["proposed_status"].is_null(),
                    "item_type='unknown' must have proposed_status=null (HP4)");
            } else if let Some(status) = item["proposed_status"].as_str() {
                // HP3: recognized type → proposed_status from vocabulary status set
                assert!(valid_statuses.contains(&status),
                    "proposed_status '{}' for item_type '{}' must be from the vocabulary status set",
                    status, item_type);
            }
        }
    }
}

#[test]
fn test_r6_unknown_item_satisfies_uncertainty_invariants() {
    if !gemini_key_available() { return; }
    let dir = setup_temp_dir();
    write_project_schema(&dir, CUSTOM_VOCAB_SCHEMA);

    // HOME removed: standard types (task, milestone, etc.) are unrecognized → stored as "unknown".
    run_binary_isolated(
        &dir,
        b"Deploy the new release by end of week. Sarah is the release manager. Risk: vendor delays.\n",
        &["--yes"],
    );

    let events = read_events(&dir);
    if let Some(extracted) = events.iter().find(|e| e["event_type"] == "ItemsExtracted") {
        let items = extracted["payload"]["items"].as_array().unwrap();
        for item in items {
            if item["item_type"].as_str() == Some("unknown") {
                // HP2: unrecognized type → uncertain=true, uncertainty_reason set, proposed_status=null
                assert_eq!(item["uncertain"].as_bool().unwrap_or(false), true,
                    "item_type='unknown' must have uncertain=true (HP2)");
                assert!(!item["uncertainty_reason"].is_null(),
                    "item_type='unknown' must have a non-null uncertainty_reason (HP2)");
                let reason = item["uncertainty_reason"].as_str().unwrap();
                assert!(reason.contains("not recognized"),
                    "uncertainty_reason must identify the type as unrecognized by the vocabulary, got: '{}'",
                    reason);
                assert!(item["proposed_status"].is_null(),
                    "item_type='unknown' must have proposed_status=null (HP4)");
            }
        }
    }
}

//! Replay verification tests for pm_structuring.
//!
//! Loads JSONL event fixtures and verifies they conform to the approved event
//! schema (events/pm_structuring_schema.md): required fields, valid event types,
//! correct payload shapes, and valid event sequences.

use serde_json::Value;

fn load_fixture(name: &str) -> Vec<Value> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/replay/fixtures")
        .join(name);
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("Could not read fixture: {}", path.display()));
    content
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

const VALID_EVENT_TYPES: &[&str] = &[
    "TextSubmitted",
    "ItemsExtracted",
    "ExtractionConfirmed",
    "ExtractionRejected",
    "ExtractionFailedEmptyInput",
    "ExtractionFailedNoContent",
    "ExtractionFailedApiRequest",
    "FolderScanRequested",
    "FolderScanCompleted",
    "ExtractionFailedFolderNotFound",
];

const VALID_ITEM_TYPES: &[&str] = &["task", "milestone", "risk", "issue", "stakeholder"];

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

// ── Schema conformance ────────────────────────────────────────────────────────

#[test]
fn test_happy_path_fixture_all_events_have_required_base_fields() {
    let events = load_fixture("pm_structuring_happy_path.jsonl");
    assert!(!events.is_empty(), "Fixture must not be empty");

    for event in &events {
        let t = event["event_type"].as_str().unwrap_or("unknown");
        assert!(event["event_id"].as_str().is_some(),       "{}: event_id must be a string", t);
        assert!(event["event_type"].as_str().is_some(),     "{}: event_type must be a string", t);
        assert!(event["timestamp"].as_u64().is_some(),      "{}: timestamp must be a u64", t);
        assert!(event["correlation_id"].as_str().is_some(), "{}: correlation_id must be a string", t);
        assert!(event["source_module"].as_str().is_some(),  "{}: source_module must be a string", t);
        assert!(event["payload"].is_object(),               "{}: payload must be an object", t);
        assert_eq!(
            event["source_module"].as_str().unwrap(), "pm_structuring",
            "{}: source_module must be 'pm_structuring'", t
        );
    }
}

#[test]
fn test_happy_path_fixture_event_types_are_schema_members() {
    let events = load_fixture("pm_structuring_happy_path.jsonl");
    for event in &events {
        let t = event["event_type"].as_str().unwrap();
        assert!(VALID_EVENT_TYPES.contains(&t),
            "Event type '{}' is not in the approved schema", t);
    }
}

#[test]
fn test_happy_path_fixture_correlation_id_consistent() {
    let events = load_fixture("pm_structuring_happy_path.jsonl");
    let first = events[0]["correlation_id"].as_str().unwrap();
    for event in &events {
        assert_eq!(
            event["correlation_id"].as_str().unwrap(), first,
            "All events in a run must share the same correlation_id"
        );
    }
}

// ── Sequence conformance ──────────────────────────────────────────────────────

#[test]
fn test_happy_path_fixture_starts_with_text_submitted() {
    let events = load_fixture("pm_structuring_happy_path.jsonl");
    assert_eq!(
        events[0]["event_type"].as_str().unwrap(), "TextSubmitted",
        "First event must always be TextSubmitted"
    );
}

#[test]
fn test_happy_path_fixture_ends_with_terminal_event() {
    let events = load_fixture("pm_structuring_happy_path.jsonl");
    let terminal = ["ExtractionConfirmed", "ExtractionRejected",
                    "ExtractionFailedEmptyInput", "ExtractionFailedNoContent"];
    let last = events.last().unwrap()["event_type"].as_str().unwrap();
    assert!(terminal.contains(&last),
        "Last event must be a terminal event, got: {}", last);
}

#[test]
fn test_happy_path_fixture_items_extracted_before_confirmed() {
    let events = load_fixture("pm_structuring_happy_path.jsonl");
    let types: Vec<&str> = events.iter()
        .map(|e| e["event_type"].as_str().unwrap())
        .collect();

    if types.contains(&"ExtractionConfirmed") {
        let extracted_pos = types.iter().position(|&t| t == "ItemsExtracted")
            .expect("ItemsExtracted must precede ExtractionConfirmed");
        let confirmed_pos = types.iter().position(|&t| t == "ExtractionConfirmed").unwrap();
        assert!(extracted_pos < confirmed_pos);
    }
}

// ── Payload shape conformance ─────────────────────────────────────────────────

#[test]
fn test_happy_path_fixture_text_submitted_payload() {
    let events = load_fixture("pm_structuring_happy_path.jsonl");
    let submitted = events.iter().find(|e| e["event_type"] == "TextSubmitted").unwrap();

    assert!(submitted["payload"]["source_text"].as_str().is_some(),
        "TextSubmitted.source_text must be a string");
    assert!(submitted["payload"]["input_length"].as_u64().is_some(),
        "TextSubmitted.input_length must be a u64");

    let source_len = submitted["payload"]["source_text"].as_str().unwrap().len() as u64;
    let stated_len = submitted["payload"]["input_length"].as_u64().unwrap();
    assert_eq!(source_len, stated_len,
        "input_length must equal actual source_text byte length");
}

#[test]
fn test_happy_path_fixture_items_extracted_payload() {
    let events = load_fixture("pm_structuring_happy_path.jsonl");
    if let Some(extracted) = events.iter().find(|e| e["event_type"] == "ItemsExtracted") {
        let items = extracted["payload"]["items"].as_array().unwrap();
        let item_count = extracted["payload"]["item_count"].as_u64().unwrap();
        let uncertain_count = extracted["payload"]["uncertain_count"].as_u64().unwrap();

        assert_eq!(items.len() as u64, item_count);
        let actual_uncertain = items.iter()
            .filter(|i| i["uncertain"].as_bool().unwrap_or(false))
            .count() as u64;
        assert_eq!(actual_uncertain, uncertain_count);

        for item in items {
            assert!(item["item_id"].as_str().is_some(),      "item_id must be a string");
            assert!(item["description"].as_str().is_some(),  "description must be a string");
            assert!(item["uncertain"].as_bool().is_some(),   "uncertain must be a bool");

            let item_type = item["item_type"].as_str().unwrap();
            assert!(VALID_ITEM_TYPES.contains(&item_type),
                "item_type '{}' not in schema", item_type);

            if item["uncertain"].as_bool().unwrap() {
                assert!(!item["uncertainty_reason"].is_null(),
                    "uncertainty_reason must not be null when uncertain is true");
            }
        }
    }
}

#[test]
fn test_happy_path_fixture_extraction_confirmed_payload() {
    let events = load_fixture("pm_structuring_happy_path.jsonl");
    if let Some(confirmed) = events.iter().find(|e| e["event_type"] == "ExtractionConfirmed") {
        let ids = confirmed["payload"]["accepted_item_ids"].as_array()
            .expect("accepted_item_ids must be an array");
        let count = confirmed["payload"]["accepted_count"].as_u64()
            .expect("accepted_count must be a u64");
        assert_eq!(ids.len() as u64, count,
            "accepted_count must match accepted_item_ids length");
    }
}

// ── Proposed status/priority conformance (R1) ─────────────────────────────────

#[test]
fn test_happy_path_fixture_items_have_proposed_fields() {
    let events = load_fixture("pm_structuring_happy_path.jsonl");
    if let Some(extracted) = events.iter().find(|e| e["event_type"] == "ItemsExtracted") {
        let items = extracted["payload"]["items"].as_array().unwrap();
        assert!(!items.is_empty(), "Fixture must contain at least one item");
        for item in items {
            let t = item["item_type"].as_str().unwrap_or("unknown");
            assert!(item.get("proposed_status").is_some(),
                "{}: proposed_status field must be present (may be null)", t);
            assert!(item.get("proposed_priority").is_some(),
                "{}: proposed_priority field must be present (may be null)", t);
        }
    }
}

#[test]
fn test_happy_path_fixture_proposed_status_values_are_valid() {
    let events = load_fixture("pm_structuring_happy_path.jsonl");
    if let Some(extracted) = events.iter().find(|e| e["event_type"] == "ItemsExtracted") {
        let items = extracted["payload"]["items"].as_array().unwrap();
        for item in items {
            if let Some(status) = item["proposed_status"].as_str() {
                let item_type = item["item_type"].as_str().unwrap();
                let valid = valid_statuses_for(item_type);
                assert!(valid.contains(&status),
                    "proposed_status '{}' is not valid for item_type '{}'", status, item_type);
            }
        }
    }
}

#[test]
fn test_happy_path_fixture_proposed_priority_values_are_valid() {
    let events = load_fixture("pm_structuring_happy_path.jsonl");
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
fn test_happy_path_fixture_confirmed_ids_match_extracted_ids() {
    let events = load_fixture("pm_structuring_happy_path.jsonl");

    let extracted = events.iter().find(|e| e["event_type"] == "ItemsExtracted");
    let confirmed  = events.iter().find(|e| e["event_type"] == "ExtractionConfirmed");

    if let (Some(ext), Some(conf)) = (extracted, confirmed) {
        let extracted_ids: Vec<&str> = ext["payload"]["items"].as_array().unwrap()
            .iter().map(|i| i["item_id"].as_str().unwrap()).collect();
        let accepted_ids: Vec<&str> = conf["payload"]["accepted_item_ids"].as_array().unwrap()
            .iter().map(|i| i.as_str().unwrap()).collect();
        assert_eq!(extracted_ids, accepted_ids,
            "accepted_item_ids must match item_ids from ItemsExtracted");
    }
}

// ── R2: Folder happy path fixture ─────────────────────────────────────────────

fn folder_events() -> Vec<Value> {
    load_fixture("pm_structuring_folder_happy_path.jsonl")
}

fn file_events(all: &[Value]) -> Vec<&Value> {
    all.iter()
        .filter(|e| matches!(e["event_type"].as_str(),
            Some("TextSubmitted") | Some("ItemsExtracted") |
            Some("ExtractionConfirmed") | Some("ExtractionRejected")))
        .collect()
}

#[test]
fn test_folder_fixture_all_events_have_required_base_fields() {
    let events = folder_events();
    assert!(!events.is_empty(), "Fixture must not be empty");

    for event in &events {
        let t = event["event_type"].as_str().unwrap_or("unknown");
        assert!(event["event_id"].as_str().is_some(),       "{}: event_id must be a string", t);
        assert!(event["event_type"].as_str().is_some(),     "{}: event_type must be a string", t);
        assert!(event["timestamp"].as_u64().is_some(),      "{}: timestamp must be a u64", t);
        assert!(event["correlation_id"].as_str().is_some(), "{}: correlation_id must be a string", t);
        assert!(event["source_module"].as_str().is_some(),  "{}: source_module must be a string", t);
        assert!(event["payload"].is_object(),               "{}: payload must be an object", t);
        assert_eq!(event["source_module"].as_str().unwrap(), "pm_structuring",
            "{}: source_module must be 'pm_structuring'", t);
        assert!(event["timestamp"].as_u64().unwrap() > 0,
            "{}: timestamp must be positive", t);
    }
}

#[test]
fn test_folder_fixture_event_types_are_schema_members() {
    let events = folder_events();
    for event in &events {
        let t = event["event_type"].as_str().unwrap();
        assert!(VALID_EVENT_TYPES.contains(&t),
            "Event type '{}' is not in the approved schema", t);
    }
}

#[test]
fn test_folder_fixture_no_failure_events() {
    let events = folder_events();
    for event in &events {
        let t = event["event_type"].as_str().unwrap();
        assert!(!t.contains("Failed"),
            "Happy path fixture must not contain failure event '{}'", t);
    }
}

#[test]
fn test_folder_fixture_scan_requested_precedes_completed() {
    let events = folder_events();
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    let req_pos = types.iter().position(|&t| t == "FolderScanRequested")
        .expect("FolderScanRequested must be present");
    let cmp_pos = types.iter().position(|&t| t == "FolderScanCompleted")
        .expect("FolderScanCompleted must be present");
    assert!(req_pos < cmp_pos, "FolderScanRequested must precede FolderScanCompleted");
}

#[test]
fn test_folder_fixture_scan_events_share_correlation_id() {
    let events = folder_events();
    let req = events.iter().find(|e| e["event_type"] == "FolderScanRequested").unwrap();
    let cmp = events.iter().find(|e| e["event_type"] == "FolderScanCompleted").unwrap();

    assert_eq!(
        req["correlation_id"].as_str().unwrap(),
        cmp["correlation_id"].as_str().unwrap(),
        "FolderScanRequested and FolderScanCompleted must share correlation_id"
    );
}

#[test]
fn test_folder_fixture_file_events_use_different_correlation_id_from_scan() {
    let events = folder_events();
    let scan_corr = events.iter()
        .find(|e| e["event_type"] == "FolderScanRequested")
        .unwrap()["correlation_id"].as_str().unwrap();

    let file_evts = file_events(&events);
    assert!(!file_evts.is_empty(), "Fixture must contain per-file events");

    let file_corr = file_evts[0]["correlation_id"].as_str().unwrap();
    assert_ne!(file_corr, scan_corr,
        "Per-file events must use a different correlation_id from the folder scan");
}

#[test]
fn test_folder_fixture_scan_requested_payload_shape() {
    let events = folder_events();
    let req = events.iter().find(|e| e["event_type"] == "FolderScanRequested").unwrap();
    let p = &req["payload"];

    assert!(p["folder_path"].as_str().is_some(), "folder_path must be a string");
    assert!(p["auto_confirm"].as_bool().is_some(), "auto_confirm must be a bool");
}

#[test]
fn test_folder_fixture_scan_completed_payload_shape() {
    let events = folder_events();
    let cmp = events.iter().find(|e| e["event_type"] == "FolderScanCompleted").unwrap();
    let p = &cmp["payload"];

    assert!(p["folder_path"].as_str().is_some(),   "folder_path must be a string");
    assert!(p["files_found"].as_u64().is_some(),    "files_found must be a u64");
    assert!(p["files_skipped"].as_u64().is_some(),  "files_skipped must be a u64");
    assert!(p["files_processed"].as_u64().is_some(),"files_processed must be a u64");

    let found     = p["files_found"].as_u64().unwrap();
    let skipped   = p["files_skipped"].as_u64().unwrap();
    let processed = p["files_processed"].as_u64().unwrap();
    assert_eq!(found, skipped + processed,
        "files_found must equal files_skipped + files_processed");
}

#[test]
fn test_folder_fixture_items_extracted_source_file_is_set() {
    let events = folder_events();
    let extracted = events.iter().find(|e| e["event_type"] == "ItemsExtracted").unwrap();

    let source_file = extracted["payload"]["source_file"].as_str();
    assert!(source_file.is_some(), "source_file must be a string in folder-mode ItemsExtracted");
    assert!(!source_file.unwrap().is_empty(), "source_file must not be empty");
}

#[test]
fn test_folder_fixture_scan_completed_files_processed_matches_extractions() {
    let events = folder_events();
    let cmp = events.iter().find(|e| e["event_type"] == "FolderScanCompleted").unwrap();
    let files_processed = cmp["payload"]["files_processed"].as_u64().unwrap();

    let extraction_count = events.iter()
        .filter(|e| e["event_type"] == "ItemsExtracted"
            && e["payload"]["source_file"].as_str().is_some())
        .count() as u64;

    assert_eq!(files_processed, extraction_count,
        "files_processed must equal the number of ItemsExtracted events with source_file set");
}

// ── R2: FolderNotFound failure fixture ────────────────────────────────────────

#[test]
fn test_folder_not_found_fixture_failure_reason() {
    let events = load_fixture("pm_structuring_folder_not_found.jsonl");
    let failure = events.iter()
        .find(|e| e["event_type"] == "ExtractionFailedFolderNotFound")
        .expect("ExtractionFailedFolderNotFound must be present");

    assert_eq!(failure["payload"]["failure_reason"].as_str().unwrap(), "folder_not_found");
    assert!(failure["payload"]["folder_path"].as_str().is_some(),
        "folder_path must be present in failure payload");
}

#[test]
fn test_folder_not_found_fixture_scan_requested_precedes_failure() {
    let events = load_fixture("pm_structuring_folder_not_found.jsonl");
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    let req_pos = types.iter().position(|&t| t == "FolderScanRequested")
        .expect("FolderScanRequested must be present");
    let fail_pos = types.iter().position(|&t| t == "ExtractionFailedFolderNotFound")
        .expect("ExtractionFailedFolderNotFound must be present");
    assert!(req_pos < fail_pos, "FolderScanRequested must precede ExtractionFailedFolderNotFound");
}

#[test]
fn test_folder_not_found_fixture_scan_events_share_correlation_id() {
    let events = load_fixture("pm_structuring_folder_not_found.jsonl");
    let req = events.iter().find(|e| e["event_type"] == "FolderScanRequested").unwrap();
    let fail = events.iter().find(|e| e["event_type"] == "ExtractionFailedFolderNotFound").unwrap();

    assert_eq!(
        req["correlation_id"].as_str().unwrap(),
        fail["correlation_id"].as_str().unwrap(),
        "FolderScanRequested and ExtractionFailedFolderNotFound must share correlation_id"
    );
}

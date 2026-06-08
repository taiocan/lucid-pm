//! Replay verification tests for demo.
//!
//! demo has a structural null event spine — it has no executable module and
//! emits no events. These tests verify:
//! 1. No events with source_module "demo" appear in the pre-populated record
//! 2. All events in the record conform to the required base field schema
//! 3. The record contains events from the expected source modules only
//!
//! The pre-populated demo record is the fixture — it is read directly from
//! demo/events/runtime_events.jsonl rather than a separate copy.

use serde_json::Value;
use std::path::Path;

fn demo_record() -> Vec<Value> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap()
        .parent().unwrap()
        .join("demo/events/runtime_events.jsonl");
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("demo/events/runtime_events.jsonl must exist"));
    content
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

/// Structural null spine: no events from source_module "demo" in the record.
#[test]
fn test_null_spine_no_demo_events_in_record() {
    let events = demo_record();
    let demo_events: Vec<&Value> = events.iter()
        .filter(|e| e["source_module"].as_str() == Some("demo"))
        .collect();
    assert!(
        demo_events.is_empty(),
        "structural null spine violated: {} demo event(s) found in record",
        demo_events.len()
    );
}

/// All events in the demo record have the required base fields.
#[test]
fn test_all_record_events_have_required_base_fields() {
    let events = demo_record();
    assert!(!events.is_empty(), "demo record must not be empty");
    for event in &events {
        let et = event["event_type"].as_str().unwrap_or("?");
        assert!(event["event_id"].is_string(),       "missing event_id in {et}");
        assert!(event["event_type"].is_string(),     "missing event_type in {et}");
        assert!(event["timestamp"].is_number(),      "missing timestamp in {et}");
        assert!(event["correlation_id"].is_string(), "missing correlation_id in {et}");
        assert!(event["source_module"].is_string(),  "missing source_module in {et}");
        assert!(event["payload"].is_object(),        "missing payload in {et}");
    }
}

/// Events in the record come only from known LucidPM feature modules.
#[test]
fn test_record_events_from_known_modules_only() {
    let events = demo_record();
    for event in &events {
        let module = event["source_module"].as_str().unwrap_or("");
        assert!(
            KNOWN_MODULES.contains(&module),
            "unexpected source_module '{module}' in demo record — \
             only known feature modules expected"
        );
    }
}

/// Extraction events (ItemsExtracted) are followed by ExtractionConfirmed
/// in the same correlation chain — basic sequence conformance.
#[test]
fn test_extraction_sequences_are_complete() {
    let events = demo_record();
    let extracted_chains: Vec<&str> = events.iter()
        .filter(|e| e["event_type"].as_str() == Some("ItemsExtracted"))
        .filter_map(|e| e["correlation_id"].as_str())
        .collect();
    for cid in &extracted_chains {
        let confirmed = events.iter().any(|e|
            e["correlation_id"].as_str() == Some(cid) &&
            e["event_type"].as_str() == Some("ExtractionConfirmed")
        );
        assert!(
            confirmed,
            "correlation chain {cid} has ItemsExtracted but no ExtractionConfirmed"
        );
    }
}

/// Incorporated sessions were previously extracted — ItemsIncorporated references
/// a session_id that has a corresponding ExtractionConfirmed event.
#[test]
fn test_incorporated_sessions_were_extracted() {
    let events = demo_record();
    let incorporated_sessions: Vec<&str> = events.iter()
        .filter(|e| e["event_type"].as_str() == Some("ItemsIncorporated"))
        .filter_map(|e| e["payload"]["session_id"].as_str())
        .collect();
    for session_id in &incorporated_sessions {
        let extracted = events.iter().any(|e|
            e["correlation_id"].as_str() == Some(session_id) &&
            e["event_type"].as_str() == Some("ExtractionConfirmed")
        );
        assert!(
            extracted,
            "ItemsIncorporated references session {session_id} \
             but no ExtractionConfirmed found for that session"
        );
    }
}

/// The record contains all expected item types — verifies demo content breadth.
#[test]
fn test_record_contains_all_expected_item_types() {
    let events = demo_record();
    let extracted_types: Vec<&str> = events.iter()
        .filter(|e| e["event_type"].as_str() == Some("ItemsExtracted"))
        .flat_map(|e| e["payload"]["items"].as_array().unwrap().iter()
            .filter_map(|i| i["item_type"].as_str()))
        .collect();
    for expected_type in &["milestone", "risk", "issue", "stakeholder"] {
        assert!(
            extracted_types.contains(expected_type),
            "demo record missing expected item type: {expected_type}"
        );
    }
    // Task is added via task_model, not extraction
    let has_task = events.iter().any(|e| e["event_type"].as_str() == Some("TaskAdded"));
    assert!(has_task, "demo record must contain at least one TaskAdded event");
}

/// The record contains typed links — verifies demo link content.
#[test]
fn test_record_contains_links() {
    let events = demo_record();
    let has_links = events.iter().any(|e| e["event_type"].as_str() == Some("ItemLinked"));
    assert!(has_links, "demo record must contain at least one ItemLinked event");
}

const KNOWN_MODULES: &[&str] = &[
    "pm_structuring",
    "project_state",
    "item_status",
    "item_links",
    "task_model",
    "logseq_export",
    "logseq_sync",
    "ontology_suggest",
    "priority_view",
    "report_export",
    "project_schema",
    "journal",
    "multi_project",
];

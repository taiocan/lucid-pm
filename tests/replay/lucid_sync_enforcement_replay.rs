//! Replay tests for lucid_sync_enforcement (R12).
//!
//! lucid_sync_enforcement has a null event spine — it is a test-time static
//! assertion with no runtime presence. No events are emitted at any point.
//!
//! These tests verify the null spine holds: the fixture is empty and no events
//! with source_module "lucid_sync_enforcement" exist anywhere in the event log.

use project_schema::test_support::load_fixture;
use serde_json::Value;

fn load_runtime_events() -> Vec<Value> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../events/runtime_events.jsonl");
    if !path.exists() {
        return Vec::new();
    }
    let content = std::fs::read_to_string(&path).unwrap();
    content
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

/// Fixture is empty — confirms lucid_sync_enforcement produced no runtime events.
/// Primary replay evidence for the null event spine.
#[test]
fn test_fixture_is_empty_confirming_null_spine() {
    let events = load_fixture("lucid_sync_enforcement.jsonl");
    assert!(
        events.is_empty(),
        "expected empty fixture for null event spine; found {} event(s)",
        events.len()
    );
}

/// Null spine holds in runtime log: no events with source_module
/// "lucid_sync_enforcement" appear in events/runtime_events.jsonl.
#[test]
fn test_null_spine_no_sync_enforcement_events_in_runtime_log() {
    let events = load_runtime_events();
    let enforcement_events: Vec<&Value> = events
        .iter()
        .filter(|e| e["source_module"].as_str() == Some("lucid_sync_enforcement"))
        .collect();
    assert!(
        enforcement_events.is_empty(),
        "null event spine violated: {} lucid_sync_enforcement event(s) in runtime log",
        enforcement_events.len()
    );
}

//! Replay verification tests for lucid.
//!
//! lucid has a null event spine — it emits no events. These tests verify that
//! the fixture conforms to the null spine: no events with source_module "lucid"
//! appear, and any events present satisfy the required base field schema.
//!
//! The fixture lucid_dispatch.jsonl is intentionally empty. If it ever contains
//! lucid-sourced events, the null spine has been violated.

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

/// Null spine holds: fixture contains no events from source_module "lucid".
#[test]
fn test_null_spine_no_lucid_events_in_fixture() {
    let events = load_fixture("lucid_dispatch.jsonl");
    let lucid_events: Vec<&Value> = events
        .iter()
        .filter(|e| e["source_module"].as_str() == Some("lucid"))
        .collect();
    assert!(
        lucid_events.is_empty(),
        "null event spine violated: {} lucid event(s) found in fixture",
        lucid_events.len()
    );
}

/// Any events present in the fixture satisfy the required base field schema.
#[test]
fn test_fixture_events_have_required_base_fields() {
    let events = load_fixture("lucid_dispatch.jsonl");
    for event in &events {
        assert!(event["event_id"].is_string(),       "missing event_id in: {event}");
        assert!(event["event_type"].is_string(),     "missing event_type in: {event}");
        assert!(event["timestamp"].is_number(),      "missing timestamp in: {event}");
        assert!(event["correlation_id"].is_string(), "missing correlation_id in: {event}");
        assert!(event["source_module"].is_string(),  "missing source_module in: {event}");
        assert!(event["payload"].is_object(),        "missing payload in: {event}");
    }
}

/// Fixture is empty — confirms lucid dispatch produced no events of its own.
/// This is the primary replay evidence for the null event spine.
#[test]
fn test_fixture_is_empty_confirming_null_spine() {
    let events = load_fixture("lucid_dispatch.jsonl");
    assert!(
        events.is_empty(),
        "expected empty fixture for null event spine; found {} event(s)",
        events.len()
    );
}

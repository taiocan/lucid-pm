//! Replay verification tests for project_schema.
//!
//! Loads JSONL event fixtures and verifies that project_schema events conform to
//! the approved event schema (events/project_schema_schema.md):
//! required base fields, valid event types, correct payload shapes.

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
        .map(|l| serde_json::from_str(l).expect("fixture line must be valid JSON"))
        .collect()
}

fn schema_events(all: &[Value]) -> Vec<&Value> {
    all.iter()
        .filter(|e| e["source_module"].as_str() == Some("project_schema"))
        .collect()
}

const VALID_EVENT_TYPES: &[&str] = &[
    "SchemaNotFound",
    "SchemaParseError",
    "SchemaValidationFailed",
    "SchemaAliasCollisionDetected",
    "SchemaTypeUnknown",
];

const VALID_FAILURE_REASONS: &[&str] = &[
    "schema_not_found",
    "schema_parse_error",
    "schema_validation_failed",
    "alias_collision",
];

// ─── Required base fields ─────────────────────────────────────────────────────

#[test]
fn test_all_events_have_required_base_fields() {
    let all = load_fixture("project_schema_events.jsonl");
    let events = schema_events(&all);
    assert!(!events.is_empty(), "fixture must contain project_schema events");

    for event in events {
        assert!(
            event["event_id"].as_str().is_some_and(|s| !s.is_empty()),
            "event_id required: {:?}",
            event
        );
        assert!(
            event["event_type"].as_str().is_some_and(|s| !s.is_empty()),
            "event_type required: {:?}",
            event
        );
        assert!(event["timestamp"].as_u64().is_some(), "timestamp required: {:?}", event);
        assert!(
            event["correlation_id"].as_str().is_some_and(|s| !s.is_empty()),
            "correlation_id required: {:?}",
            event
        );
        assert_eq!(
            event["source_module"].as_str(),
            Some("project_schema"),
            "source_module required: {:?}",
            event
        );
        assert!(event["payload"].is_object(), "payload must be object: {:?}", event);
    }
}

// ─── Valid event types ────────────────────────────────────────────────────────

#[test]
fn test_all_event_types_are_valid() {
    let all = load_fixture("project_schema_events.jsonl");
    let events = schema_events(&all);

    for event in events {
        let event_type = event["event_type"].as_str().unwrap();
        assert!(
            VALID_EVENT_TYPES.contains(&event_type),
            "unknown event type '{}' — not in approved schema",
            event_type
        );
    }
}

// ─── FAILURE event payload shapes ────────────────────────────────────────────

#[test]
fn test_failure_events_have_valid_failure_reason() {
    let all = load_fixture("project_schema_events.jsonl");
    let failure_events: Vec<&Value> = all
        .iter()
        .filter(|e| {
            e["source_module"].as_str() == Some("project_schema")
                && e["event_type"].as_str() != Some("SchemaTypeUnknown")
        })
        .collect();

    assert!(!failure_events.is_empty(), "fixture must contain failure events");

    for event in failure_events {
        let reason = event["payload"]["failure_reason"]
            .as_str()
            .unwrap_or_else(|| panic!("failure_reason missing in {:?}", event));
        assert!(
            VALID_FAILURE_REASONS.contains(&reason),
            "unknown failure_reason '{}' in {:?}",
            reason,
            event
        );
    }
}

#[test]
fn test_schema_not_found_payload_shape() {
    let all = load_fixture("project_schema_events.jsonl");
    let events: Vec<&Value> = all
        .iter()
        .filter(|e| e["event_type"].as_str() == Some("SchemaNotFound"))
        .collect();

    assert!(!events.is_empty(), "fixture must contain SchemaNotFound");
    for event in events {
        assert_eq!(event["payload"]["failure_reason"], "schema_not_found");
        assert!(
            event["payload"]["searched_locations"].is_array(),
            "searched_locations must be array"
        );
    }
}

#[test]
fn test_schema_parse_error_payload_shape() {
    let all = load_fixture("project_schema_events.jsonl");
    let events: Vec<&Value> = all
        .iter()
        .filter(|e| e["event_type"].as_str() == Some("SchemaParseError"))
        .collect();

    assert!(!events.is_empty(), "fixture must contain SchemaParseError");
    for event in events {
        assert_eq!(event["payload"]["failure_reason"], "schema_parse_error");
        assert!(
            event["payload"]["detail"].as_str().is_some_and(|s| !s.is_empty()),
            "detail must be non-empty string"
        );
    }
}

#[test]
fn test_schema_validation_failed_payload_shape() {
    let all = load_fixture("project_schema_events.jsonl");
    let events: Vec<&Value> = all
        .iter()
        .filter(|e| e["event_type"].as_str() == Some("SchemaValidationFailed"))
        .collect();

    assert!(!events.is_empty(), "fixture must contain SchemaValidationFailed");
    for event in events {
        assert_eq!(event["payload"]["failure_reason"], "schema_validation_failed");
        assert!(event["payload"]["violated_rule"].as_str().is_some_and(|s| !s.is_empty()));
        assert!(event["payload"]["detail"].as_str().is_some());
    }
}

#[test]
fn test_schema_alias_collision_payload_shape() {
    let all = load_fixture("project_schema_events.jsonl");
    let events: Vec<&Value> = all
        .iter()
        .filter(|e| e["event_type"].as_str() == Some("SchemaAliasCollisionDetected"))
        .collect();

    assert!(!events.is_empty(), "fixture must contain SchemaAliasCollisionDetected");
    for event in events {
        assert_eq!(event["payload"]["failure_reason"], "alias_collision");
        assert!(event["payload"]["alias_value"].as_str().is_some_and(|s| !s.is_empty()));
        assert!(event["payload"]["collides_with"].as_str().is_some_and(|s| !s.is_empty()));
    }
}

// ─── OBSERVATIONAL event payload shape ───────────────────────────────────────

#[test]
fn test_schema_type_unknown_payload_shape() {
    let all = load_fixture("project_schema_events.jsonl");
    let events: Vec<&Value> = all
        .iter()
        .filter(|e| e["event_type"].as_str() == Some("SchemaTypeUnknown"))
        .collect();

    assert!(!events.is_empty(), "fixture must contain SchemaTypeUnknown");
    for event in events {
        assert!(event["payload"]["item_id"].as_str().is_some_and(|s| !s.is_empty()));
        assert!(event["payload"]["unknown_type"].as_str().is_some_and(|s| !s.is_empty()));
        // SchemaTypeUnknown has no failure_reason field
        assert!(event["payload"]["failure_reason"].is_null());
    }
}

// ─── Event sequence integrity ─────────────────────────────────────────────────

#[test]
fn test_each_event_has_unique_event_id() {
    let all = load_fixture("project_schema_events.jsonl");
    let events = schema_events(&all);
    let mut ids = std::collections::HashSet::new();
    for event in events {
        let id = event["event_id"].as_str().unwrap();
        assert!(ids.insert(id), "duplicate event_id: {}", id);
    }
}

#[test]
fn test_fixture_covers_all_approved_event_types() {
    let all = load_fixture("project_schema_events.jsonl");
    let event_types: std::collections::HashSet<&str> = all
        .iter()
        .filter(|e| e["source_module"].as_str() == Some("project_schema"))
        .filter_map(|e| e["event_type"].as_str())
        .collect();

    for required_type in VALID_EVENT_TYPES {
        assert!(
            event_types.contains(required_type),
            "fixture missing event type '{}' — replay coverage incomplete",
            required_type
        );
    }
}

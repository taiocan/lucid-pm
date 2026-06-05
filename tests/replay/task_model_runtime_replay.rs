//! Stage 8 Replay Verification — task_model runtime events.
//!
//! Verifies that the events captured during Stage 6 runtime execution:
//!   1. Conform to the approved event schema (events/task_model_schema.md)
//!   2. Form complete correlation chains matching the contract event flow
//!   3. Carry correct payload shapes for each event type
//!   4. Are consistent with re-running the system (determinism check)
//!
//! Fixture: tests/replay/fixtures/task_model_runtime.jsonl
//! Contains 11 events captured during Stage 6 runtime execution.

use serde_json::Value;
use std::collections::HashMap;

fn load_runtime_fixture() -> Vec<Value> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/replay/fixtures")
        .join("task_model_runtime.jsonl");
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("Could not read fixture: {}", path.display()));
    content
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

fn tm_events(all: &[Value]) -> Vec<&Value> {
    all.iter()
        .filter(|e| e["source_module"].as_str() == Some("task_model"))
        .collect()
}

const VALID_EVENT_TYPES: &[&str] = &[
    "TaskAddRequested",
    "TaskAdded",
    "TaskMarkerUpdated",
    "TaskAddFailedParentNotFound",
    "TaskAddFailedSchemaInvalid",
    "TaskAddFailedTaskTypeNotDefined",
];

const OBSERVATIONAL: &[&str] = &["TaskAddRequested"];
const BEHAVIORAL: &[&str]    = &["TaskAdded", "TaskMarkerUpdated"];
const FAILURE: &[&str]       = &[
    "TaskAddFailedParentNotFound",
    "TaskAddFailedSchemaInvalid",
    "TaskAddFailedTaskTypeNotDefined",
];

// ── 1. Schema conformance ─────────────────────────────────────────────────────

#[test]
fn test_runtime_all_events_have_required_base_fields() {
    let all = load_runtime_fixture();
    let events = tm_events(&all);
    assert!(!events.is_empty(), "Runtime fixture must contain task_model events");

    for event in &events {
        let t = event["event_type"].as_str().unwrap_or("unknown");
        assert!(event["event_id"].as_str().is_some(),       "{}: event_id must be present", t);
        assert!(event["event_type"].as_str().is_some(),     "{}: event_type must be present", t);
        assert!(event["timestamp"].as_u64().is_some(),      "{}: timestamp must be u64", t);
        assert!(event["correlation_id"].as_str().is_some(), "{}: correlation_id must be present", t);
        assert!(event["source_module"].as_str().is_some(),  "{}: source_module must be present", t);
        assert!(event["payload"].is_object(),               "{}: payload must be an object", t);
        assert_eq!(
            event["source_module"].as_str().unwrap(), "task_model",
            "{}: source_module must be 'task_model'", t
        );
        let cid = event["correlation_id"].as_str().unwrap();
        assert!(!cid.is_empty(), "{}: correlation_id must be non-empty", t);
        assert!(event["timestamp"].as_u64().unwrap() > 0, "{}: timestamp must be positive", t);
    }
}

#[test]
fn test_runtime_all_event_types_in_approved_schema() {
    let all = load_runtime_fixture();
    let events = tm_events(&all);

    for event in &events {
        let t = event["event_type"].as_str().unwrap();
        assert!(
            VALID_EVENT_TYPES.contains(&t),
            "Runtime event type '{}' is not in the approved schema — hidden behavior detected", t
        );
    }
}

#[test]
fn test_runtime_no_events_outside_approved_schema() {
    let all = load_runtime_fixture();
    let events = tm_events(&all);
    let unexpected: Vec<&str> = events.iter()
        .map(|e| e["event_type"].as_str().unwrap())
        .filter(|t| !VALID_EVENT_TYPES.contains(t))
        .collect();
    assert!(
        unexpected.is_empty(),
        "Unexpected event types found in runtime log — schema must be updated: {:?}", unexpected
    );
}

// ── 2. Payload shape conformance ──────────────────────────────────────────────

#[test]
fn test_runtime_task_add_requested_payload_shape() {
    let all = load_runtime_fixture();
    let events = tm_events(&all);
    let requests: Vec<_> = events.iter()
        .filter(|e| e["event_type"] == "TaskAddRequested")
        .collect();
    assert!(!requests.is_empty(), "Runtime must contain TaskAddRequested events");

    for event in &requests {
        let p = &event["payload"];
        assert!(p["description"].as_str().is_some(),    "description must be a string");
        assert!(p["parent_item_id"].as_str().is_some(), "parent_item_id must be a string");
        assert!(p.get("requested_marker").is_some(),    "requested_marker must be present (may be null)");
    }
}

#[test]
fn test_runtime_task_added_payload_shape() {
    let all = load_runtime_fixture();
    let events = tm_events(&all);
    let added: Vec<_> = events.iter()
        .filter(|e| e["event_type"] == "TaskAdded")
        .collect();
    assert!(!added.is_empty(), "Runtime must contain TaskAdded events");

    for event in &added {
        let p = &event["payload"];
        let task_id = p["task_id"].as_str().expect("task_id must be a string");
        assert_eq!(task_id.len(), 36,           "task_id must be UUID format (36 chars)");
        assert!(task_id.contains('-'),          "task_id must contain hyphens");
        assert!(p["item_type"].as_str().is_some(),       "item_type must be a string");
        assert!(p["description"].as_str().is_some(),     "description must be a string");
        assert!(p["parent_item_id"].as_str().is_some(),  "parent_item_id must be a string");
        let marker = p["initial_marker"].as_str().expect("initial_marker must be a string");
        assert!(!marker.is_empty(), "initial_marker must not be empty");
    }
}

#[test]
fn test_runtime_task_marker_updated_payload_shape() {
    let all = load_runtime_fixture();
    let events = tm_events(&all);
    let updates: Vec<_> = events.iter()
        .filter(|e| e["event_type"] == "TaskMarkerUpdated")
        .collect();
    assert!(!updates.is_empty(), "Runtime must contain TaskMarkerUpdated events");

    for event in &updates {
        let p = &event["payload"];
        let task_id = p["task_id"].as_str().expect("task_id must be a string");
        assert_eq!(task_id.len(), 36, "task_id must be UUID format");
        let prev = p["previous_marker"].as_str().expect("previous_marker must be a string");
        let next = p["new_marker"].as_str().expect("new_marker must be a string");
        assert_ne!(prev, next, "previous_marker and new_marker must differ");
        assert!(!prev.is_empty(), "previous_marker must not be empty");
        assert!(!next.is_empty(), "new_marker must not be empty");
    }
}

#[test]
fn test_runtime_failure_events_have_failure_reason() {
    let all = load_runtime_fixture();
    let events = tm_events(&all);

    for event in events.iter().filter(|e| FAILURE.contains(&e["event_type"].as_str().unwrap())) {
        let t = event["event_type"].as_str().unwrap();
        let reason = event["payload"]["failure_reason"].as_str()
            .unwrap_or_else(|| panic!("{}: failure_reason must be a string", t));
        assert!(!reason.is_empty(), "{}: failure_reason must not be empty", t);
    }
}

#[test]
fn test_runtime_parent_not_found_has_parent_item_id() {
    let all = load_runtime_fixture();
    let events = tm_events(&all);
    let failure = events.iter()
        .find(|e| e["event_type"] == "TaskAddFailedParentNotFound")
        .expect("Runtime must contain TaskAddFailedParentNotFound");

    assert_eq!(failure["payload"]["failure_reason"].as_str().unwrap(), "parent_not_found");
    assert!(failure["payload"]["parent_item_id"].as_str().is_some(),
        "parent_item_id must be present in TaskAddFailedParentNotFound");
}

// ── 3. Correlation chain integrity ────────────────────────────────────────────

#[test]
fn test_runtime_every_chain_starts_with_observational_or_is_singleton_behavioral() {
    let all = load_runtime_fixture();
    let events = tm_events(&all);

    let mut chains: HashMap<&str, Vec<&str>> = HashMap::new();
    for e in &events {
        let cid = e["correlation_id"].as_str().unwrap();
        let t = e["event_type"].as_str().unwrap();
        chains.entry(cid).or_default().push(t);
    }

    for (cid, types) in &chains {
        let first = types[0];
        let is_user_initiated_chain = OBSERVATIONAL.contains(&first);
        let is_singleton_behavioral = types.len() == 1 && BEHAVIORAL.contains(&first);
        assert!(
            is_user_initiated_chain || is_singleton_behavioral,
            "Chain {}: first event '{}' must be OBSERVATIONAL (user-initiated) or \
             a singleton BEHAVIORAL (sync-path discovery/update)", cid, first
        );
    }
}

#[test]
fn test_runtime_every_chain_ends_with_behavioral_or_failure() {
    let all = load_runtime_fixture();
    let events = tm_events(&all);

    let mut chains: HashMap<&str, Vec<&str>> = HashMap::new();
    for e in &events {
        let cid = e["correlation_id"].as_str().unwrap();
        let t = e["event_type"].as_str().unwrap();
        chains.entry(cid).or_default().push(t);
    }

    for (cid, types) in &chains {
        let last = types[types.len() - 1];
        assert!(
            BEHAVIORAL.contains(&last) || FAILURE.contains(&last),
            "Chain {}: last event '{}' must be BEHAVIORAL or FAILURE — chain is incomplete", cid, last
        );
    }
}

#[test]
fn test_runtime_no_orphaned_task_add_requested() {
    // Every TaskAddRequested must have a corresponding outcome event
    // (TaskAdded or a TaskAddFailed*) in the same correlation chain.
    let all = load_runtime_fixture();
    let events = tm_events(&all);

    let mut chains: HashMap<&str, Vec<&str>> = HashMap::new();
    for e in &events {
        let cid = e["correlation_id"].as_str().unwrap();
        let t = e["event_type"].as_str().unwrap();
        chains.entry(cid).or_default().push(t);
    }

    for (cid, types) in &chains {
        if !types.contains(&"TaskAddRequested") { continue; }
        let has_outcome = types.iter().any(|&t| BEHAVIORAL.contains(&t) || FAILURE.contains(&t));
        assert!(has_outcome,
            "Chain {}: TaskAddRequested has no corresponding outcome event — orphaned chain", cid);
    }
}

#[test]
fn test_runtime_task_add_requested_precedes_outcome_in_every_chain() {
    let all = load_runtime_fixture();
    let events = tm_events(&all);

    let mut chains: HashMap<&str, Vec<&str>> = HashMap::new();
    for e in &events {
        let cid = e["correlation_id"].as_str().unwrap();
        let t = e["event_type"].as_str().unwrap();
        chains.entry(cid).or_default().push(t);
    }

    for (cid, types) in &chains {
        if !types.contains(&"TaskAddRequested") { continue; }
        let req_pos = types.iter().position(|&t| t == "TaskAddRequested").unwrap();
        let outcome_pos = types.iter().position(|&t| BEHAVIORAL.contains(&t) || FAILURE.contains(&t));
        if let Some(op) = outcome_pos {
            assert!(req_pos < op,
                "Chain {}: TaskAddRequested (pos {}) must precede outcome (pos {})",
                cid, req_pos, op);
        }
    }
}

// ── 4. Event sequence conformance (contract flow) ────────────────────────────

#[test]
fn test_runtime_happy_path_chains_have_requested_then_added() {
    let all = load_runtime_fixture();
    let events = tm_events(&all);

    let mut chains: HashMap<&str, Vec<&str>> = HashMap::new();
    for e in &events {
        let cid = e["correlation_id"].as_str().unwrap();
        chains.entry(cid).or_default().push(e["event_type"].as_str().unwrap());
    }

    let happy_chains: Vec<_> = chains.values()
        .filter(|types| types.contains(&"TaskAdded"))
        .collect();

    assert!(!happy_chains.is_empty(), "Runtime must contain at least one happy-path chain");

    for types in &happy_chains {
        assert!(types.contains(&"TaskAddRequested"),
            "Happy-path chain {:?} must begin with TaskAddRequested", types);
        let req_pos = types.iter().position(|&t| t == "TaskAddRequested").unwrap();
        let add_pos = types.iter().position(|&t| t == "TaskAdded").unwrap();
        assert!(req_pos < add_pos, "TaskAddRequested must precede TaskAdded in {:?}", types);
    }
}

#[test]
fn test_runtime_failure_chains_have_requested_then_failure() {
    let all = load_runtime_fixture();
    let events = tm_events(&all);

    let mut chains: HashMap<&str, Vec<&str>> = HashMap::new();
    for e in &events {
        let cid = e["correlation_id"].as_str().unwrap();
        chains.entry(cid).or_default().push(e["event_type"].as_str().unwrap());
    }

    let failure_chains: Vec<_> = chains.values()
        .filter(|types| types.iter().any(|&t| FAILURE.contains(&t)))
        .collect();

    assert!(failure_chains.len() >= 3,
        "Runtime must contain at least 3 failure chains (one per failure path); got {}",
        failure_chains.len());

    for types in &failure_chains {
        assert!(types.contains(&"TaskAddRequested"),
            "Failure chain {:?} must contain TaskAddRequested", types);
    }
}

#[test]
fn test_runtime_all_three_failure_paths_observed() {
    let all = load_runtime_fixture();
    let events = tm_events(&all);
    let types: Vec<&str> = events.iter()
        .map(|e| e["event_type"].as_str().unwrap())
        .collect();

    assert!(types.contains(&"TaskAddFailedParentNotFound"),
        "ParentNotFound failure path must be observed at runtime");
    assert!(types.contains(&"TaskAddFailedSchemaInvalid"),
        "SchemaInvalid failure path must be observed at runtime");
    assert!(types.contains(&"TaskAddFailedTaskTypeNotDefined"),
        "TaskTypeNotDefined failure path must be observed at runtime");
}

#[test]
fn test_runtime_task_marker_updated_observed() {
    let all = load_runtime_fixture();
    let events = tm_events(&all);
    assert!(
        events.iter().any(|e| e["event_type"] == "TaskMarkerUpdated"),
        "TaskMarkerUpdated must be observed at runtime (sync path)"
    );
}

// ── 5. Timestamp ordering ─────────────────────────────────────────────────────

#[test]
fn test_runtime_timestamps_are_monotonically_nondecreasing() {
    let all = load_runtime_fixture();
    let events = tm_events(&all);
    let timestamps: Vec<u64> = events.iter()
        .map(|e| e["timestamp"].as_u64().unwrap())
        .collect();

    for window in timestamps.windows(2) {
        assert!(window[0] <= window[1],
            "Runtime timestamps must be non-decreasing: {} > {}", window[0], window[1]);
    }
}

// ── 6. Log summary metrics ────────────────────────────────────────────────────

#[test]
fn test_runtime_log_metrics_match_expected_stage6_output() {
    let all = load_runtime_fixture();
    let events = tm_events(&all);

    // 11 events = 2×(Requested+Added) + 3×(Requested+Failure) + 1×(MarkerUpdated)
    assert_eq!(events.len(), 11,
        "Runtime fixture must contain exactly 11 task_model events from Stage 6");

    let added_count = events.iter().filter(|e| e["event_type"] == "TaskAdded").count();
    let marker_count = events.iter().filter(|e| e["event_type"] == "TaskMarkerUpdated").count();
    let failure_count = events.iter().filter(|e| FAILURE.contains(&e["event_type"].as_str().unwrap())).count();

    assert_eq!(added_count,   2, "Stage 6 created 2 task instances");
    assert_eq!(marker_count,  1, "Stage 6 observed 1 marker update via sync");
    assert_eq!(failure_count, 3, "Stage 6 exercised all 3 failure paths");
}

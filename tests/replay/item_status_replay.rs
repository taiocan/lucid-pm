//! Replay verification tests for item_status.
//!
//! Loads JSONL event fixtures and verifies that item_status events conform to
//! the approved event schema (events/item_status_schema.md): required fields,
//! valid event types, correct payload shapes, and valid event sequences.

use project_schema::test_support::load_fixture;
use serde_json::Value;

/// Filter to only item_status events.
fn is_events(all: &[Value]) -> Vec<&Value> {
    all.iter()
        .filter(|e| e["source_module"].as_str() == Some("item_status"))
        .collect()
}

const VALID_EVENT_TYPES: &[&str] = &[
    "StatusUpdateRequested",
    "ItemStatusUpdated",
    "StatusUpdateFailedItemNotFound",
    "StatusUpdateFailedInvalidStatus",
    "PriorityUpdateRequested",
    "ItemPriorityUpdated",
    "PriorityUpdateFailedItemNotFound",
    "PriorityUpdateFailedInvalidValue",
    "ItemStatusQueried",
    "ItemStatusReturned",
    "ItemStatusQueryFailedItemNotFound",
    // R5: non-failure observational event for stale recorded status
    "ItemStatusUnrecognized",
];

const VALID_PRIORITIES: &[&str] = &["high", "medium", "low"];

const VALID_STATUSES_BY_TYPE: &[(&str, &[&str])] = &[
    ("task",        &["todo", "doing", "done", "waiting", "cancelled"]),
    ("milestone",   &["pending", "achieved", "missed"]),
    ("risk",        &["open", "mitigated", "accepted", "closed"]),
    ("issue",       &["open", "in_progress", "resolved", "closed"]),
    ("stakeholder", &["active", "inactive"]),
];

fn valid_statuses_for(item_type: &str) -> &'static [&'static str] {
    VALID_STATUSES_BY_TYPE
        .iter()
        .find(|(t, _)| *t == item_type)
        .map(|(_, v)| *v)
        .unwrap_or(&[])
}

// ── Schema conformance ────────────────────────────────────────────────────────

#[test]
fn test_happy_path_all_is_events_have_required_base_fields() {
    let all = load_fixture("item_status_happy_path.jsonl");
    let events = is_events(&all);
    assert!(!events.is_empty(), "Fixture must contain item_status events");

    for event in &events {
        let t = event["event_type"].as_str().unwrap_or("unknown");
        assert!(event["event_id"].as_str().is_some(),       "{}: event_id must be a string", t);
        assert!(event["event_type"].as_str().is_some(),     "{}: event_type must be a string", t);
        assert!(event["timestamp"].as_u64().is_some(),      "{}: timestamp must be a u64", t);
        assert!(event["correlation_id"].as_str().is_some(), "{}: correlation_id must be a string", t);
        assert!(event["source_module"].as_str().is_some(),  "{}: source_module must be a string", t);
        assert!(event["payload"].is_object(),               "{}: payload must be an object", t);
        assert_eq!(
            event["source_module"].as_str().unwrap(), "item_status",
            "{}: source_module must be 'item_status'", t
        );
        assert!(event["timestamp"].as_u64().unwrap() > 0, "{}: timestamp must be positive", t);
    }
}

#[test]
fn test_happy_path_event_types_are_schema_members() {
    let all = load_fixture("item_status_happy_path.jsonl");
    let events = is_events(&all);
    for event in &events {
        let t = event["event_type"].as_str().unwrap();
        assert!(VALID_EVENT_TYPES.contains(&t),
            "Event type '{}' is not in the approved item_status schema", t);
    }
}

#[test]
fn test_happy_path_no_failure_events() {
    let all = load_fixture("item_status_happy_path.jsonl");
    let events = is_events(&all);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    for t in &types {
        assert!(!t.contains("Failed"),
            "Happy path must not contain failure event '{}'", t);
    }
}

// ── Sequence conformance ──────────────────────────────────────────────────────

#[test]
fn test_happy_path_status_update_requested_before_updated() {
    let all = load_fixture("item_status_happy_path.jsonl");
    let events = is_events(&all);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"StatusUpdateRequested"), "Fixture must contain StatusUpdateRequested");
    assert!(types.contains(&"ItemStatusUpdated"),     "Fixture must contain ItemStatusUpdated");

    let req_pos = types.iter().position(|&t| t == "StatusUpdateRequested").unwrap();
    let upd_pos = types.iter().position(|&t| t == "ItemStatusUpdated").unwrap();
    assert!(req_pos < upd_pos, "StatusUpdateRequested must precede ItemStatusUpdated");
}

#[test]
fn test_happy_path_priority_update_requested_before_updated() {
    let all = load_fixture("item_status_happy_path.jsonl");
    let events = is_events(&all);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"PriorityUpdateRequested"), "Fixture must contain PriorityUpdateRequested");
    assert!(types.contains(&"ItemPriorityUpdated"),     "Fixture must contain ItemPriorityUpdated");

    let req_pos = types.iter().position(|&t| t == "PriorityUpdateRequested").unwrap();
    let upd_pos = types.iter().position(|&t| t == "ItemPriorityUpdated").unwrap();
    assert!(req_pos < upd_pos, "PriorityUpdateRequested must precede ItemPriorityUpdated");
}

#[test]
fn test_happy_path_item_status_queried_before_returned() {
    let all = load_fixture("item_status_happy_path.jsonl");
    let events = is_events(&all);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"ItemStatusQueried"),  "Fixture must contain ItemStatusQueried");
    assert!(types.contains(&"ItemStatusReturned"), "Fixture must contain ItemStatusReturned");

    let q_pos = types.iter().position(|&t| t == "ItemStatusQueried").unwrap();
    let r_pos = types.iter().position(|&t| t == "ItemStatusReturned").unwrap();
    assert!(q_pos < r_pos, "ItemStatusQueried must precede ItemStatusReturned");
}

#[test]
fn test_happy_path_each_command_has_distinct_correlation_id() {
    let all = load_fixture("item_status_happy_path.jsonl");
    let events = is_events(&all);

    let status_cid = events.iter()
        .find(|e| e["event_type"] == "StatusUpdateRequested")
        .map(|e| e["correlation_id"].as_str().unwrap())
        .expect("StatusUpdateRequested must be present");

    let priority_cid = events.iter()
        .find(|e| e["event_type"] == "PriorityUpdateRequested")
        .map(|e| e["correlation_id"].as_str().unwrap())
        .expect("PriorityUpdateRequested must be present");

    let get_cid = events.iter()
        .find(|e| e["event_type"] == "ItemStatusQueried")
        .map(|e| e["correlation_id"].as_str().unwrap())
        .expect("ItemStatusQueried must be present");

    assert_ne!(status_cid, priority_cid, "set-status and set-priority must have different correlation_ids");
    assert_ne!(status_cid, get_cid,      "set-status and get must have different correlation_ids");
    assert_ne!(priority_cid, get_cid,    "set-priority and get must have different correlation_ids");
}

// ── Correlation ID consistency within each invocation ─────────────────────────

#[test]
fn test_happy_path_set_status_invocation_shares_correlation_id() {
    let all = load_fixture("item_status_happy_path.jsonl");
    let events = is_events(&all);

    let req = events.iter().find(|e| e["event_type"] == "StatusUpdateRequested").unwrap();
    let upd = events.iter().find(|e| e["event_type"] == "ItemStatusUpdated").unwrap();

    assert_eq!(
        req["correlation_id"].as_str().unwrap(),
        upd["correlation_id"].as_str().unwrap(),
        "StatusUpdateRequested and ItemStatusUpdated must share correlation_id"
    );
}

#[test]
fn test_happy_path_get_invocation_shares_correlation_id() {
    let all = load_fixture("item_status_happy_path.jsonl");
    let events = is_events(&all);

    let queried  = events.iter().find(|e| e["event_type"] == "ItemStatusQueried").unwrap();
    let returned = events.iter().find(|e| e["event_type"] == "ItemStatusReturned").unwrap();

    assert_eq!(
        queried["correlation_id"].as_str().unwrap(),
        returned["correlation_id"].as_str().unwrap(),
        "ItemStatusQueried and ItemStatusReturned must share correlation_id"
    );
}

// ── Payload shape conformance ─────────────────────────────────────────────────

#[test]
fn test_happy_path_item_status_updated_payload() {
    let all = load_fixture("item_status_happy_path.jsonl");
    let events = is_events(&all);
    let event = events.iter().find(|e| e["event_type"] == "ItemStatusUpdated").unwrap();
    let p = &event["payload"];

    assert!(p["item_id"].as_str().is_some(),   "item_id must be a string");
    assert!(p["item_type"].as_str().is_some(),  "item_type must be a string");
    assert!(p["new_status"].as_str().is_some(), "new_status must be a string");
    assert!(p.get("previous_status").is_some(), "previous_status field must be present");
}

#[test]
fn test_happy_path_item_priority_updated_payload() {
    let all = load_fixture("item_status_happy_path.jsonl");
    let events = is_events(&all);
    let event = events.iter().find(|e| e["event_type"] == "ItemPriorityUpdated").unwrap();
    let p = &event["payload"];

    assert!(p["item_id"].as_str().is_some(),    "item_id must be a string");
    assert!(p["new_priority"].as_str().is_some(),"new_priority must be a string");
    assert!(p.get("previous_priority").is_some(),"previous_priority field must be present");

    let priority = p["new_priority"].as_str().unwrap();
    assert!(VALID_PRIORITIES.contains(&priority),
        "new_priority '{}' must be one of: high, medium, low", priority);
}

#[test]
fn test_happy_path_item_status_returned_payload() {
    let all = load_fixture("item_status_happy_path.jsonl");
    let events = is_events(&all);
    let event = events.iter().find(|e| e["event_type"] == "ItemStatusReturned").unwrap();
    let p = &event["payload"];

    assert!(p["item_id"].as_str().is_some(),   "item_id must be a string");
    assert!(p["item_type"].as_str().is_some(),  "item_type must be a string");
    assert!(p.get("current_status").is_some(),  "current_status field must be present");
    assert!(p.get("current_priority").is_some(),"current_priority field must be present");
    // R5 payload amendment: status_source indicates which resolution step produced the value
    assert!(p.get("status_source").is_some(),   "status_source field must be present (may be null)");
}

#[test]
fn test_happy_path_set_status_item_id_matches_get_item_id() {
    let all = load_fixture("item_status_happy_path.jsonl");
    let events = is_events(&all);

    let updated  = events.iter().find(|e| e["event_type"] == "ItemStatusUpdated").unwrap();
    let returned = events.iter().find(|e| e["event_type"] == "ItemStatusReturned").unwrap();

    assert_eq!(
        updated["payload"]["item_id"].as_str().unwrap(),
        returned["payload"]["item_id"].as_str().unwrap(),
        "ItemStatusUpdated and ItemStatusReturned must reference the same item_id"
    );
}

// ── Proposed status/priority in ItemsExtracted (R1) ──────────────────────────

#[test]
fn test_happy_path_extracted_items_have_proposed_fields() {
    let all = load_fixture("item_status_happy_path.jsonl");
    let extracted = all.iter()
        .find(|e| e["source_module"].as_str() == Some("pm_structuring")
               && e["event_type"].as_str() == Some("ItemsExtracted"))
        .expect("ItemsExtracted must be present in fixture");

    let items = extracted["payload"]["items"].as_array().unwrap();
    assert!(!items.is_empty(), "ItemsExtracted must contain items");
    for item in items {
        let t = item["item_type"].as_str().unwrap_or("unknown");
        assert!(item.get("proposed_status").is_some(),
            "{}: proposed_status field must be present (may be null)", t);
        assert!(item.get("proposed_priority").is_some(),
            "{}: proposed_priority field must be present (may be null)", t);
    }
}

#[test]
fn test_happy_path_extracted_proposed_status_valid_for_type() {
    let all = load_fixture("item_status_happy_path.jsonl");
    let extracted = all.iter()
        .find(|e| e["source_module"].as_str() == Some("pm_structuring")
               && e["event_type"].as_str() == Some("ItemsExtracted"))
        .unwrap();

    for item in extracted["payload"]["items"].as_array().unwrap() {
        if let Some(status) = item["proposed_status"].as_str() {
            let item_type = item["item_type"].as_str().unwrap();
            assert!(valid_statuses_for(item_type).contains(&status),
                "proposed_status '{}' is not valid for item_type '{}'", status, item_type);
        }
    }
}

#[test]
fn test_happy_path_extracted_proposed_priority_valid() {
    let all = load_fixture("item_status_happy_path.jsonl");
    let extracted = all.iter()
        .find(|e| e["source_module"].as_str() == Some("pm_structuring")
               && e["event_type"].as_str() == Some("ItemsExtracted"))
        .unwrap();

    for item in extracted["payload"]["items"].as_array().unwrap() {
        if let Some(priority) = item["proposed_priority"].as_str() {
            assert!(VALID_PRIORITIES.contains(&priority),
                "proposed_priority '{}' must be one of: high, medium, low", priority);
        }
    }
}

#[test]
fn test_happy_path_risk_get_returns_proposed_status_as_fallback() {
    // Risk item had no explicit set-status; current_status in ItemStatusReturned
    // must come from proposed_status in ItemsExtracted (open).
    let all = load_fixture("item_status_happy_path.jsonl");
    let events = is_events(&all);

    // Find the ItemStatusReturned for the risk item (second get invocation)
    let risk_item_id = "e5f6a7b8-c9d0-1234-ef01-234567890123";
    let returned = events.iter()
        .find(|e| e["event_type"].as_str() == Some("ItemStatusReturned")
               && e["payload"]["item_id"].as_str() == Some(risk_item_id))
        .expect("ItemStatusReturned for risk item must be present");

    assert_eq!(returned["payload"]["current_status"].as_str().unwrap(), "open",
        "current_status must be 'open' (from proposed fallback — no explicit set-status for risk)");
    assert_eq!(returned["payload"]["current_priority"].as_str().unwrap(), "high",
        "current_priority must be 'high' (from explicit ItemPriorityUpdated)");
}

// ── R5: status_source field conformance ──────────────────────────────────────

#[test]
fn test_happy_path_task_item_status_returned_has_explicit_source() {
    // Task item had explicit set-status → status_source must be "explicit"
    let all = load_fixture("item_status_happy_path.jsonl");
    let events = is_events(&all);

    let task_item_id = "c3d4e5f6-a7b8-9012-cdef-012345678901";
    let returned = events.iter()
        .find(|e| e["event_type"].as_str() == Some("ItemStatusReturned")
               && e["payload"]["item_id"].as_str() == Some(task_item_id))
        .expect("ItemStatusReturned for task item must be present");

    assert_eq!(returned["payload"]["status_source"].as_str().unwrap(), "explicit",
        "status_source must be 'explicit' for the task item which had an explicit set-status");
}

#[test]
fn test_happy_path_risk_item_status_returned_has_proposed_source() {
    // Risk item had no set-status; proposed_status from extraction → status_source must be "proposed"
    let all = load_fixture("item_status_happy_path.jsonl");
    let events = is_events(&all);

    let risk_item_id = "e5f6a7b8-c9d0-1234-ef01-234567890123";
    let returned = events.iter()
        .find(|e| e["event_type"].as_str() == Some("ItemStatusReturned")
               && e["payload"]["item_id"].as_str() == Some(risk_item_id))
        .expect("ItemStatusReturned for risk item must be present");

    assert_eq!(returned["payload"]["status_source"].as_str().unwrap(), "proposed",
        "status_source must be 'proposed' for the risk item which had only a proposed_status");
}

#[test]
fn test_happy_path_status_source_values_are_valid() {
    let all = load_fixture("item_status_happy_path.jsonl");
    let events = is_events(&all);
    const VALID_SOURCES: &[&str] = &["explicit", "marker_derived", "proposed"];

    for event in events.iter().filter(|e| e["event_type"] == "ItemStatusReturned") {
        let source = &event["payload"]["status_source"];
        if !source.is_null() {
            let s = source.as_str().expect("status_source must be a string or null");
            assert!(VALID_SOURCES.contains(&s),
                "status_source '{}' must be one of: explicit, marker_derived, proposed", s);
        }
    }
}

// ── R5: ItemStatusUnrecognized conformance ────────────────────────────────────

#[test]
fn test_happy_path_item_status_unrecognized_payload_shape() {
    // This test passes vacuously if no ItemStatusUnrecognized events are present.
    // When present, they must conform to the schema.
    let all = load_fixture("item_status_happy_path.jsonl");
    let events = is_events(&all);

    for event in events.iter().filter(|e| e["event_type"] == "ItemStatusUnrecognized") {
        let p = &event["payload"];
        assert!(p["item_id"].as_str().is_some(),      "item_id must be a string");
        assert!(p["item_type"].as_str().is_some(),     "item_type must be a string");
        assert!(p["recorded_status"].as_str().is_some(),"recorded_status must be a string");
    }
}

#[test]
fn test_happy_path_unrecognized_precedes_returned_when_present() {
    // If ItemStatusUnrecognized is in the fixture, it must precede the
    // ItemStatusReturned event that shares its correlation_id.
    let all = load_fixture("item_status_happy_path.jsonl");
    let events = is_events(&all);

    for unrecognized in events.iter().filter(|e| e["event_type"] == "ItemStatusUnrecognized") {
        let cid = unrecognized["correlation_id"].as_str().unwrap();
        let unrecognized_ts = unrecognized["timestamp"].as_u64().unwrap();
        let returned_ts = events.iter()
            .find(|e| e["event_type"] == "ItemStatusReturned"
                   && e["correlation_id"].as_str() == Some(cid))
            .map(|e| e["timestamp"].as_u64().unwrap())
            .expect("ItemStatusReturned must follow ItemStatusUnrecognized");

        assert!(unrecognized_ts <= returned_ts,
            "ItemStatusUnrecognized must be emitted before ItemStatusReturned");
    }
}

//! Replay verification tests for priority_view.
//!
//! Loads JSONL event fixtures and verifies that priority_view events conform to
//! the approved event schema (events/priority_view_schema.md): required fields,
//! valid event types, correct payload shapes, and valid event sequences.

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

fn pv_events(all: &[Value]) -> Vec<&Value> {
    all.iter()
        .filter(|e| e["source_module"].as_str() == Some("priority_view"))
        .collect()
}

const VALID_EVENT_TYPES: &[&str] = &[
    "PriorityViewRequested",
    "PriorityViewReturned",
    "PriorityViewFailedEmptyRecord",
    "PriorityViewFailedInvalidFilter",
];

const VALID_PRIORITIES: &[&str] = &["high", "medium", "low"];

const VALID_STATUSES: &[&str] = &[
    "todo", "doing", "done", "waiting", "cancelled",
    "pending", "achieved", "missed",
    "open", "in_progress", "resolved", "mitigated", "accepted", "closed",
    "active", "inactive",
];

const VALID_ITEM_TYPES: &[&str] = &["task", "milestone", "risk", "issue", "stakeholder"];

// ── Schema conformance ────────────────────────────────────────────────────────

#[test]
fn test_happy_path_all_pv_events_have_required_base_fields() {
    let all = load_fixture("priority_view_happy_path.jsonl");
    let events = pv_events(&all);
    assert!(!events.is_empty(), "Fixture must contain priority_view events");

    for event in &events {
        let t = event["event_type"].as_str().unwrap_or("unknown");
        assert!(event["event_id"].as_str().is_some(),       "{}: event_id must be a string", t);
        assert!(event["event_type"].as_str().is_some(),     "{}: event_type must be a string", t);
        assert!(event["timestamp"].as_u64().is_some(),      "{}: timestamp must be a u64", t);
        assert!(event["correlation_id"].as_str().is_some(), "{}: correlation_id must be a string", t);
        assert!(event["source_module"].as_str().is_some(),  "{}: source_module must be a string", t);
        assert!(event["payload"].is_object(),               "{}: payload must be an object", t);
        assert_eq!(
            event["source_module"].as_str().unwrap(), "priority_view",
            "{}: source_module must be 'priority_view'", t
        );
        assert!(event["timestamp"].as_u64().unwrap() > 0, "{}: timestamp must be positive", t);
    }
}

#[test]
fn test_happy_path_event_types_are_schema_members() {
    let all = load_fixture("priority_view_happy_path.jsonl");
    let events = pv_events(&all);
    for event in &events {
        let t = event["event_type"].as_str().unwrap();
        assert!(VALID_EVENT_TYPES.contains(&t),
            "Event type '{}' is not in the approved priority_view schema", t);
    }
}

#[test]
fn test_happy_path_no_failure_events() {
    let all = load_fixture("priority_view_happy_path.jsonl");
    let events = pv_events(&all);
    for event in &events {
        let t = event["event_type"].as_str().unwrap();
        assert!(!t.contains("Failed"),
            "Happy path fixture must not contain failure event '{}'", t);
    }
}

// ── Sequence conformance ──────────────────────────────────────────────────────

#[test]
fn test_happy_path_requested_before_returned() {
    let all = load_fixture("priority_view_happy_path.jsonl");
    let events = pv_events(&all);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"PriorityViewRequested"), "Fixture must contain PriorityViewRequested");
    assert!(types.contains(&"PriorityViewReturned"),  "Fixture must contain PriorityViewReturned");

    let req_pos = types.iter().position(|&t| t == "PriorityViewRequested").unwrap();
    let ret_pos = types.iter().position(|&t| t == "PriorityViewReturned").unwrap();
    assert!(req_pos < ret_pos, "PriorityViewRequested must precede PriorityViewReturned");
}

#[test]
fn test_happy_path_requested_and_returned_share_correlation_id() {
    let all = load_fixture("priority_view_happy_path.jsonl");
    let events = pv_events(&all);

    let req = events.iter().find(|e| e["event_type"] == "PriorityViewRequested").unwrap();
    let ret = events.iter().find(|e| e["event_type"] == "PriorityViewReturned").unwrap();

    assert_eq!(
        req["correlation_id"].as_str().unwrap(),
        ret["correlation_id"].as_str().unwrap(),
        "PriorityViewRequested and PriorityViewReturned must share correlation_id"
    );
}

// ── Payload shape conformance ─────────────────────────────────────────────────

#[test]
fn test_happy_path_requested_payload_shape() {
    let all = load_fixture("priority_view_happy_path.jsonl");
    let events = pv_events(&all);
    let req = events.iter().find(|e| e["event_type"] == "PriorityViewRequested").unwrap();
    let p = &req["payload"];

    assert!(p.get("filter_type").is_some(),     "filter_type field must be present");
    assert!(p.get("filter_status").is_some(),   "filter_status field must be present");
    assert!(p.get("filter_priority").is_some(), "filter_priority field must be present");
}

#[test]
fn test_happy_path_returned_payload_shape() {
    let all = load_fixture("priority_view_happy_path.jsonl");
    let events = pv_events(&all);
    let ret = events.iter().find(|e| e["event_type"] == "PriorityViewReturned").unwrap();
    let p = &ret["payload"];

    assert!(p["item_count"].as_u64().is_some(),  "item_count must be a non-negative integer");
    assert!(p["filters_applied"].is_object(),    "filters_applied must be an object");
    assert!(p["items"].is_array(),               "items must be an array");
    assert_eq!(
        p["item_count"].as_u64().unwrap() as usize,
        p["items"].as_array().unwrap().len(),
        "item_count must equal items array length"
    );
}

#[test]
fn test_happy_path_filters_applied_has_three_keys() {
    let all = load_fixture("priority_view_happy_path.jsonl");
    let events = pv_events(&all);
    let ret = events.iter().find(|e| e["event_type"] == "PriorityViewReturned").unwrap();
    let fa = &ret["payload"]["filters_applied"];

    assert!(fa.get("type").is_some(),     "filters_applied.type must be present");
    assert!(fa.get("status").is_some(),   "filters_applied.status must be present");
    assert!(fa.get("priority").is_some(), "filters_applied.priority must be present");
}

#[test]
fn test_happy_path_each_item_in_returned_has_required_fields() {
    let all = load_fixture("priority_view_happy_path.jsonl");
    let events = pv_events(&all);
    let ret = events.iter().find(|e| e["event_type"] == "PriorityViewReturned").unwrap();
    let items = ret["payload"]["items"].as_array().unwrap();

    assert!(!items.is_empty(), "Fixture happy path must contain at least one item");

    for item in items {
        assert!(item["item_id"].as_str().is_some(),    "item must have item_id (string)");
        assert!(item["item_type"].as_str().is_some(),  "item must have item_type (string)");
        assert!(item["description"].as_str().is_some(),"item must have description (string)");
        assert!(item["session_id"].as_str().is_some(), "item must have session_id (string)");
        assert!(item.get("priority").is_some(),        "item must have priority field (string or null)");
        assert!(item.get("status").is_some(),          "item must have status field (string or null)");
    }
}

#[test]
fn test_happy_path_item_types_are_valid() {
    let all = load_fixture("priority_view_happy_path.jsonl");
    let events = pv_events(&all);
    let ret = events.iter().find(|e| e["event_type"] == "PriorityViewReturned").unwrap();

    for item in ret["payload"]["items"].as_array().unwrap() {
        let t = item["item_type"].as_str().unwrap();
        assert!(VALID_ITEM_TYPES.contains(&t),
            "item_type '{}' is not a recognised item type", t);
    }
}

#[test]
fn test_happy_path_item_priorities_are_valid_when_set() {
    let all = load_fixture("priority_view_happy_path.jsonl");
    let events = pv_events(&all);
    let ret = events.iter().find(|e| e["event_type"] == "PriorityViewReturned").unwrap();

    for item in ret["payload"]["items"].as_array().unwrap() {
        if let Some(p) = item["priority"].as_str() {
            assert!(VALID_PRIORITIES.contains(&p),
                "priority '{}' is not a recognised priority value", p);
        }
    }
}

#[test]
fn test_happy_path_item_statuses_are_valid_when_set() {
    let all = load_fixture("priority_view_happy_path.jsonl");
    let events = pv_events(&all);
    let ret = events.iter().find(|e| e["event_type"] == "PriorityViewReturned").unwrap();

    for item in ret["payload"]["items"].as_array().unwrap() {
        if let Some(s) = item["status"].as_str() {
            assert!(VALID_STATUSES.contains(&s),
                "status '{}' is not a recognised status value", s);
        }
    }
}

// ── Sort order conformance ────────────────────────────────────────────────────

// ── R7: SchemaInvalid fixture (FP1) ──────────────────────────────────────────

fn schema_invalid_events() -> Vec<Value> {
    load_fixture("priority_view_schema_invalid.jsonl")
}

#[test]
fn test_schema_invalid_fixture_has_no_priority_view_events() {
    let events = schema_invalid_events();
    assert!(!events.is_empty(), "Fixture must not be empty");
    let pv = pv_events(&events);
    assert!(pv.is_empty(),
        "No priority_view events must be present when vocabulary is invalid");
}

#[test]
fn test_schema_invalid_fixture_contains_project_schema_failure_event() {
    let events = schema_invalid_events();
    let failure = events.iter().find(|e| {
        e["source_module"].as_str() == Some("project_schema")
            && matches!(e["event_type"].as_str(),
                Some("SchemaParseError") | Some("SchemaValidationFailed"))
    });
    assert!(failure.is_some(),
        "Fixture must contain a SchemaParseError or SchemaValidationFailed from project_schema");
}

#[test]
fn test_schema_invalid_fixture_failure_event_has_required_fields() {
    let events = schema_invalid_events();
    let failure = events.iter()
        .find(|e| e["source_module"].as_str() == Some("project_schema")
            && matches!(e["event_type"].as_str(),
                Some("SchemaParseError") | Some("SchemaValidationFailed")))
        .unwrap();

    assert!(failure["event_id"].as_str().is_some(),       "event_id must be a string");
    assert!(failure["timestamp"].as_u64().is_some(),      "timestamp must be a u64");
    assert!(failure["correlation_id"].as_str().is_some(), "correlation_id must be a string");
    assert!(failure["payload"].is_object(),               "payload must be an object");
}

// ── R7: Unrecognized item exclusion fixture (HP3) ────────────────────────────

fn unrecognized_excluded_events() -> Vec<Value> {
    load_fixture("priority_view_unrecognized_excluded.jsonl")
}

#[test]
fn test_unrecognized_excluded_fixture_has_schema_type_unknown() {
    let events = unrecognized_excluded_events();
    let unknown = events.iter().find(|e| {
        e["source_module"].as_str() == Some("project_schema")
            && e["event_type"].as_str() == Some("SchemaTypeUnknown")
    });
    assert!(unknown.is_some(),
        "Fixture must contain a SchemaTypeUnknown event for the excluded item");
}

#[test]
fn test_unrecognized_excluded_fixture_schema_type_unknown_payload() {
    let events = unrecognized_excluded_events();
    let unknown = events.iter()
        .find(|e| e["event_type"].as_str() == Some("SchemaTypeUnknown"))
        .unwrap();

    assert!(unknown["payload"]["item_id"].as_str().is_some(),
        "SchemaTypeUnknown.item_id must be a string");
    assert!(unknown["payload"]["unknown_type"].as_str().is_some(),
        "SchemaTypeUnknown.unknown_type must be a string");
    assert_eq!(unknown["payload"]["unknown_type"].as_str().unwrap(), "widget",
        "unknown_type must record the unrecognized type string as stored in the event log");
}

#[test]
fn test_unrecognized_excluded_fixture_command_completes_successfully() {
    let events = unrecognized_excluded_events();
    let pv = pv_events(&events);
    let types: Vec<&str> = pv.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"PriorityViewRequested"),
        "PriorityViewRequested must be emitted");
    assert!(types.contains(&"PriorityViewReturned"),
        "PriorityViewReturned must be emitted — exclusion is not a failure");
    assert!(!types.contains(&"PriorityViewFailedEmptyRecord"),
        "EmptyRecord must NOT fire when recognized-type items remain");
}

#[test]
fn test_unrecognized_excluded_fixture_excluded_item_absent_from_returned() {
    let events = unrecognized_excluded_events();
    let pv = pv_events(&events);
    let returned = pv.iter().find(|e| e["event_type"] == "PriorityViewReturned").unwrap();
    let items = returned["payload"]["items"].as_array().unwrap();

    // "widget" item must not appear in the result
    assert!(items.iter().all(|i| i["item_type"].as_str() != Some("widget")),
        "The unrecognized-type 'widget' item must be absent from PriorityViewReturned items");
}

#[test]
fn test_unrecognized_excluded_fixture_schema_type_unknown_shares_correlation_id() {
    let events = unrecognized_excluded_events();
    let pv = pv_events(&events);
    let returned = pv.iter()
        .find(|e| e["event_type"] == "PriorityViewReturned")
        .unwrap();
    let pv_corr = returned["correlation_id"].as_str().unwrap();

    let unknown = events.iter()
        .find(|e| e["event_type"].as_str() == Some("SchemaTypeUnknown"))
        .unwrap();
    assert_eq!(unknown["correlation_id"].as_str().unwrap(), pv_corr,
        "SchemaTypeUnknown must share the priority_view invocation's correlation_id");
}

#[test]
fn test_unrecognized_excluded_fixture_priority_view_event_types_are_schema_members() {
    let events = unrecognized_excluded_events();
    let pv = pv_events(&events);
    for event in &pv {
        let t = event["event_type"].as_str().unwrap();
        assert!(VALID_EVENT_TYPES.contains(&t),
            "priority_view event type '{}' is not in the approved schema", t);
    }
}

fn priority_rank(p: Option<&str>) -> u8 {
    match p {
        Some("high")   => 1,
        Some("medium") => 2,
        Some("low")    => 3,
        _              => 4,
    }
}

fn status_rank(s: Option<&str>) -> u8 {
    match s {
        Some("doing") | Some("in_progress") | Some("active")      => 1,
        Some("todo")  | Some("open")        | Some("pending")     => 2,
        Some("waiting")                                            => 3,
        Some("done")      | Some("achieved")   | Some("resolved")
        | Some("mitigated") | Some("accepted") | Some("cancelled")
        | Some("missed")  | Some("closed")     | Some("inactive") => 4,
        _                                                          => 5,
    }
}

#[test]
fn test_happy_path_items_are_sorted_by_priority_then_status() {
    let all = load_fixture("priority_view_happy_path.jsonl");
    let events = pv_events(&all);
    let ret = events.iter().find(|e| e["event_type"] == "PriorityViewReturned").unwrap();
    let items = ret["payload"]["items"].as_array().unwrap();

    let mut last_pri_rank = 0u8;
    let mut last_sta_rank = 0u8;

    for item in items {
        let pri = priority_rank(item["priority"].as_str());
        let sta = status_rank(item["status"].as_str());

        if pri == last_pri_rank {
            assert!(sta >= last_sta_rank,
                "within the same priority level, status rank must be non-decreasing");
        } else {
            assert!(pri >= last_pri_rank,
                "priority rank must be non-decreasing across items");
        }

        last_pri_rank = pri;
        last_sta_rank = if pri == last_pri_rank { sta } else { sta };
    }
}

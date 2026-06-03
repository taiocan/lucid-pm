//! Replay verification tests for report_export.
//!
//! Loads JSONL event fixtures and verifies that report_export events conform to
//! the approved event schema (events/report_export_schema.md): required fields,
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

fn re_events(all: &[Value]) -> Vec<&Value> {
    all.iter()
        .filter(|e| e["source_module"].as_str() == Some("report_export"))
        .collect()
}

const VALID_EVENT_TYPES: &[&str] = &[
    "ReportRequested",
    "ReportGenerated",
    "ReportFailedEmptyRecord",
    "ReportFailedInvalidType",
    "ReportFailedOutputNotFound",
];

const VALID_REPORT_TYPES: &[&str] = &["weekly", "risk-register", "stakeholders", "full"];

// ── Schema conformance ────────────────────────────────────────────────────────

#[test]
fn test_happy_path_all_re_events_have_required_base_fields() {
    let all = load_fixture("report_export_happy_path.jsonl");
    let events = re_events(&all);
    assert!(!events.is_empty(), "Fixture must contain report_export events");

    for event in &events {
        let t = event["event_type"].as_str().unwrap_or("unknown");
        assert!(event["event_id"].as_str().is_some(),       "{}: event_id must be a string", t);
        assert!(event["event_type"].as_str().is_some(),     "{}: event_type must be a string", t);
        assert!(event["timestamp"].as_u64().is_some(),      "{}: timestamp must be a u64", t);
        assert!(event["correlation_id"].as_str().is_some(), "{}: correlation_id must be a string", t);
        assert!(event["source_module"].as_str().is_some(),  "{}: source_module must be a string", t);
        assert!(event["payload"].is_object(),               "{}: payload must be an object", t);
        assert_eq!(
            event["source_module"].as_str().unwrap(), "report_export",
            "{}: source_module must be 'report_export'", t
        );
        assert!(event["timestamp"].as_u64().unwrap() > 0, "{}: timestamp must be positive", t);
    }
}

#[test]
fn test_happy_path_event_types_are_schema_members() {
    let all = load_fixture("report_export_happy_path.jsonl");
    let events = re_events(&all);
    for event in &events {
        let t = event["event_type"].as_str().unwrap();
        assert!(VALID_EVENT_TYPES.contains(&t),
            "Event type '{}' is not in the approved report_export schema", t);
    }
}

#[test]
fn test_happy_path_no_failure_events() {
    let all = load_fixture("report_export_happy_path.jsonl");
    let events = re_events(&all);
    for event in &events {
        let t = event["event_type"].as_str().unwrap();
        assert!(!t.contains("Failed"),
            "Happy path fixture must not contain failure event '{}'", t);
    }
}

// ── Sequence conformance ──────────────────────────────────────────────────────

#[test]
fn test_happy_path_requested_before_generated() {
    let all = load_fixture("report_export_happy_path.jsonl");
    let events = re_events(&all);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"ReportRequested"), "Fixture must contain ReportRequested");
    assert!(types.contains(&"ReportGenerated"), "Fixture must contain ReportGenerated");

    let req_pos = types.iter().position(|&t| t == "ReportRequested").unwrap();
    let gen_pos = types.iter().position(|&t| t == "ReportGenerated").unwrap();
    assert!(req_pos < gen_pos, "ReportRequested must precede ReportGenerated");
}

#[test]
fn test_happy_path_requested_and_generated_share_correlation_id() {
    let all = load_fixture("report_export_happy_path.jsonl");
    let events = re_events(&all);

    let req      = events.iter().find(|e| e["event_type"] == "ReportRequested").unwrap();
    let generated = events.iter().find(|e| e["event_type"] == "ReportGenerated").unwrap();

    assert_eq!(
        req["correlation_id"].as_str().unwrap(),
        generated["correlation_id"].as_str().unwrap(),
        "ReportRequested and ReportGenerated must share correlation_id"
    );
}

// ── Payload shape conformance ─────────────────────────────────────────────────

#[test]
fn test_happy_path_requested_payload_shape() {
    let all = load_fixture("report_export_happy_path.jsonl");
    let events = re_events(&all);
    let req = events.iter().find(|e| e["event_type"] == "ReportRequested").unwrap();
    let p = &req["payload"];

    assert!(p["report_type"].as_str().is_some(), "report_type must be a string");
    assert!(VALID_REPORT_TYPES.contains(&p["report_type"].as_str().unwrap()),
        "report_type in ReportRequested must be a valid report type");
    assert!(p.get("graph_path").is_some(), "graph_path field must be present");
}

#[test]
fn test_happy_path_generated_payload_shape() {
    let all = load_fixture("report_export_happy_path.jsonl");
    let events = re_events(&all);
    let generated = events.iter().find(|e| e["event_type"] == "ReportGenerated").unwrap();
    let p = &generated["payload"];

    assert!(p["report_type"].as_str().is_some(),        "report_type must be a string");
    assert!(p["output_destination"].as_str().is_some(), "output_destination must be a string");
    assert!(p.get("report_file").is_some(),             "report_file field must be present");
    assert!(p["item_count"].as_u64().is_some(),         "item_count must be a non-negative integer");
    assert!(p["generated_at"].as_u64().is_some(),       "generated_at must be a timestamp");
}

#[test]
fn test_happy_path_generated_report_type_is_valid() {
    let all = load_fixture("report_export_happy_path.jsonl");
    let events = re_events(&all);
    let generated = events.iter().find(|e| e["event_type"] == "ReportGenerated").unwrap();

    let rt = generated["payload"]["report_type"].as_str().unwrap();
    assert!(VALID_REPORT_TYPES.contains(&rt),
        "report_type '{}' in ReportGenerated is not a valid report type", rt);
}

#[test]
fn test_happy_path_generated_report_type_matches_requested() {
    let all = load_fixture("report_export_happy_path.jsonl");
    let events = re_events(&all);

    let req       = events.iter().find(|e| e["event_type"] == "ReportRequested").unwrap();
    let generated = events.iter().find(|e| e["event_type"] == "ReportGenerated").unwrap();

    assert_eq!(
        req["payload"]["report_type"].as_str().unwrap(),
        generated["payload"]["report_type"].as_str().unwrap(),
        "report_type in ReportGenerated must match the type in ReportRequested"
    );
}

#[test]
fn test_happy_path_stdout_destination_has_null_report_file() {
    let all = load_fixture("report_export_happy_path.jsonl");
    let events = re_events(&all);
    let generated = events.iter().find(|e| e["event_type"] == "ReportGenerated").unwrap();
    let p = &generated["payload"];

    if p["output_destination"].as_str() == Some("stdout") {
        assert!(p["report_file"].is_null(),
            "report_file must be null when output_destination is stdout");
    }
}

#[test]
fn test_happy_path_item_count_is_non_negative() {
    let all = load_fixture("report_export_happy_path.jsonl");
    let events = re_events(&all);
    let generated = events.iter().find(|e| e["event_type"] == "ReportGenerated").unwrap();

    let count = generated["payload"]["item_count"].as_u64().unwrap();
    assert!(count > 0, "happy path item_count must be > 0");
}

#[test]
fn test_happy_path_generated_at_is_positive_timestamp() {
    let all = load_fixture("report_export_happy_path.jsonl");
    let events = re_events(&all);
    let generated = events.iter().find(|e| e["event_type"] == "ReportGenerated").unwrap();

    let ts = generated["payload"]["generated_at"].as_u64().unwrap();
    assert!(ts > 0, "generated_at must be a positive timestamp");
}

// ─────────────────────────────────────────────────────────────────────────────
// R8: schema-driven vocabulary — replay verification
// ─────────────────────────────────────────────────────────────────────────────

// ── Schema-invalid fixture: vocabulary load failure ───────────────────────────

#[test]
fn test_schema_invalid_no_report_requested_in_stream() {
    let all = load_fixture("report_export_schema_invalid.jsonl");
    let events = re_events(&all);
    assert!(!events.iter().any(|e| e["event_type"] == "ReportRequested"),
        "ReportRequested must NOT appear in a schema-invalid event stream");
}

#[test]
fn test_schema_invalid_contains_project_schema_failure_event() {
    let all = load_fixture("report_export_schema_invalid.jsonl");
    let schema_events: Vec<&Value> = all.iter()
        .filter(|e| e["source_module"].as_str() == Some("project_schema"))
        .collect();
    assert!(!schema_events.is_empty(),
        "Fixture must contain a project_schema failure event");

    for e in &schema_events {
        let t = e["event_type"].as_str().unwrap_or("unknown");
        assert!(
            matches!(t, "SchemaParseError" | "SchemaValidationFailed"),
            "project_schema event type must be SchemaParseError or SchemaValidationFailed; got '{}'", t
        );
    }
}

#[test]
fn test_schema_invalid_project_schema_event_has_required_base_fields() {
    let all = load_fixture("report_export_schema_invalid.jsonl");
    let schema_events: Vec<&Value> = all.iter()
        .filter(|e| e["source_module"].as_str() == Some("project_schema"))
        .collect();
    assert!(!schema_events.is_empty());

    for e in &schema_events {
        let t = e["event_type"].as_str().unwrap_or("unknown");
        assert!(e["event_id"].as_str().is_some(),       "{}: event_id must be a string", t);
        assert!(e["event_type"].as_str().is_some(),     "{}: event_type must be a string", t);
        assert!(e["timestamp"].as_u64().is_some(),      "{}: timestamp must be a u64", t);
        assert!(e["correlation_id"].as_str().is_some(), "{}: correlation_id must be a string", t);
        assert!(e["source_module"].as_str().is_some(),  "{}: source_module must be a string", t);
        assert!(e["payload"].is_object(),               "{}: payload must be an object", t);
    }
}

// ── Unrecognized-excluded fixture: SchemaTypeUnknown per excluded item ─────────

#[test]
fn test_unrecognized_excluded_schema_type_unknown_has_item_id_and_type() {
    let all = load_fixture("report_export_unrecognized_excluded.jsonl");
    let unknown_events: Vec<&Value> = all.iter()
        .filter(|e| e["source_module"].as_str() == Some("project_schema")
            && e["event_type"].as_str() == Some("SchemaTypeUnknown"))
        .collect();
    assert!(!unknown_events.is_empty(), "Fixture must contain SchemaTypeUnknown events");

    for e in &unknown_events {
        assert!(e["payload"]["item_id"].as_str().is_some(),
            "SchemaTypeUnknown must carry item_id");
        assert!(e["payload"]["unknown_type"].as_str().is_some(),
            "SchemaTypeUnknown must carry unknown_type");
    }
}

#[test]
fn test_unrecognized_excluded_schema_type_unknown_shares_correlation_id_with_report() {
    let all = load_fixture("report_export_unrecognized_excluded.jsonl");
    let report_cid = re_events(&all).iter()
        .find(|e| e["event_type"] == "ReportRequested")
        .and_then(|e| e["correlation_id"].as_str())
        .expect("Fixture must contain ReportRequested");

    let unknown_events: Vec<&Value> = all.iter()
        .filter(|e| e["source_module"].as_str() == Some("project_schema")
            && e["event_type"].as_str() == Some("SchemaTypeUnknown"))
        .collect();
    assert!(!unknown_events.is_empty());

    for e in &unknown_events {
        assert_eq!(e["correlation_id"].as_str().unwrap(), report_cid,
            "SchemaTypeUnknown must share the correlation_id of the report invocation");
    }
}

#[test]
fn test_unrecognized_excluded_report_generated_not_empty_record() {
    let all = load_fixture("report_export_unrecognized_excluded.jsonl");
    let events = re_events(&all);
    assert!(events.iter().any(|e| e["event_type"] == "ReportGenerated"),
        "ReportGenerated must be present — exclusion is not a failure");
    assert!(!events.iter().any(|e| e["event_type"] == "ReportFailedEmptyRecord"),
        "ReportFailedEmptyRecord must NOT be present when exclusion causes empty content");
}

#[test]
fn test_unrecognized_excluded_item_count_does_not_include_excluded_items() {
    let all = load_fixture("report_export_unrecognized_excluded.jsonl");
    let generated = re_events(&all).into_iter()
        .find(|e| e["event_type"] == "ReportGenerated")
        .expect("ReportGenerated must be present");

    let item_count = generated["payload"]["item_count"].as_u64().unwrap();
    let excluded_count = all.iter()
        .filter(|e| e["source_module"].as_str() == Some("project_schema")
            && e["event_type"].as_str() == Some("SchemaTypeUnknown"))
        .count() as u64;
    let extracted_count = all.iter()
        .find(|e| e["source_module"].as_str() == Some("pm_structuring")
            && e["event_type"].as_str() == Some("ItemsExtracted"))
        .and_then(|e| e["payload"]["item_count"].as_u64())
        .unwrap_or(0);

    assert!(excluded_count > 0, "Fixture must have at least one excluded item");
    assert!(item_count + excluded_count <= extracted_count,
        "item_count ({}) + excluded ({}) must not exceed total extracted ({})",
        item_count, excluded_count, extracted_count);
}

#[test]
fn test_unrecognized_excluded_sequence_requested_before_generated() {
    let all = load_fixture("report_export_unrecognized_excluded.jsonl");
    let events = re_events(&all);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    let req_pos = types.iter().position(|&t| t == "ReportRequested").unwrap();
    let gen_pos = types.iter().position(|&t| t == "ReportGenerated").unwrap();
    assert!(req_pos < gen_pos, "ReportRequested must precede ReportGenerated");
}

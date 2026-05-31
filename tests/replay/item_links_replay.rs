//! Replay verification tests for item_links.
//!
//! Loads JSONL event fixtures and verifies that item_links events conform to
//! the approved event schema (events/item_links_schema.md): required fields,
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

fn il_events(all: &[Value]) -> Vec<&Value> {
    all.iter()
        .filter(|e| e["source_module"].as_str() == Some("item_links"))
        .collect()
}

const VALID_EVENT_TYPES: &[&str] = &[
    "LinkAddRequested",
    "LinkRemoveRequested",
    "LinkListRequested",
    "ItemLinked",
    "ItemUnlinked",
    "LinkListReturned",
    "LinkFailedItemNotFound",
    "LinkFailedInvalidLinkType",
    "LinkFailedDuplicateLink",
    "LinkFailedLinkNotFound",
];

const VALID_LINK_TYPES: &[&str] = &[
    "blocks", "affects", "assigned_to", "mitigated_by", "escalates_to", "related_to",
];

const VALID_ITEM_TYPES: &[&str] = &["task", "milestone", "risk", "issue", "stakeholder"];

const VALID_DIRECTIONS: &[&str] = &["outgoing", "incoming"];

// ── Schema conformance ────────────────────────────────────────────────────────

#[test]
fn test_happy_path_all_il_events_have_required_base_fields() {
    let all = load_fixture("item_links_happy_path.jsonl");
    let events = il_events(&all);
    assert!(!events.is_empty(), "Fixture must contain item_links events");

    for event in &events {
        let t = event["event_type"].as_str().unwrap_or("unknown");
        assert!(event["event_id"].as_str().is_some(),       "{}: event_id must be a string", t);
        assert!(event["event_type"].as_str().is_some(),     "{}: event_type must be a string", t);
        assert!(event["timestamp"].as_u64().is_some(),      "{}: timestamp must be a u64", t);
        assert!(event["correlation_id"].as_str().is_some(), "{}: correlation_id must be a string", t);
        assert!(event["source_module"].as_str().is_some(),  "{}: source_module must be a string", t);
        assert!(event["payload"].is_object(),               "{}: payload must be an object", t);
        assert_eq!(event["source_module"].as_str().unwrap(), "item_links",
            "{}: source_module must be 'item_links'", t);
        assert!(event["timestamp"].as_u64().unwrap() > 0,
            "{}: timestamp must be positive", t);
    }
}

#[test]
fn test_happy_path_event_types_are_schema_members() {
    let all = load_fixture("item_links_happy_path.jsonl");
    let events = il_events(&all);
    for event in &events {
        let t = event["event_type"].as_str().unwrap();
        assert!(VALID_EVENT_TYPES.contains(&t),
            "Event type '{}' is not in the approved item_links schema", t);
    }
}

#[test]
fn test_happy_path_no_failure_events() {
    let all = load_fixture("item_links_happy_path.jsonl");
    let events = il_events(&all);
    for event in &events {
        let t = event["event_type"].as_str().unwrap();
        assert!(!t.starts_with("LinkFailed"),
            "Happy path fixture must not contain failure event '{}'", t);
    }
}

// ── Sequence conformance ──────────────────────────────────────────────────────

#[test]
fn test_happy_path_add_requested_before_linked() {
    let all = load_fixture("item_links_happy_path.jsonl");
    let events = il_events(&all);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"LinkAddRequested"), "Fixture must contain LinkAddRequested");
    assert!(types.contains(&"ItemLinked"),       "Fixture must contain ItemLinked");

    let req_pos = types.iter().position(|&t| t == "LinkAddRequested").unwrap();
    let lnk_pos = types.iter().position(|&t| t == "ItemLinked").unwrap();
    assert!(req_pos < lnk_pos, "LinkAddRequested must precede ItemLinked");
}

#[test]
fn test_happy_path_list_requested_before_returned() {
    let all = load_fixture("item_links_happy_path.jsonl");
    let events = il_events(&all);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"LinkListRequested"), "Fixture must contain LinkListRequested");
    assert!(types.contains(&"LinkListReturned"),  "Fixture must contain LinkListReturned");

    let req_pos = types.iter().position(|&t| t == "LinkListRequested").unwrap();
    let ret_pos = types.iter().position(|&t| t == "LinkListReturned").unwrap();
    assert!(req_pos < ret_pos, "LinkListRequested must precede LinkListReturned");
}

#[test]
fn test_happy_path_add_requested_and_linked_share_correlation_id() {
    let all = load_fixture("item_links_happy_path.jsonl");
    let events = il_events(&all);

    let req    = events.iter().find(|e| e["event_type"] == "LinkAddRequested").unwrap();
    let linked = events.iter().find(|e| e["event_type"] == "ItemLinked").unwrap();

    assert_eq!(
        req["correlation_id"].as_str().unwrap(),
        linked["correlation_id"].as_str().unwrap(),
        "LinkAddRequested and ItemLinked must share correlation_id"
    );
}

#[test]
fn test_happy_path_list_requested_and_returned_share_correlation_id() {
    let all = load_fixture("item_links_happy_path.jsonl");
    let events = il_events(&all);

    let req     = events.iter().find(|e| e["event_type"] == "LinkListRequested").unwrap();
    let returned = events.iter().find(|e| e["event_type"] == "LinkListReturned").unwrap();

    assert_eq!(
        req["correlation_id"].as_str().unwrap(),
        returned["correlation_id"].as_str().unwrap(),
        "LinkListRequested and LinkListReturned must share correlation_id"
    );
}

// ── Payload shape conformance ─────────────────────────────────────────────────

#[test]
fn test_happy_path_itemlinked_payload_shape() {
    let all = load_fixture("item_links_happy_path.jsonl");
    let events = il_events(&all);
    let linked = events.iter().find(|e| e["event_type"] == "ItemLinked").unwrap();
    let p = &linked["payload"];

    assert!(p["source_id"].as_str().is_some(),   "source_id must be a string");
    assert!(p["source_type"].as_str().is_some(),  "source_type must be a string");
    assert!(p["link_type"].as_str().is_some(),    "link_type must be a string");
    assert!(p["target_id"].as_str().is_some(),    "target_id must be a string");
    assert!(p["target_type"].as_str().is_some(),  "target_type must be a string");

    assert!(VALID_LINK_TYPES.contains(&p["link_type"].as_str().unwrap()),
        "link_type in ItemLinked must be a valid link type");
    assert!(VALID_ITEM_TYPES.contains(&p["source_type"].as_str().unwrap()),
        "source_type in ItemLinked must be a valid item type");
    assert!(VALID_ITEM_TYPES.contains(&p["target_type"].as_str().unwrap()),
        "target_type in ItemLinked must be a valid item type");
}

#[test]
fn test_happy_path_linklistreturned_payload_shape() {
    let all = load_fixture("item_links_happy_path.jsonl");
    let events = il_events(&all);
    let returned = events.iter().find(|e| e["event_type"] == "LinkListReturned").unwrap();
    let p = &returned["payload"];

    assert!(p.get("item_id").is_some(),       "item_id field must be present");
    assert!(p["link_count"].as_u64().is_some(),"link_count must be a non-negative integer");
    assert!(p["links"].is_array(),             "links must be an array");
    assert_eq!(
        p["link_count"].as_u64().unwrap() as usize,
        p["links"].as_array().unwrap().len(),
        "link_count must equal links array length"
    );
}

#[test]
fn test_happy_path_each_link_entry_has_required_fields() {
    let all = load_fixture("item_links_happy_path.jsonl");
    let events = il_events(&all);
    let returned = events.iter().find(|e| e["event_type"] == "LinkListReturned").unwrap();
    let links = returned["payload"]["links"].as_array().unwrap();

    assert!(!links.is_empty(), "Happy path fixture must have at least one link");

    for link in links {
        assert!(link["source_id"].as_str().is_some(),    "link entry must have source_id");
        assert!(link["source_type"].as_str().is_some(),  "link entry must have source_type");
        assert!(link["link_type"].as_str().is_some(),    "link entry must have link_type");
        assert!(link["target_id"].as_str().is_some(),    "link entry must have target_id");
        assert!(link["target_type"].as_str().is_some(),  "link entry must have target_type");
        assert!(link["direction"].as_str().is_some(),    "link entry must have direction");
        assert!(link["display_label"].as_str().is_some(),"link entry must have display_label");

        assert!(VALID_DIRECTIONS.contains(&link["direction"].as_str().unwrap()),
            "direction must be 'outgoing' or 'incoming'");
        assert!(VALID_LINK_TYPES.contains(&link["link_type"].as_str().unwrap()),
            "link_type in link entry must be a valid link type");
    }
}

#[test]
fn test_happy_path_list_all_links_have_outgoing_direction() {
    let all = load_fixture("item_links_happy_path.jsonl");
    let events = il_events(&all);
    let returned = events.iter().find(|e| e["event_type"] == "LinkListReturned").unwrap();

    if returned["payload"]["item_id"].is_null() {
        let links = returned["payload"]["links"].as_array().unwrap();
        for link in links {
            assert_eq!(link["direction"].as_str().unwrap(), "outgoing",
                "all-links listing must show only outgoing direction");
        }
    }
}

#[test]
fn test_happy_path_link_count_is_positive() {
    let all = load_fixture("item_links_happy_path.jsonl");
    let events = il_events(&all);
    let returned = events.iter().find(|e| e["event_type"] == "LinkListReturned").unwrap();

    assert!(returned["payload"]["link_count"].as_u64().unwrap() > 0,
        "happy path link_count must be > 0");
}

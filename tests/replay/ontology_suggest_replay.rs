//! Replay verification tests for ontology_suggest.
//!
//! Loads JSONL event fixtures and verifies that ontology_suggest events
//! conform to the approved event schema (events/ontology_suggest_schema.md):
//! required fields, valid event types, correct payload shapes, and valid
//! event sequences. Also verifies delegated events have correct source_module.

use project_schema::test_support::load_fixture;
use serde_json::Value;

fn os_events(all: &[Value]) -> Vec<&Value> {
    all.iter()
        .filter(|e| e["source_module"].as_str() == Some("ontology_suggest"))
        .collect()
}

const VALID_OS_EVENT_TYPES: &[&str] = &[
    "OntologyReviewRequested",
    "OntologyReviewProposed",
    "OntologyReviewFailedEmptyRecord",
    "OntologyReviewFailedLLMUnavailable",
    "OntologyReviewFailedSchemaInvalid",     // R11
    "OntologyReviewFailedNoRecognizedItems", // R11
    "OntologyConfirmRequested",
    "OntologyReviewConfirmed",
    "OntologyConfirmFailedReviewNotFound",
];

const VALID_PROPOSAL_TYPES: &[&str] = &["link", "status", "priority"];

const VALID_LINK_TYPES: &[&str] = &[
    "blocks",
    "affects",
    "assigned_to",
    "mitigated_by",
    "escalates_to",
    "related_to",
];

// ── Schema conformance ────────────────────────────────────────────────────────

#[test]
fn test_happy_path_all_os_events_have_required_base_fields() {
    let all = load_fixture("ontology_suggest_happy_path.jsonl");
    let events = os_events(&all);
    assert!(!events.is_empty(), "Fixture must contain ontology_suggest events");

    for event in &events {
        let t = event["event_type"].as_str().unwrap_or("unknown");
        assert!(
            event["event_id"].as_str().is_some(),
            "{}: event_id must be a string",
            t
        );
        assert!(
            event["event_type"].as_str().is_some(),
            "{}: event_type must be a string",
            t
        );
        assert!(
            event["timestamp"].as_u64().is_some(),
            "{}: timestamp must be a u64",
            t
        );
        assert!(
            event["correlation_id"].as_str().is_some(),
            "{}: correlation_id must be a string",
            t
        );
        assert!(
            event["source_module"].as_str().is_some(),
            "{}: source_module must be a string",
            t
        );
        assert!(
            event["payload"].is_object(),
            "{}: payload must be an object",
            t
        );
        assert_eq!(
            event["source_module"].as_str().unwrap(),
            "ontology_suggest",
            "{}: source_module must be ontology_suggest",
            t
        );
        assert!(
            event["timestamp"].as_u64().unwrap() > 0,
            "{}: timestamp must be positive",
            t
        );
    }
}

#[test]
fn test_happy_path_event_types_are_schema_members() {
    let all = load_fixture("ontology_suggest_happy_path.jsonl");
    let events = os_events(&all);
    for event in &events {
        let t = event["event_type"].as_str().unwrap();
        assert!(
            VALID_OS_EVENT_TYPES.contains(&t),
            "Event type '{}' is not in the approved ontology_suggest schema",
            t
        );
    }
}

#[test]
fn test_happy_path_no_failure_events() {
    let all = load_fixture("ontology_suggest_happy_path.jsonl");
    let events = os_events(&all);
    for event in &events {
        let t = event["event_type"].as_str().unwrap();
        assert!(
            !t.starts_with("OntologyReviewFailed") && !t.starts_with("OntologyConfirmFailed"),
            "Happy path fixture must not contain failure event '{}'",
            t
        );
    }
}

// ── Sequence conformance ──────────────────────────────────────────────────────

#[test]
fn test_happy_path_review_requested_before_proposed() {
    let all = load_fixture("ontology_suggest_happy_path.jsonl");
    let events = os_events(&all);
    let types: Vec<&str> = events
        .iter()
        .map(|e| e["event_type"].as_str().unwrap())
        .collect();

    assert!(
        types.contains(&"OntologyReviewRequested"),
        "Fixture must contain OntologyReviewRequested"
    );
    assert!(
        types.contains(&"OntologyReviewProposed"),
        "Fixture must contain OntologyReviewProposed"
    );

    let req_pos = types
        .iter()
        .position(|&t| t == "OntologyReviewRequested")
        .unwrap();
    let prop_pos = types
        .iter()
        .position(|&t| t == "OntologyReviewProposed")
        .unwrap();
    assert!(
        req_pos < prop_pos,
        "OntologyReviewRequested must precede OntologyReviewProposed"
    );
}

#[test]
fn test_happy_path_confirm_requested_before_confirmed() {
    let all = load_fixture("ontology_suggest_happy_path.jsonl");
    let events = os_events(&all);
    let types: Vec<&str> = events
        .iter()
        .map(|e| e["event_type"].as_str().unwrap())
        .collect();

    assert!(
        types.contains(&"OntologyConfirmRequested"),
        "Fixture must contain OntologyConfirmRequested"
    );
    assert!(
        types.contains(&"OntologyReviewConfirmed"),
        "Fixture must contain OntologyReviewConfirmed"
    );

    let req_pos = types
        .iter()
        .position(|&t| t == "OntologyConfirmRequested")
        .unwrap();
    let conf_pos = types
        .iter()
        .position(|&t| t == "OntologyReviewConfirmed")
        .unwrap();
    assert!(
        req_pos < conf_pos,
        "OntologyConfirmRequested must precede OntologyReviewConfirmed"
    );
}

#[test]
fn test_happy_path_item_linked_between_confirm_requested_and_confirmed() {
    let all = load_fixture("ontology_suggest_happy_path.jsonl");
    let types: Vec<(&str, &str)> = all
        .iter()
        .map(|e| {
            (
                e["event_type"].as_str().unwrap(),
                e["source_module"].as_str().unwrap(),
            )
        })
        .collect();

    let req_pos = types
        .iter()
        .position(|(et, _)| *et == "OntologyConfirmRequested")
        .unwrap();
    let conf_pos = types
        .iter()
        .position(|(et, _)| *et == "OntologyReviewConfirmed")
        .unwrap();
    let linked_pos = types
        .iter()
        .position(|(et, sm)| *et == "ItemLinked" && *sm == "item_links")
        .expect("Fixture must contain ItemLinked with source_module=item_links");

    assert!(
        req_pos < linked_pos,
        "ItemLinked must come after OntologyConfirmRequested"
    );
    assert!(
        linked_pos < conf_pos,
        "ItemLinked must come before OntologyReviewConfirmed"
    );
}

#[test]
fn test_happy_path_confirm_phases_share_correlation_id() {
    let all = load_fixture("ontology_suggest_happy_path.jsonl");

    let req = all
        .iter()
        .find(|e| e["event_type"] == "OntologyConfirmRequested")
        .unwrap();
    let conf = all
        .iter()
        .find(|e| e["event_type"] == "OntologyReviewConfirmed")
        .unwrap();
    let linked = all
        .iter()
        .find(|e| e["event_type"] == "ItemLinked")
        .unwrap();

    let cid = req["correlation_id"].as_str().unwrap();
    assert_eq!(
        conf["correlation_id"].as_str().unwrap(),
        cid,
        "OntologyReviewConfirmed must share correlation_id with OntologyConfirmRequested"
    );
    assert_eq!(
        linked["correlation_id"].as_str().unwrap(),
        cid,
        "ItemLinked must share correlation_id with OntologyConfirmRequested"
    );
}

// ── Payload shape conformance ─────────────────────────────────────────────────

#[test]
fn test_happy_path_review_proposed_payload_shape() {
    let all = load_fixture("ontology_suggest_happy_path.jsonl");
    let events = os_events(&all);
    let proposed = events
        .iter()
        .find(|e| e["event_type"] == "OntologyReviewProposed")
        .unwrap();
    let p = &proposed["payload"];

    assert!(p["review_id"].as_str().is_some(), "review_id must be a string");
    assert!(p["generated_count"].as_u64().is_some(), "generated_count must be a u64");
    assert!(p["proposal_count"].as_u64().is_some(), "proposal_count must be a u64");
    assert!(p["rejected_count"].as_u64().is_some(), "rejected_count must be a u64");
    assert!(p["proposals"].is_array(), "proposals must be an array");
    assert_eq!(
        p["proposal_count"].as_u64().unwrap() as usize,
        p["proposals"].as_array().unwrap().len(),
        "proposal_count must equal proposals array length"
    );
    assert_eq!(
        p["generated_count"].as_u64().unwrap(),
        p["proposal_count"].as_u64().unwrap() + p["rejected_count"].as_u64().unwrap(),
        "generated_count must equal proposal_count + rejected_count"
    );
}

#[test]
fn test_happy_path_each_proposal_has_required_fields() {
    let all = load_fixture("ontology_suggest_happy_path.jsonl");
    let events = os_events(&all);
    let proposed = events
        .iter()
        .find(|e| e["event_type"] == "OntologyReviewProposed")
        .unwrap();
    let proposals = proposed["payload"]["proposals"].as_array().unwrap();

    assert!(!proposals.is_empty(), "Happy path must have at least one proposal");

    for p in proposals {
        assert!(
            p["proposal_id"].as_str().is_some(),
            "Each proposal must have a proposal_id"
        );
        let ptype = p["type"].as_str().expect("Each proposal must have a type");
        assert!(
            VALID_PROPOSAL_TYPES.contains(&ptype),
            "Proposal type '{}' must be one of: link, status, priority",
            ptype
        );
        assert!(
            p["rationale"].as_str().is_some(),
            "Each proposal must have a rationale"
        );

        match ptype {
            "link" => {
                assert!(p["source_id"].as_str().is_some(), "link proposal must have source_id");
                assert!(p["link_type"].as_str().is_some(), "link proposal must have link_type");
                assert!(p["target_id"].as_str().is_some(), "link proposal must have target_id");
                assert!(VALID_LINK_TYPES.contains(&p["link_type"].as_str().unwrap()),
                    "link_type must be a valid type");
            }
            "status" => {
                assert!(p["item_id"].as_str().is_some(),       "status proposal must have item_id");
                assert!(p["proposed_status"].as_str().is_some(),"status proposal must have proposed_status");
            }
            "priority" => {
                assert!(p["item_id"].as_str().is_some(),         "priority proposal must have item_id");
                assert!(p["proposed_priority"].as_str().is_some(),"priority proposal must have proposed_priority");
                assert!(
                    ["high", "medium", "low"].contains(&p["proposed_priority"].as_str().unwrap()),
                    "proposed_priority must be high, medium, or low"
                );
            }
            _ => {}
        }
    }
}

#[test]
fn test_happy_path_review_confirmed_payload_shape() {
    let all = load_fixture("ontology_suggest_happy_path.jsonl");
    let events = os_events(&all);
    let confirmed = events
        .iter()
        .find(|e| e["event_type"] == "OntologyReviewConfirmed")
        .unwrap();
    let p = &confirmed["payload"];

    assert!(p["review_id"].as_str().is_some(),       "review_id must be a string");
    assert!(p["accepted_count"].as_u64().is_some(),  "accepted_count must be a u64");
    assert!(p["rejected_count"].as_u64().is_some(),  "rejected_count must be a u64");
    assert!(p["skipped_count"].as_u64().is_some(),   "skipped_count must be a u64");
    assert!(p["accepted_ids"].is_array(),             "accepted_ids must be an array");
    assert!(p["rejected_ids"].is_array(),             "rejected_ids must be an array");
    assert!(p["skipped_ids"].is_array(),              "skipped_ids must be an array");

    assert_eq!(
        p["accepted_count"].as_u64().unwrap() as usize,
        p["accepted_ids"].as_array().unwrap().len(),
        "accepted_count must equal accepted_ids array length"
    );
    assert_eq!(
        p["rejected_count"].as_u64().unwrap() as usize,
        p["rejected_ids"].as_array().unwrap().len(),
        "rejected_count must equal rejected_ids array length"
    );
    assert_eq!(
        p["skipped_count"].as_u64().unwrap() as usize,
        p["skipped_ids"].as_array().unwrap().len(),
        "skipped_count must equal skipped_ids array length"
    );
}

#[test]
fn test_happy_path_delegated_item_linked_has_correct_source_module() {
    let all = load_fixture("ontology_suggest_happy_path.jsonl");
    let linked = all
        .iter()
        .find(|e| e["event_type"] == "ItemLinked")
        .expect("Fixture must contain an ItemLinked event");

    assert_eq!(
        linked["source_module"].as_str().unwrap(),
        "item_links",
        "ItemLinked produced by ontology_suggest confirm must have source_module=item_links"
    );

    let p = &linked["payload"];
    assert!(p["source_id"].as_str().is_some(),  "ItemLinked must have source_id");
    assert!(p["source_type"].as_str().is_some(), "ItemLinked must have source_type");
    assert!(p["link_type"].as_str().is_some(),   "ItemLinked must have link_type");
    assert!(p["target_id"].as_str().is_some(),  "ItemLinked must have target_id");
    assert!(p["target_type"].as_str().is_some(), "ItemLinked must have target_type");
}

#[test]
fn test_happy_path_review_ids_consistent_across_analyse_and_confirm_phases() {
    let all = load_fixture("ontology_suggest_happy_path.jsonl");
    let os = os_events(&all);

    let proposed = os.iter().find(|e| e["event_type"] == "OntologyReviewProposed").unwrap();
    let req      = os.iter().find(|e| e["event_type"] == "OntologyConfirmRequested").unwrap();
    let confirmed = os.iter().find(|e| e["event_type"] == "OntologyReviewConfirmed").unwrap();

    let review_id = proposed["payload"]["review_id"].as_str().unwrap();
    assert_eq!(
        req["payload"]["review_id"].as_str().unwrap(),
        review_id,
        "OntologyConfirmRequested review_id must match OntologyReviewProposed review_id"
    );
    assert_eq!(
        confirmed["payload"]["review_id"].as_str().unwrap(),
        review_id,
        "OntologyReviewConfirmed review_id must match OntologyReviewProposed review_id"
    );
}

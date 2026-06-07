use anyhow::{Context, Result};
use clap::Parser;
use project_schema::{all_status_names, emit_type_unknown, is_block_type, load_and_validate, marker_to_status, resolve_type, EventEnvelope, ProjectSchema};
use serde_json::{json, Value};
use std::fs;
use std::io::BufRead;
use std::path::Path;
use uuid::Uuid;

const EVENTS_FILE: &str = "events/runtime_events.jsonl";
const SOURCE_MODULE: &str = "priority_view";

const VALID_PRIORITIES: &[&str] = &["high", "medium", "low"];

#[derive(Parser)]
#[command(about = "LucidPM priority-ranked item view")]
struct Cli {
    #[arg(long = "type", value_name = "TYPE")]
    filter_type: Option<String>,
    #[arg(long = "status", value_name = "STATUS")]
    filter_status: Option<String>,
    #[arg(long = "priority", value_name = "PRIORITY")]
    filter_priority: Option<String>,
}

struct ItemSummary {
    item_id: String,
    item_type: String,
    description: String,
    session_id: String,
    status: Option<String>,
    priority: Option<String>,
}

fn emit_event(event_type: &str, correlation_id: &str, payload: Value) {
    project_schema::emit_event(Path::new(EVENTS_FILE), EventEnvelope {
        source_module: SOURCE_MODULE,
        event_type,
        correlation_id,
        payload,
    });
}

fn priority_rank(priority: Option<&str>) -> u8 {
    match priority {
        Some("high")   => 1,
        Some("medium") => 2,
        Some("low")    => 3,
        _              => 4,
    }
}

fn status_rank(status: Option<&str>) -> u8 {
    match status {
        Some("doing") | Some("in_progress") | Some("active")      => 1,
        Some("todo")  | Some("open")        | Some("pending")     => 2,
        Some("waiting")                                            => 3,
        Some("done")      | Some("achieved")   | Some("resolved")
        | Some("mitigated") | Some("accepted") | Some("cancelled")
        | Some("missed")  | Some("closed")     | Some("inactive") => 4,
        _                                                          => 5,
    }
}

fn read_events() -> Result<Vec<Value>> {
    if !Path::new(EVENTS_FILE).exists() {
        return Ok(vec![]);
    }
    let file = fs::File::open(EVENTS_FILE).context("opening events file")?;
    Ok(std::io::BufReader::new(file)
        .lines()
        .filter_map(|l| l.ok())
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str(&l).ok())
        .collect())
}

fn incorporated_sessions(events: &[Value]) -> Vec<String> {
    events.iter()
        .filter(|e| {
            e["source_module"].as_str() == Some("project_state")
                && e["event_type"].as_str() == Some("ItemsIncorporated")
        })
        .filter_map(|e| e["payload"]["session_id"].as_str().map(String::from))
        .collect()
}

fn confirmed_items_for_session(
    events: &[Value],
    session_id: &str,
) -> Vec<(String, String, String)> {
    let mut items_extracted: Option<Vec<Value>> = None;
    let mut accepted_ids: Option<Vec<String>> = None;

    for e in events {
        if e["correlation_id"].as_str() != Some(session_id) { continue; }
        match e["event_type"].as_str() {
            Some("ItemsExtracted") => {
                items_extracted = e["payload"]["items"].as_array().cloned();
            }
            Some("ExtractionConfirmed") => {
                accepted_ids = e["payload"]["accepted_item_ids"]
                    .as_array()
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect());
            }
            _ => {}
        }
    }

    let raw = items_extracted.unwrap_or_default();
    let accepted = accepted_ids.unwrap_or_default();

    raw.into_iter()
        .filter(|item| {
            item["item_id"].as_str()
                .map(|id| accepted.contains(&id.to_string()))
                .unwrap_or(false)
        })
        .map(|item| (
            item["item_id"].as_str().unwrap_or("").to_string(),
            item["item_type"].as_str().unwrap_or("").to_string(),
            item["description"].as_str().unwrap_or("").to_string(),
        ))
        .collect()
}

fn proposed_values_for(events: &[Value], item_id: &str) -> (Option<String>, Option<String>) {
    let candidate = events.iter().find_map(|e| {
        if e["source_module"].as_str() != Some("pm_structuring") { return None; }
        if e["event_type"].as_str() != Some("ItemsExtracted") { return None; }
        let corr_id = e["correlation_id"].as_str()?;
        let items = e["payload"]["items"].as_array()?;
        let item = items.iter().find(|i| i["item_id"].as_str() == Some(item_id))?;
        Some((
            corr_id.to_string(),
            item["proposed_status"].as_str().map(String::from),
            item["proposed_priority"].as_str().map(String::from),
        ))
    });

    let (corr_id, prop_status, prop_priority) = match candidate {
        Some(c) => c,
        None => return (None, None),
    };

    let confirmed = events.iter().any(|e| {
        e["source_module"].as_str() == Some("pm_structuring")
            && e["event_type"].as_str() == Some("ExtractionConfirmed")
            && e["correlation_id"].as_str() == Some(corr_id.as_str())
            && e["payload"]["accepted_item_ids"]
                .as_array()
                .map(|arr| arr.iter().any(|id| id.as_str() == Some(item_id)))
                .unwrap_or(false)
    });

    if confirmed { (prop_status, prop_priority) } else { (None, None) }
}

/// Current marker for a task instance (latest TaskMarkerUpdated, else TaskAdded initial).
fn current_task_marker_from_events(events: &[Value], task_id: &str) -> Option<String> {
    let mut current = None;
    for e in events {
        if e["source_module"].as_str() != Some("task_model") { continue; }
        match e["event_type"].as_str() {
            Some("TaskAdded") if e["payload"]["task_id"].as_str() == Some(task_id) => {
                current = e["payload"]["initial_marker"].as_str().map(String::from);
            }
            Some("TaskMarkerUpdated") if e["payload"]["task_id"].as_str() == Some(task_id) => {
                current = e["payload"]["new_marker"].as_str().map(String::from);
            }
            _ => {}
        }
    }
    current
}

fn effective_status_priority(
    events: &[Value],
    item_id: &str,
    item_type: &str,
    schema: &ProjectSchema,
) -> (Option<String>, Option<String>) {
    let mut last_status = None;
    let mut last_priority = None;

    for e in events {
        let src = e["source_module"].as_str().unwrap_or("");
        if (src == "item_status" || src == "logseq_sync")
            && e["payload"]["item_id"].as_str() == Some(item_id)
        {
            match e["event_type"].as_str() {
                Some("ItemStatusUpdated") => {
                    last_status = e["payload"]["new_status"].as_str().map(String::from);
                }
                Some("ItemPriorityUpdated") => {
                    last_priority = e["payload"]["new_priority"].as_str().map(String::from);
                }
                _ => {}
            }
        }
    }

    // Marker-derived status for block-type items (Representation Ban: is_block_type via API)
    if last_status.is_none() && is_block_type(schema, item_type) {
        if let Some(marker) = current_task_marker_from_events(events, item_id) {
            if let Some(mapped) = marker_to_status(schema, &marker) {
                last_status = Some(mapped.to_string());
            }
        }
    }

    if last_status.is_none() || last_priority.is_none() {
        let (prop_status, prop_priority) = proposed_values_for(events, item_id);
        if last_status.is_none()   { last_status   = prop_status;   }
        if last_priority.is_none() { last_priority = prop_priority; }
    }

    (last_status, last_priority)
}

// Check whether a status string is present in the vocabulary's global status union.
fn status_in_vocabulary(schema: &ProjectSchema, status: &str) -> bool {
    all_status_names(schema).contains(&status)
}

fn cmd_view(
    filter_type: Option<&str>,
    filter_status: Option<&str>,
    filter_priority: Option<&str>,
) -> Result<()> {
    let correlation_id = Uuid::new_v4().to_string();

    // Schema load before any priority_view event — abort if schema invalid (FP1: SchemaInvalid).
    // project_schema emits the failure event and prints to stderr.
    let schema = match load_and_validate(Path::new("."), Path::new(EVENTS_FILE), &correlation_id) {
        Some(s) => s,
        None => return Ok(()),
    };

    emit_event("PriorityViewRequested", &correlation_id, json!({
        "filter_type":     filter_type,
        "filter_status":   filter_status,
        "filter_priority": filter_priority,
    }));

    let events = read_events()?;
    let sessions = incorporated_sessions(&events);

    let mut all_items: Vec<(String, String, String, String)> = sessions.iter()
        .flat_map(|sid| {
            confirmed_items_for_session(&events, sid)
                .into_iter()
                .map(|(id, ty, desc)| (id, ty, desc, sid.clone()))
        })
        .collect();

    // Task instances from TaskAdded events
    for e in &events {
        if e["source_module"].as_str() == Some("task_model")
            && e["event_type"].as_str() == Some("TaskAdded")
        {
            let task_id = e["payload"]["task_id"].as_str().unwrap_or("").to_string();
            let item_type = e["payload"]["item_type"].as_str().unwrap_or("").to_string();
            if !task_id.is_empty() && !item_type.is_empty() {
                all_items.push((
                    task_id,
                    item_type,
                    e["payload"]["description"].as_str().unwrap_or("").to_string(),
                    "task_model".to_string(),
                ));
            }
        }
    }

    // Contract failure: EmptyRecord (project record empty before any exclusion).
    if all_items.is_empty() {
        eprintln!("No items in project record.");
        emit_event("PriorityViewFailedEmptyRecord", &correlation_id, json!({
            "failure_reason": "empty_record",
        }));
        return Ok(());
    }

    // Contract failure: InvalidFilter — validate type and status against active vocabulary.
    // Priority filter remains hardcoded (not schema-driven in this release).
    if let Some(t) = filter_type {
        if resolve_type(&schema, t).is_none() {
            eprintln!("Invalid --type '{}'. Value is not recognized by the active vocabulary.", t);
            emit_event("PriorityViewFailedInvalidFilter", &correlation_id, json!({
                "failure_reason": "invalid_filter",
                "filter_field":   "type",
                "filter_value":   t,
            }));
            return Ok(());
        }
    }
    if let Some(s) = filter_status {
        if !status_in_vocabulary(&schema, s) {
            eprintln!("Invalid --status '{}'. Value is not in the active vocabulary.", s);
            emit_event("PriorityViewFailedInvalidFilter", &correlation_id, json!({
                "failure_reason": "invalid_filter",
                "filter_field":   "status",
                "filter_value":   s,
            }));
            return Ok(());
        }
    }
    if let Some(p) = filter_priority {
        if !VALID_PRIORITIES.contains(&p) {
            eprintln!("Invalid --priority '{}'. Valid values: {}", p, VALID_PRIORITIES.join(", "));
            emit_event("PriorityViewFailedInvalidFilter", &correlation_id, json!({
                "failure_reason": "invalid_filter",
                "filter_field":   "priority",
                "filter_value":   p,
            }));
            return Ok(());
        }
    }

    // Build item summaries, excluding items with unrecognized entity types.
    // emit_type_unknown (source_module: "project_schema") is called per excluded item.
    let mut summaries: Vec<ItemSummary> = all_items.into_iter()
        .filter_map(|(item_id, item_type, description, session_id)| {
            if resolve_type(&schema, &item_type).is_none() {
                emit_type_unknown(
                    Path::new(EVENTS_FILE),
                    &item_id,
                    &item_type,
                    &correlation_id,
                );
                return None;
            }
            let (status, priority) = effective_status_priority(&events, &item_id, &item_type, &schema);
            Some(ItemSummary { item_id, item_type, description, session_id, status, priority })
        })
        .collect();

    // Apply filters (conjunctive — contract invariant).
    // Type filter uses alias resolution: filter value and stored type both resolved to
    // canonical name before comparison, so --type epic matches items stored as "Initiative".
    if let Some(t) = filter_type {
        let canonical_filter = resolve_type(&schema, t);
        summaries.retain(|i| resolve_type(&schema, &i.item_type) == canonical_filter);
    }
    if let Some(s) = filter_status   { summaries.retain(|i| i.status.as_deref() == Some(s)); }
    if let Some(p) = filter_priority { summaries.retain(|i| i.priority.as_deref() == Some(p)); }

    // Sort: primary priority rank, secondary status rank
    summaries.sort_by_key(|i| (
        priority_rank(i.priority.as_deref()),
        status_rank(i.status.as_deref()),
    ));

    let items_payload: Vec<Value> = summaries.iter().map(|i| json!({
        "item_id":     i.item_id,
        "item_type":   i.item_type,
        "description": i.description,
        "priority":    i.priority,
        "status":      i.status,
        "session_id":  i.session_id,
    })).collect();

    let item_count = summaries.len();

    emit_event("PriorityViewReturned", &correlation_id, json!({
        "item_count": item_count,
        "filters_applied": {
            "type":     filter_type,
            "status":   filter_status,
            "priority": filter_priority,
        },
        "items": items_payload,
    }));

    if summaries.is_empty() {
        println!("No items match the specified filters.");
        return Ok(());
    }

    println!("{:<8} {:<14} {:<14} {}", "PRI", "TYPE", "STATUS", "DESCRIPTION");
    println!("{}", "-".repeat(74));
    for item in &summaries {
        let pri = item.priority.as_deref().unwrap_or("-");
        let sta = item.status.as_deref().unwrap_or("-");
        println!("{:<8} {:<14} {:<14} {}", pri, item.item_type, sta, item.description);
    }
    println!("\n{} item(s) shown.", item_count);

    Ok(())
}

fn main() {
    let cli = Cli::parse();
    let result = cmd_view(
        cli.filter_type.as_deref(),
        cli.filter_status.as_deref(),
        cli.filter_priority.as_deref(),
    );
    if let Err(e) = result {
        eprintln!("Error: {:#}", e);
        std::process::exit(1);
    }
}

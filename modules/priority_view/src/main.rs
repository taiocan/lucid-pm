use anyhow::{Context, Result};
use clap::Parser;
use serde_json::{json, Value};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const EVENTS_FILE: &str = "events/runtime_events.jsonl";
const SOURCE_MODULE: &str = "priority_view";

const VALID_TYPES: &[&str] = &["task", "milestone", "risk", "issue", "stakeholder"];
const VALID_PRIORITIES: &[&str] = &["high", "medium", "low"];
const VALID_STATUSES: &[&str] = &[
    "todo", "doing", "done", "waiting", "cancelled",
    "pending", "achieved", "missed",
    "open", "in_progress", "resolved", "closed",
    "active", "inactive",
];

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

fn timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

fn emit_event(event_type: &str, correlation_id: &str, payload: Value) {
    let event = json!({
        "event_id": Uuid::new_v4().to_string(),
        "event_type": event_type,
        "timestamp": timestamp_ms(),
        "correlation_id": correlation_id,
        "source_module": SOURCE_MODULE,
        "payload": payload,
    });
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(EVENTS_FILE)
        .expect("Failed to open events file");
    writeln!(file, "{}", event).expect("Failed to write event");
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

fn effective_status_priority(events: &[Value], item_id: &str) -> (Option<String>, Option<String>) {
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

    if last_status.is_none() || last_priority.is_none() {
        let (prop_status, prop_priority) = proposed_values_for(events, item_id);
        if last_status.is_none()   { last_status   = prop_status;   }
        if last_priority.is_none() { last_priority = prop_priority; }
    }

    (last_status, last_priority)
}

fn cmd_view(
    filter_type: Option<&str>,
    filter_status: Option<&str>,
    filter_priority: Option<&str>,
) -> Result<()> {
    let correlation_id = Uuid::new_v4().to_string();

    emit_event("PriorityViewRequested", &correlation_id, json!({
        "filter_type":     filter_type,
        "filter_status":   filter_status,
        "filter_priority": filter_priority,
    }));

    // Contract failure: InvalidFilter
    if let Some(t) = filter_type {
        if !VALID_TYPES.contains(&t) {
            eprintln!("Invalid --type '{}'. Valid values: {}", t, VALID_TYPES.join(", "));
            emit_event("PriorityViewFailedInvalidFilter", &correlation_id, json!({
                "failure_reason": "invalid_filter",
                "filter_field":   "type",
                "filter_value":   t,
            }));
            return Ok(());
        }
    }
    if let Some(s) = filter_status {
        if !VALID_STATUSES.contains(&s) {
            eprintln!("Invalid --status '{}'. Valid values: {}", s, VALID_STATUSES.join(", "));
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

    let events = read_events()?;
    let sessions = incorporated_sessions(&events);

    let all_items: Vec<(String, String, String, String)> = sessions.iter()
        .flat_map(|sid| {
            confirmed_items_for_session(&events, sid)
                .into_iter()
                .map(|(id, ty, desc)| (id, ty, desc, sid.clone()))
        })
        .collect();

    // Contract failure: EmptyRecord
    if all_items.is_empty() {
        eprintln!("No items in project record.");
        emit_event("PriorityViewFailedEmptyRecord", &correlation_id, json!({
            "failure_reason": "empty_record",
        }));
        return Ok(());
    }

    let mut summaries: Vec<ItemSummary> = all_items.into_iter()
        .map(|(item_id, item_type, description, session_id)| {
            let (status, priority) = effective_status_priority(&events, &item_id);
            ItemSummary { item_id, item_type, description, session_id, status, priority }
        })
        .collect();

    // Apply filters (conjunctive — contract invariant)
    if let Some(t) = filter_type     { summaries.retain(|i| i.item_type == t); }
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

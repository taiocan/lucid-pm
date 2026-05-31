use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde_json::{json, Value};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const EVENTS_FILE: &str = "events/runtime_events.jsonl";
const SOURCE_MODULE: &str = "item_status";

#[derive(Parser)]
#[command(about = "LucidPM item status and priority manager")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Set the status of a recorded project item
    SetStatus {
        /// Item ID from the project record
        item_id: String,
        /// New status value (must be valid for the item's type)
        status: String,
    },
    /// Set the priority of a recorded project item
    SetPriority {
        /// Item ID from the project record
        item_id: String,
        /// New priority value: high, medium, or low
        priority: String,
    },
    /// Query the current status and priority of a recorded project item
    Get {
        /// Item ID from the project record
        item_id: String,
    },
}

#[derive(Clone)]
struct RecordedItem {
    item_id: String,
    item_type: String,
    description: String,
}

fn valid_statuses_for_type(item_type: &str) -> &'static [&'static str] {
    match item_type {
        "task"        => &["todo", "doing", "done", "waiting", "cancelled"],
        "milestone"   => &["pending", "achieved", "missed"],
        "risk"        => &["open", "mitigated", "accepted", "closed"],
        "issue"       => &["open", "in_progress", "resolved", "closed"],
        "stakeholder" => &["active", "inactive"],
        _             => &[],
    }
}

const VALID_PRIORITIES: &[&str] = &["high", "medium", "low"];

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

/// Return all sessions that project_state has incorporated, in emission order.
fn read_incorporated_sessions() -> Result<Vec<String>> {
    if !Path::new(EVENTS_FILE).exists() {
        return Ok(vec![]);
    }
    let file = fs::File::open(EVENTS_FILE).context("opening events file")?;
    let mut sessions = Vec::new();
    for line in std::io::BufReader::new(file).lines() {
        let line = line.context("reading events file")?;
        if line.trim().is_empty() { continue; }
        let event: Value = serde_json::from_str(&line).context("parsing event line")?;
        if event["source_module"].as_str() == Some("project_state")
            && event["event_type"].as_str() == Some("ItemsIncorporated")
        {
            if let Some(sid) = event["payload"]["session_id"].as_str() {
                sessions.push(sid.to_string());
            }
        }
    }
    Ok(sessions)
}

/// Return confirmed items from a pm_structuring session by scanning the event log.
fn find_confirmed_items(session_id: &str) -> Result<Vec<RecordedItem>> {
    let file = fs::File::open(EVENTS_FILE)
        .with_context(|| format!("opening {}", EVENTS_FILE))?;

    let mut items_extracted: Option<Vec<Value>> = None;
    let mut accepted_ids: Option<Vec<String>> = None;

    for line in std::io::BufReader::new(file).lines() {
        let line = line.context("reading runtime events")?;
        if line.trim().is_empty() { continue; }
        let event: Value = serde_json::from_str(&line).context("parsing event line")?;
        if event["correlation_id"].as_str() != Some(session_id) { continue; }

        match event["event_type"].as_str() {
            Some("ItemsExtracted") => {
                items_extracted = event["payload"]["items"].as_array().cloned();
            }
            Some("ExtractionConfirmed") => {
                accepted_ids = event["payload"]["accepted_item_ids"]
                    .as_array()
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect());
            }
            _ => {}
        }
    }

    let raw_items = items_extracted.unwrap_or_default();
    let accepted = accepted_ids.unwrap_or_default();

    let items = raw_items
        .into_iter()
        .filter(|item| {
            item["item_id"].as_str()
                .map(|id| accepted.contains(&id.to_string()))
                .unwrap_or(false)
        })
        .map(|item| RecordedItem {
            item_id: item["item_id"].as_str().unwrap_or("").to_string(),
            item_type: item["item_type"].as_str().unwrap_or("").to_string(),
            description: item["description"].as_str().unwrap_or("").to_string(),
        })
        .collect();

    Ok(items)
}

/// Return all items currently in the project record.
fn read_all_record_items() -> Result<Vec<RecordedItem>> {
    let sessions = read_incorporated_sessions()?;
    let mut all = Vec::new();
    for sid in sessions {
        all.extend(find_confirmed_items(&sid)?);
    }
    Ok(all)
}

/// Return the item if it exists in the project record, None otherwise.
fn find_item(item_id: &str) -> Result<Option<RecordedItem>> {
    Ok(read_all_record_items()?.into_iter().find(|i| i.item_id == item_id))
}

/// Return the most recently recorded status for item_id, or None if never set.
fn current_status(item_id: &str) -> Result<Option<String>> {
    if !Path::new(EVENTS_FILE).exists() { return Ok(None); }
    let file = fs::File::open(EVENTS_FILE).context("opening events file")?;
    let mut last = None;
    for line in std::io::BufReader::new(file).lines() {
        let line = line.context("reading events file")?;
        if line.trim().is_empty() { continue; }
        let event: Value = serde_json::from_str(&line).context("parsing event line")?;
        let src = event["source_module"].as_str().unwrap_or("");
        if (src == SOURCE_MODULE || src == "logseq_sync")
            && event["event_type"].as_str() == Some("ItemStatusUpdated")
            && event["payload"]["item_id"].as_str() == Some(item_id)
        {
            last = event["payload"]["new_status"].as_str().map(String::from);
        }
    }
    Ok(last)
}

/// Return the most recently recorded priority for item_id, or None if never set.
fn current_priority(item_id: &str) -> Result<Option<String>> {
    if !Path::new(EVENTS_FILE).exists() { return Ok(None); }
    let file = fs::File::open(EVENTS_FILE).context("opening events file")?;
    let mut last = None;
    for line in std::io::BufReader::new(file).lines() {
        let line = line.context("reading events file")?;
        if line.trim().is_empty() { continue; }
        let event: Value = serde_json::from_str(&line).context("parsing event line")?;
        let src = event["source_module"].as_str().unwrap_or("");
        if (src == SOURCE_MODULE || src == "logseq_sync")
            && event["event_type"].as_str() == Some("ItemPriorityUpdated")
            && event["payload"]["item_id"].as_str() == Some(item_id)
        {
            last = event["payload"]["new_priority"].as_str().map(String::from);
        }
    }
    Ok(last)
}

/// Return the proposed status and priority from extraction for item_id,
/// but only if the extraction was confirmed (ExtractionConfirmed includes the item_id).
fn proposed_values_from_extraction(item_id: &str) -> Result<(Option<String>, Option<String>)> {
    if !Path::new(EVENTS_FILE).exists() { return Ok((None, None)); }
    let file = fs::File::open(EVENTS_FILE).context("opening events file")?;
    let events: Vec<Value> = std::io::BufReader::new(file)
        .lines()
        .filter_map(|l| l.ok())
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str(&l).ok())
        .collect();

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

    let (corr_id, proposed_status, proposed_priority) = match candidate {
        Some(c) => c,
        None => return Ok((None, None)),
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

    if confirmed {
        Ok((proposed_status, proposed_priority))
    } else {
        Ok((None, None))
    }
}

fn cmd_set_status(item_id: &str, status: &str) -> Result<()> {
    let correlation_id = Uuid::new_v4().to_string();

    emit_event("StatusUpdateRequested", &correlation_id, json!({
        "item_id": item_id,
        "requested_status": status,
    }));

    // Contract failure: ItemNotFound
    let item = match find_item(item_id)? {
        Some(i) => i,
        None => {
            println!("Item '{}' not found in project record.", item_id);
            emit_event("StatusUpdateFailedItemNotFound", &correlation_id, json!({
                "failure_reason": "item_not_found",
                "item_id": item_id,
            }));
            return Ok(());
        }
    };

    // Contract failure: InvalidStatusForType
    let valid = valid_statuses_for_type(&item.item_type);
    if !valid.contains(&status) {
        println!(
            "Status '{}' is not valid for item type '{}'. Valid values: {}",
            status,
            item.item_type,
            valid.join(", ")
        );
        emit_event("StatusUpdateFailedInvalidStatus", &correlation_id, json!({
            "failure_reason": "invalid_status_for_type",
            "item_id": item_id,
            "item_type": item.item_type,
            "requested_status": status,
        }));
        return Ok(());
    }

    let previous_status = current_status(item_id)?;

    emit_event("ItemStatusUpdated", &correlation_id, json!({
        "item_id": item_id,
        "item_type": item.item_type,
        "new_status": status,
        "previous_status": previous_status,
    }));

    println!(
        "Status of '{}' ({}) set to '{}'.",
        &item_id[..8.min(item_id.len())],
        item.item_type,
        status
    );

    Ok(())
}

fn cmd_set_priority(item_id: &str, priority: &str) -> Result<()> {
    let correlation_id = Uuid::new_v4().to_string();

    emit_event("PriorityUpdateRequested", &correlation_id, json!({
        "item_id": item_id,
        "requested_priority": priority,
    }));

    // Contract failure: ItemNotFound
    let item = match find_item(item_id)? {
        Some(i) => i,
        None => {
            println!("Item '{}' not found in project record.", item_id);
            emit_event("PriorityUpdateFailedItemNotFound", &correlation_id, json!({
                "failure_reason": "item_not_found",
                "item_id": item_id,
            }));
            return Ok(());
        }
    };

    // Contract failure: InvalidPriorityValue
    if !VALID_PRIORITIES.contains(&priority) {
        println!(
            "Priority '{}' is not valid. Valid values: {}",
            priority,
            VALID_PRIORITIES.join(", ")
        );
        emit_event("PriorityUpdateFailedInvalidValue", &correlation_id, json!({
            "failure_reason": "invalid_priority_value",
            "item_id": item_id,
            "requested_priority": priority,
        }));
        return Ok(());
    }

    let previous_priority = current_priority(item_id)?;

    emit_event("ItemPriorityUpdated", &correlation_id, json!({
        "item_id": item_id,
        "new_priority": priority,
        "previous_priority": previous_priority,
    }));

    println!(
        "Priority of '{}' ({}) set to '{}'.",
        &item_id[..8.min(item_id.len())],
        item.item_type,
        priority
    );

    Ok(())
}

fn cmd_get(item_id: &str) -> Result<()> {
    let correlation_id = Uuid::new_v4().to_string();

    emit_event("ItemStatusQueried", &correlation_id, json!({
        "item_id": item_id,
    }));

    // Contract failure: ItemNotFound
    let item = match find_item(item_id)? {
        Some(i) => i,
        None => {
            println!("Item '{}' not found in project record.", item_id);
            emit_event("ItemStatusQueryFailedItemNotFound", &correlation_id, json!({
                "failure_reason": "item_not_found",
                "item_id": item_id,
            }));
            return Ok(());
        }
    };

    let explicit_status   = current_status(item_id)?;
    let explicit_priority = current_priority(item_id)?;

    let (prop_status, prop_priority) = proposed_values_from_extraction(item_id)?;

    let effective_status   = explicit_status.as_ref().or(prop_status.as_ref()).cloned();
    let effective_priority = explicit_priority.as_ref().or(prop_priority.as_ref()).cloned();

    let status_display = match explicit_status.as_deref() {
        Some(s) => s.to_string(),
        None    => match prop_status.as_deref() {
            Some(s) => format!("{} (proposed)", s),
            None    => "(not set)".to_string(),
        },
    };
    let priority_display = match explicit_priority.as_deref() {
        Some(p) => p.to_string(),
        None    => match prop_priority.as_deref() {
            Some(p) => format!("{} (proposed)", p),
            None    => "(not set)".to_string(),
        },
    };

    println!(
        "[{}] {} ({})\n  Status:   {}\n  Priority: {}",
        item.item_type.to_uppercase(),
        item.description,
        &item_id[..8.min(item_id.len())],
        status_display,
        priority_display,
    );

    emit_event("ItemStatusReturned", &correlation_id, json!({
        "item_id": item_id,
        "item_type": item.item_type,
        "current_status": effective_status,
        "current_priority": effective_priority,
    }));

    Ok(())
}

fn main() {
    let cli = Cli::parse();
    let result = match &cli.command {
        Commands::SetStatus   { item_id, status }   => cmd_set_status(item_id, status),
        Commands::SetPriority { item_id, priority }  => cmd_set_priority(item_id, priority),
        Commands::Get         { item_id }             => cmd_get(item_id),
    };
    if let Err(e) = result {
        eprintln!("Error: {:#}", e);
        std::process::exit(1);
    }
}

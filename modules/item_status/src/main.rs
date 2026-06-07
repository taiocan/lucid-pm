use anyhow::Result;
use clap::{Parser, Subcommand};
use lucid_core::{open_event_log, EventEmitter, RecordedItem, EVENTS_FILE};
use project_schema::{is_block_type, load_and_validate, marker_to_status, resolve_type, ProjectSchema};
use serde_json::{json, Value};
use std::path::Path;
use uuid::Uuid;

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
        /// New status value (must be valid for the item's type per active vocabulary)
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

const VALID_PRIORITIES: &[&str] = &["high", "medium", "low"];

// Check if `status` is valid for `item_type` per the active vocabulary.
// Contract: a type with an empty allowedStatuses set has no valid statuses.
fn vocabulary_allows_status(schema: &ProjectSchema, item_type: &str, status: &str) -> bool {
    let canonical = match resolve_type(schema, item_type) {
        Some(t) => t,
        None => return false,
    };
    schema
        .page_types
        .get(canonical)
        .map(|def| {
            !def.allowed_statuses.is_empty()
                && def.allowed_statuses.iter().any(|s| s == status)
        })
        .unwrap_or(false)
}

// Return the list of valid statuses for `item_type` from the active vocabulary.
fn valid_statuses_from_vocabulary(schema: &ProjectSchema, item_type: &str) -> Vec<String> {
    let canonical = match resolve_type(schema, item_type) {
        Some(t) => t,
        None => return vec![],
    };
    schema
        .page_types
        .get(canonical)
        .map(|def| def.allowed_statuses.clone())
        .unwrap_or_default()
}

// Extract the first whitespace-delimited token from text as a potential task marker.
fn extract_marker(text: &str) -> Option<String> {
    text.split_whitespace().next().map(String::from)
}

fn read_incorporated_sessions() -> Result<Vec<String>> {
    let mut sessions = Vec::new();
    for event in open_event_log(Path::new(EVENTS_FILE))? {
        let event = event?;
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

fn find_confirmed_items(session_id: &str) -> Result<Vec<RecordedItem>> {
    let mut items_extracted: Option<Vec<Value>> = None;
    let mut accepted_ids: Option<Vec<String>> = None;

    for event in open_event_log(Path::new(EVENTS_FILE))? {
        let event = event?;
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
            ..Default::default()
        })
        .collect();

    Ok(items)
}

fn read_all_record_items() -> Result<Vec<RecordedItem>> {
    let sessions = read_incorporated_sessions()?;
    let mut all = Vec::new();
    for sid in sessions {
        all.extend(find_confirmed_items(&sid)?);
    }
    // Task instances from TaskAdded events
    for event in open_event_log(Path::new(EVENTS_FILE))? {
        let event = event?;
        if event["source_module"].as_str() == Some("task_model")
            && event["event_type"].as_str() == Some("TaskAdded")
        {
            let p = &event["payload"];
            let task_id = p["task_id"].as_str().unwrap_or("").to_string();
            let item_type = p["item_type"].as_str().unwrap_or("").to_string();
            if !task_id.is_empty() && !item_type.is_empty() {
                all.push(RecordedItem {
                    item_id: task_id,
                    item_type,
                    description: p["description"].as_str().unwrap_or("").to_string(),
                    ..Default::default()
                });
            }
        }
    }
    Ok(all)
}

/// Read the current marker for a task instance: latest TaskMarkerUpdated.new_marker,
/// falling back to TaskAdded.initial_marker if no update exists.
fn current_task_marker(task_id: &str) -> Result<Option<String>> {
    let mut current: Option<String> = None;
    for event in open_event_log(Path::new(EVENTS_FILE))? {
        let event = event?;
        if event["source_module"].as_str() != Some("task_model") { continue; }
        match event["event_type"].as_str() {
            Some("TaskAdded")
                if event["payload"]["task_id"].as_str() == Some(task_id) =>
            {
                current = event["payload"]["initial_marker"].as_str().map(String::from);
            }
            Some("TaskMarkerUpdated")
                if event["payload"]["task_id"].as_str() == Some(task_id) =>
            {
                current = event["payload"]["new_marker"].as_str().map(String::from);
            }
            _ => {}
        }
    }
    Ok(current)
}

fn find_item(item_id: &str) -> Result<Option<RecordedItem>> {
    Ok(read_all_record_items()?.into_iter().find(|i| i.item_id == item_id))
}

fn current_status(item_id: &str) -> Result<Option<String>> {
    let mut last = None;
    for event in open_event_log(Path::new(EVENTS_FILE))? {
        let event = event?;
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

fn current_priority(item_id: &str) -> Result<Option<String>> {
    let mut last = None;
    for event in open_event_log(Path::new(EVENTS_FILE))? {
        let event = event?;
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

fn proposed_values_from_extraction(item_id: &str) -> Result<(Option<String>, Option<String>)> {
    let events: Vec<Value> = open_event_log(Path::new(EVENTS_FILE))?
        .collect::<Result<Vec<_>>>()?;

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
    let emitter = EventEmitter::new(Path::new(EVENTS_FILE), SOURCE_MODULE);

    // Schema load before any item_status event — abort if schema invalid (FP1: SchemaInvalid).
    // project_schema emits the failure event and prints to stderr.
    let schema = match load_and_validate(Path::new("."), Path::new(EVENTS_FILE), &correlation_id) {
        Some(s) => s,
        None => return Ok(()),
    };

    emitter.emit("StatusUpdateRequested", &correlation_id, json!({
        "item_id": item_id,
        "requested_status": status,
    }));

    // Contract failure: ItemNotFound
    let item = match find_item(item_id)? {
        Some(i) => i,
        None => {
            println!("Item '{}' not found in project record.", item_id);
            emitter.emit("StatusUpdateFailedItemNotFound", &correlation_id, json!({
                "failure_reason": "item_not_found",
                "item_id": item_id,
            }));
            return Ok(());
        }
    };

    // Contract failure: InvalidStatusForType — validated against active vocabulary.
    // Covers empty allowedStatuses (custom type with no status vocabulary).
    if !vocabulary_allows_status(&schema, &item.item_type, status) {
        let valid = valid_statuses_from_vocabulary(&schema, &item.item_type);
        let valid_display = if valid.is_empty() {
            "(none — type has no status vocabulary)".to_string()
        } else {
            valid.join(", ")
        };
        println!(
            "Status '{}' is not valid for item type '{}'. Valid values: {}",
            status, item.item_type, valid_display
        );
        emitter.emit("StatusUpdateFailedInvalidStatus", &correlation_id, json!({
            "failure_reason": "invalid_status_for_type",
            "item_id": item_id,
            "item_type": item.item_type,
            "requested_status": status,
        }));
        return Ok(());
    }

    let previous_status = current_status(item_id)?;

    emitter.emit("ItemStatusUpdated", &correlation_id, json!({
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
    let emitter = EventEmitter::new(Path::new(EVENTS_FILE), SOURCE_MODULE);

    // Schema load before any item_status event — abort if schema invalid (FP1: SchemaInvalid).
    // Priority values are not schema-driven in this release, but schema load is required
    // because item type resolution depends on a successfully loaded vocabulary.
    let _schema = match load_and_validate(Path::new("."), Path::new(EVENTS_FILE), &correlation_id) {
        Some(s) => s,
        None => return Ok(()),
    };

    emitter.emit("PriorityUpdateRequested", &correlation_id, json!({
        "item_id": item_id,
        "requested_priority": priority,
    }));

    // Contract failure: ItemNotFound
    let item = match find_item(item_id)? {
        Some(i) => i,
        None => {
            println!("Item '{}' not found in project record.", item_id);
            emitter.emit("PriorityUpdateFailedItemNotFound", &correlation_id, json!({
                "failure_reason": "item_not_found",
                "item_id": item_id,
            }));
            return Ok(());
        }
    };

    // Contract failure: InvalidPriorityValue (hardcoded — not schema-driven in this release)
    if !VALID_PRIORITIES.contains(&priority) {
        println!(
            "Priority '{}' is not valid. Valid values: {}",
            priority,
            VALID_PRIORITIES.join(", ")
        );
        emitter.emit("PriorityUpdateFailedInvalidValue", &correlation_id, json!({
            "failure_reason": "invalid_priority_value",
            "item_id": item_id,
            "requested_priority": priority,
        }));
        return Ok(());
    }

    let previous_priority = current_priority(item_id)?;

    emitter.emit("ItemPriorityUpdated", &correlation_id, json!({
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
    let emitter = EventEmitter::new(Path::new(EVENTS_FILE), SOURCE_MODULE);

    // Schema load before any item_status event — abort if schema invalid (FP1: SchemaInvalid).
    // Schema is required for marker resolution and stale-status detection.
    let schema = match load_and_validate(Path::new("."), Path::new(EVENTS_FILE), &correlation_id) {
        Some(s) => s,
        None => return Ok(()),
    };

    emitter.emit("ItemStatusQueried", &correlation_id, json!({
        "item_id": item_id,
    }));

    // Contract failure: ItemNotFound
    let item = match find_item(item_id)? {
        Some(i) => i,
        None => {
            println!("Item '{}' not found in project record.", item_id);
            emitter.emit("ItemStatusQueryFailedItemNotFound", &correlation_id, json!({
                "failure_reason": "item_not_found",
                "item_id": item_id,
            }));
            return Ok(());
        }
    };

    let explicit_status   = current_status(item_id)?;
    let explicit_priority = current_priority(item_id)?;
    let (prop_status, prop_priority) = proposed_values_from_extraction(item_id)?;

    // Effective status resolution chain (contract invariant, highest to lowest priority):
    //   1. explicit       — most recent ItemStatusUpdated event
    //   2. marker_derived — for block-type items: marker from TaskAdded/TaskMarkerUpdated;
    //                       for other items: marker extracted from description prefix
    //   3. proposed       — proposed_status from extraction
    //   4. null
    //
    // Representation Ban: is_block_type resolves via vocabulary API before branching.
    let (effective_status, status_source): (Option<String>, Option<&str>) =
        if let Some(s) = explicit_status {
            (Some(s), Some("explicit"))
        } else if is_block_type(&schema, &item.item_type) {
            // Task-type item: read marker from event log (TaskAdded + TaskMarkerUpdated)
            let marker = current_task_marker(item_id)?;
            match marker.as_deref().and_then(|m| marker_to_status(&schema, m)) {
                Some(mapped) => (Some(mapped.to_string()), Some("marker_derived")),
                None => (prop_status.clone(), prop_status.as_ref().map(|_| "proposed")),
            }
        } else if let Some(marker) = extract_marker(&item.description) {
            if let Some(mapped) = marker_to_status(&schema, &marker) {
                (Some(mapped.to_string()), Some("marker_derived"))
            } else {
                // Unmapped marker — fall through to proposed value, no failure signal
                (prop_status.clone(), prop_status.as_ref().map(|_| "proposed"))
            }
        } else {
            (prop_status.clone(), prop_status.as_ref().map(|_| "proposed"))
        };

    // Stale status check: only when the effective status is an explicit recorded value.
    // Emit ItemStatusUnrecognized (non-failure, exactly once per get) before ItemStatusReturned.
    if status_source == Some("explicit") {
        if let Some(ref s) = effective_status {
            if !vocabulary_allows_status(&schema, &item.item_type, s) {
                emitter.emit("ItemStatusUnrecognized", &correlation_id, json!({
                    "item_id": item_id,
                    "item_type": item.item_type,
                    "recorded_status": s,
                }));
                eprintln!(
                    "warning: recorded status '{}' for item '{}' is no longer recognized by the active vocabulary",
                    s, &item_id[..8.min(item_id.len())]
                );
            }
        }
    }

    let effective_priority = explicit_priority.as_ref().or(prop_priority.as_ref()).cloned();

    let status_display = match status_source {
        Some("explicit")       => effective_status.as_deref().unwrap_or("(not set)").to_string(),
        Some("marker_derived") => format!("{} (marker)", effective_status.as_deref().unwrap_or("")),
        Some("proposed")       => format!("{} (proposed)", effective_status.as_deref().unwrap_or("")),
        _                      => "(not set)".to_string(),
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

    emitter.emit("ItemStatusReturned", &correlation_id, json!({
        "item_id": item_id,
        "item_type": item.item_type,
        "current_status": effective_status,
        "current_priority": effective_priority,
        "status_source": status_source,
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

use anyhow::Result;
use clap::{Parser, Subcommand};
use lucid_core::{open_event_log, EventEmitter, EVENTS_FILE};
use project_schema::{canonical_task_block_type, load_and_validate};
use serde_json::{json, Value};
use std::path::Path;
use uuid::Uuid;

const SOURCE_MODULE: &str = "task_model";

#[derive(Parser)]
#[command(about = "LucidPM task instance manager")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a task instance in the project record
    Add {
        /// Task description
        #[arg(long)]
        description: String,
        /// Parent project record item ID
        #[arg(long)]
        parent: String,
        /// Initial task marker (e.g. TODO, DOING, DONE); defaults to the
        /// first marker defined in the vocabulary
        #[arg(long)]
        marker: Option<String>,
    },
}

/// Read every item currently in the project record (extraction-based and task instances).
/// Returns item_ids.
fn all_record_item_ids() -> Result<Vec<String>> {
    let events: Vec<Value> = open_event_log(Path::new(EVENTS_FILE))?
        .filter_map(|r| r.ok())
        .collect();

    let mut ids: Vec<String> = Vec::new();

    // Extraction-based items via incorporated sessions
    let sessions: Vec<String> = events.iter()
        .filter(|e| {
            e["source_module"].as_str() == Some("project_state")
                && e["event_type"].as_str() == Some("ItemsIncorporated")
        })
        .filter_map(|e| e["payload"]["session_id"].as_str().map(String::from))
        .collect();

    for session_id in &sessions {
        let mut extracted_items: Option<Vec<Value>> = None;
        let mut accepted_ids: Option<Vec<String>> = None;

        for e in &events {
            if e["correlation_id"].as_str() != Some(session_id.as_str()) { continue; }
            match e["event_type"].as_str() {
                Some("ItemsExtracted") => {
                    extracted_items = e["payload"]["items"].as_array().cloned();
                }
                Some("ExtractionConfirmed") => {
                    accepted_ids = e["payload"]["accepted_item_ids"]
                        .as_array()
                        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect());
                }
                _ => {}
            }
        }

        let raw = extracted_items.unwrap_or_default();
        let accepted = accepted_ids.unwrap_or_default();
        for item in raw {
            if let Some(id) = item["item_id"].as_str() {
                if accepted.contains(&id.to_string()) {
                    ids.push(id.to_string());
                }
            }
        }
    }

    // Task instances from TaskAdded events
    for e in &events {
        if e["source_module"].as_str() == Some(SOURCE_MODULE)
            && e["event_type"].as_str() == Some("TaskAdded")
        {
            if let Some(task_id) = e["payload"]["task_id"].as_str() {
                if !task_id.is_empty() {
                    ids.push(task_id.to_string());
                }
            }
        }
    }

    Ok(ids)
}

fn cmd_add(description: &str, parent_id: &str, requested_marker: Option<&str>) -> Result<()> {
    let correlation_id = Uuid::new_v4().to_string();
    let emitter = EventEmitter::new(Path::new(EVENTS_FILE), SOURCE_MODULE);

    // Emit OBSERVATIONAL event before any validation (contract: TaskAddRequested always first)
    emitter.emit("TaskAddRequested", &correlation_id, json!({
        "description":     description,
        "parent_item_id":  parent_id,
        "requested_marker": requested_marker,
    }));

    // Load and validate schema — project_schema emits cross-module events on failure.
    // Failure Path 2: SchemaInvalid
    let schema = match load_and_validate(Path::new("."), Path::new(EVENTS_FILE), &correlation_id) {
        Some(s) => s,
        None => {
            emitter.emit("TaskAddFailedSchemaInvalid", &correlation_id, json!({
                "failure_reason": "schema_invalid",
            }));
            return Ok(());
        }
    };

    // Failure Path 3: TaskTypeNotDefined — vocabulary has no block type with markers.
    // Representation Ban: canonical_task_block_type resolves via vocabulary API.
    let (canonical_type, markers) = match canonical_task_block_type(&schema) {
        Some(t) => t,
        None => {
            eprintln!("error: active vocabulary defines no task block type");
            emitter.emit("TaskAddFailedTaskTypeNotDefined", &correlation_id, json!({
                "failure_reason": "task_type_not_defined",
            }));
            return Ok(());
        }
    };

    // Determine the initial marker.
    // If the PM specified one, use it if it is vocabulary-defined; otherwise default.
    // Contract does not define a failure for unrecognized marker — silently fall back to default.
    let initial_marker = if let Some(req) = requested_marker {
        if markers.contains_key(req) {
            req.to_string()
        } else {
            // Fall back to first marker alphabetically
            let mut sorted_markers: Vec<&str> = markers.keys().map(|s| s.as_str()).collect();
            sorted_markers.sort();
            sorted_markers.first().copied().unwrap_or("TODO").to_string()
        }
    } else {
        let mut sorted_markers: Vec<&str> = markers.keys().map(|s| s.as_str()).collect();
        sorted_markers.sort();
        sorted_markers.first().copied().unwrap_or("TODO").to_string()
    };

    // Failure Path 1: ParentNotFound — parent item must be in the project record.
    let known_ids = all_record_item_ids()?;
    if !known_ids.contains(&parent_id.to_string()) {
        eprintln!("error: parent item '{}' not found in project record", parent_id);
        emitter.emit("TaskAddFailedParentNotFound", &correlation_id, json!({
            "failure_reason":  "parent_not_found",
            "parent_item_id":  parent_id,
        }));
        return Ok(());
    }

    // All preconditions satisfied — create the task instance.
    let task_id = Uuid::new_v4().to_string();

    emitter.emit("TaskAdded", &correlation_id, json!({
        "task_id":         task_id,
        "item_type":       canonical_type,
        "description":     description,
        "parent_item_id":  parent_id,
        "initial_marker":  initial_marker,
    }));

    println!(
        "Task created: [{}] {} (parent: {}...)",
        canonical_type,
        description,
        &parent_id[..8.min(parent_id.len())]
    );

    Ok(())
}

fn main() {
    let cli = Cli::parse();
    let result = match &cli.command {
        Commands::Add { description, parent, marker } => {
            cmd_add(description, parent, marker.as_deref())
        }
    };
    if let Err(e) = result {
        eprintln!("Error: {:#}", e);
        std::process::exit(1);
    }
}

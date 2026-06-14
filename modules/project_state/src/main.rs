use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use lucid_core::{open_event_log, EventEmitter, RecordedItem, EVENTS_FILE};
use project_schema::{emit_type_unknown, load_and_validate, resolve_type};
use serde_json::{json, Value};
use std::path::Path;
use uuid::Uuid;

const SOURCE_MODULE: &str = "project_state";

#[derive(Parser)]
#[command(about = "LucidPM project state manager")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Incorporate items from a confirmed extraction session into the project record
    Incorporate {
        /// Session ID — the correlation_id from pm_structuring's ExtractionConfirmed event
        session_id: String,
    },
    /// Incorporate the most recently confirmed session that has not yet been incorporated
    IncorporateLatest,
    /// View all recorded project items
    View,
}

/// Scan runtime_events.jsonl for all ItemsIncorporated events from project_state.
/// Returns (session_id, incorporated_count) in emission order.
fn read_incorporated_sessions() -> Result<Vec<(String, u64)>> {
    let mut sessions = Vec::new();
    for event in open_event_log(Path::new(EVENTS_FILE))? {
        let event = event?;
        if event["source_module"].as_str() == Some("project_state")
            && event["event_type"].as_str() == Some("ItemsIncorporated")
        {
            let session_id = event["payload"]["session_id"]
                .as_str().unwrap_or("").to_string();
            let count = event["payload"]["incorporated_count"].as_u64().unwrap_or(0);
            sessions.push((session_id, count));
        }
    }
    Ok(sessions)
}

// Reads runtime_events.jsonl to find ItemsExtracted + ExtractionConfirmed for a session.
fn find_confirmed_items(session_id: &str) -> Result<Vec<RecordedItem>> {
    let mut items_extracted: Option<Vec<Value>> = None;
    let mut accepted_ids: Option<Vec<String>> = None;

    for event in open_event_log(Path::new(EVENTS_FILE))? {
        let event = event.context("reading runtime events")?;
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

    let raw_items = items_extracted.ok_or_else(|| {
        anyhow::anyhow!("No ItemsExtracted event found for session '{}'", session_id)
    })?;
    let accepted = accepted_ids.ok_or_else(|| {
        anyhow::anyhow!(
            "No ExtractionConfirmed event found for session '{}' — session may not be confirmed",
            session_id
        )
    })?;

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
            uncertain: item["uncertain"].as_bool().unwrap_or(false),
            uncertainty_reason: item["uncertainty_reason"].as_str().map(String::from),
            session_id: session_id.to_string(),
            parent_item_id: item["parent_item_id"].as_str().map(String::from),
            ..Default::default()
        })
        .collect();

    Ok(items)
}

/// Read task instances from TaskAdded events in the event log.
fn read_task_items() -> Result<Vec<RecordedItem>> {
    let mut items = Vec::new();
    for event in open_event_log(Path::new(EVENTS_FILE))? {
        let event = event?;
        if event["source_module"].as_str() == Some("task_model")
            && event["event_type"].as_str() == Some("TaskAdded")
        {
            let p = &event["payload"];
            let task_id = p["task_id"].as_str().unwrap_or("").to_string();
            let item_type = p["item_type"].as_str().unwrap_or("").to_string();
            if task_id.is_empty() || item_type.is_empty() { continue; }
            items.push(RecordedItem {
                item_id: task_id,
                item_type,
                description: p["description"].as_str().unwrap_or("").to_string(),
                session_id: "task_model".to_string(),
                parent_item_id: p["parent_item_id"].as_str().map(String::from),
                ..Default::default()
            });
        }
    }
    Ok(items)
}

fn cmd_incorporate(session_id: &str) -> Result<()> {
    let correlation_id = Uuid::new_v4().to_string();
    let emitter = EventEmitter::new(Path::new(EVENTS_FILE), SOURCE_MODULE);

    emitter.emit("IncorporationRequested", &correlation_id, json!({
        "session_id": session_id,
    }));

    let prior_sessions = read_incorporated_sessions()?;

    // Contract failure: SessionAlreadyIncorporated
    if prior_sessions.iter().any(|(s, _)| s == session_id) {
        println!("Session '{}' has already been incorporated.", session_id);
        emitter.emit("IncorporationFailedDuplicate", &correlation_id, json!({
            "failure_reason": "session_already_incorporated",
            "session_id": session_id,
        }));
        return Ok(());
    }

    let new_items = find_confirmed_items(session_id)?;
    let incorporated_count = new_items.len() as u64;
    let prior_total: u64 = prior_sessions.iter().map(|(_, c)| c).sum();
    let total_record_size = prior_total + incorporated_count;
    let total_sessions = prior_sessions.len() + 1;

    emitter.emit("ItemsIncorporated", &correlation_id, json!({
        "session_id": session_id,
        "incorporated_count": incorporated_count,
        "total_record_size": total_record_size,
    }));

    println!(
        "Incorporated {} items from session '{}'.",
        incorporated_count,
        &session_id[..8.min(session_id.len())]
    );
    println!(
        "Project record now contains {} items across {} session(s).",
        total_record_size,
        total_sessions,
    );

    Ok(())
}

fn read_all_confirmed_session_ids() -> Result<Vec<String>> {
    let mut ids = Vec::new();
    for event in open_event_log(Path::new(EVENTS_FILE))? {
        let event = event?;
        if event["event_type"].as_str() == Some("ExtractionConfirmed") {
            if let Some(cid) = event["correlation_id"].as_str() {
                ids.push(cid.to_string());
            }
        }
    }
    Ok(ids)
}

fn cmd_incorporate_latest() -> Result<()> {
    let confirmed = read_all_confirmed_session_ids()?;
    let incorporated: std::collections::HashSet<String> = read_incorporated_sessions()?
        .into_iter().map(|(s, _)| s).collect();
    let latest = confirmed.into_iter().rev().find(|s| !incorporated.contains(s));
    match latest {
        Some(session_id) => cmd_incorporate(&session_id),
        None => {
            println!("No unincorporated sessions found.");
            Ok(())
        }
    }
}

fn cmd_view() -> Result<()> {
    let correlation_id = Uuid::new_v4().to_string();
    let emitter = EventEmitter::new(Path::new(EVENTS_FILE), SOURCE_MODULE);

    emitter.emit("RecordQueried", &correlation_id, json!({}));

    // Contract failure: SchemaLoadFailed (R10)
    // load_and_validate emits cross-module project_schema events on error.
    let schema = match load_and_validate(Path::new("."), Path::new(EVENTS_FILE), &correlation_id) {
        Some(s) => s,
        None => {
            emitter.emit("RecordQueryFailedSchemaInvalid", &correlation_id, json!({
                "failure_reason": "schema_load_failed",
            }));
            return Ok(());
        }
    };

    let sessions = read_incorporated_sessions()?;
    let task_items = read_task_items()?;

    // Contract failure: EmptyRecord — no items of any kind in the project record
    if sessions.is_empty() && task_items.is_empty() {
        println!("The project record is empty. Run 'incorporate <session_id>' to add items.");
        emitter.emit("RecordQueryFailedEmpty", &correlation_id, json!({
            "failure_reason": "record_empty",
        }));
        return Ok(());
    }

    let mut all_items: Vec<RecordedItem> = Vec::new();
    for (session_id, _) in &sessions {
        let items = find_confirmed_items(session_id)?;
        all_items.extend(items);
    }
    all_items.extend(task_items);

    let session_count = sessions.len();
    // total_count = total items in the project record (pre-exclusion); unchanged semantics.
    let total_count = all_items.len() as u64;

    // Concept Dependency: resolve each item's stored type to its vocabulary concept.
    // Unrecognized types produce SchemaTypeUnknown (via project_schema library) and are
    // excluded. Recognized types are displayed using the canonical name.
    let mut recognized: Vec<(&RecordedItem, &str)> = Vec::new();
    for item in &all_items {
        match resolve_type(&schema, &item.item_type) {
            Some(canonical) => recognized.push((item, canonical)),
            None => emit_type_unknown(
                Path::new(EVENTS_FILE), &item.item_id, &item.item_type, &correlation_id,
            ),
        }
    }

    println!("\n=== Project Record ({} items across {} session(s)) ===\n",
        total_count, session_count);

    let mut current_session = String::new();
    for (item, canonical) in &recognized {
        if item.session_id != current_session {
            current_session = item.session_id.clone();
            println!("── Session: {} ──", &current_session[..8.min(current_session.len())]);
        }
        let uncertainty_marker = if item.uncertain { " [UNCERTAIN]" } else { "" };
        println!("  [{}]{} {}", canonical.to_uppercase(), uncertainty_marker, item.description);
        if let Some(reason) = &item.uncertainty_reason {
            println!("    Uncertainty: {}", reason);
        }
        if let Some(parent) = &item.parent_item_id {
            println!("    Parent: {}...", &parent[..8.min(parent.len())]);
        }
    }
    println!();

    let items_payload: Vec<Value> = recognized.iter().map(|(i, canonical)| json!({
        "item_id": i.item_id,
        "item_type": canonical,   // canonical name, not stored representation
        "description": i.description,
        "uncertain": i.uncertain,
        "uncertainty_reason": i.uncertainty_reason,
        "session_id": i.session_id,
        "parent_item_id": i.parent_item_id,
    })).collect();

    emitter.emit("RecordReturned", &correlation_id, json!({
        "items": items_payload,
        "total_count": total_count,  // total in record (pre-exclusion)
        "session_count": session_count,
    }));

    Ok(())
}

fn main() {
    let cli = Cli::parse();
    let result = match &cli.command {
        Commands::Incorporate { session_id } => cmd_incorporate(session_id),
        Commands::IncorporateLatest => cmd_incorporate_latest(),
        Commands::View => cmd_view(),
    };
    if let Err(e) = result {
        eprintln!("Error: {:#}", e);
        std::process::exit(1);
    }
}

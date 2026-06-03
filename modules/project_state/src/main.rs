use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use project_schema::{emit_type_unknown, load_and_validate, resolve_type};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const EVENTS_FILE: &str = "events/runtime_events.jsonl";
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
    /// View all recorded project items
    View,
}

#[derive(Serialize, Deserialize, Clone)]
struct RecordedItem {
    item_id: String,
    item_type: String,
    description: String,
    uncertain: bool,
    uncertainty_reason: Option<String>,
    session_id: String,
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

/// Scan runtime_events.jsonl for all ItemsIncorporated events from project_state.
/// Returns (session_id, incorporated_count) in emission order.
fn read_incorporated_sessions() -> Result<Vec<(String, u64)>> {
    if !std::path::Path::new(EVENTS_FILE).exists() {
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
        })
        .collect();

    Ok(items)
}

fn cmd_incorporate(session_id: &str) -> Result<()> {
    let correlation_id = Uuid::new_v4().to_string();

    emit_event("IncorporationRequested", &correlation_id, json!({
        "session_id": session_id,
    }));

    let prior_sessions = read_incorporated_sessions()?;

    // Contract failure: SessionAlreadyIncorporated
    if prior_sessions.iter().any(|(s, _)| s == session_id) {
        println!("Session '{}' has already been incorporated.", session_id);
        emit_event("IncorporationFailedDuplicate", &correlation_id, json!({
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

    emit_event("ItemsIncorporated", &correlation_id, json!({
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

fn cmd_view() -> Result<()> {
    let correlation_id = Uuid::new_v4().to_string();

    emit_event("RecordQueried", &correlation_id, json!({}));

    // Contract failure: SchemaLoadFailed (R10)
    // load_and_validate emits cross-module project_schema events on error.
    let schema = match load_and_validate(Path::new("."), Path::new(EVENTS_FILE), &correlation_id) {
        Some(s) => s,
        None => {
            emit_event("RecordQueryFailedSchemaInvalid", &correlation_id, json!({
                "failure_reason": "schema_load_failed",
            }));
            return Ok(());
        }
    };

    let sessions = read_incorporated_sessions()?;

    // Contract failure: EmptyRecord
    if sessions.is_empty() {
        println!("The project record is empty. Run 'incorporate <session_id>' to add items.");
        emit_event("RecordQueryFailedEmpty", &correlation_id, json!({
            "failure_reason": "record_empty",
        }));
        return Ok(());
    }

    let mut all_items: Vec<RecordedItem> = Vec::new();
    for (session_id, _) in &sessions {
        let items = find_confirmed_items(session_id)?;
        all_items.extend(items);
    }

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
    }
    println!();

    let items_payload: Vec<Value> = recognized.iter().map(|(i, canonical)| json!({
        "item_id": i.item_id,
        "item_type": canonical,   // canonical name, not stored representation
        "description": i.description,
        "uncertain": i.uncertain,
        "uncertainty_reason": i.uncertainty_reason,
        "session_id": i.session_id,
    })).collect();

    emit_event("RecordReturned", &correlation_id, json!({
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
        Commands::View => cmd_view(),
    };
    if let Err(e) = result {
        eprintln!("Error: {:#}", e);
        std::process::exit(1);
    }
}

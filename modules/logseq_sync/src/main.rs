use anyhow::{Context, Result};
use clap::Parser;
use project_schema::{is_block_type, load_and_validate, marker_to_status, resolve_type, EventEnvelope, ProjectSchema};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::io::BufRead;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const EVENTS_FILE: &str = "events/runtime_events.jsonl";
const SOURCE_MODULE: &str = "logseq_sync";

#[derive(Parser)]
#[command(about = "Sync Logseq status/priority changes back into the project record")]
struct Cli {
    /// Path to the Logseq graph directory (pages must be at <graph_dir>/pages/)
    #[arg(long)]
    graph: String,
}

struct RecordedItem {
    item_id: String,
    item_type: String,
}

struct TaskRecord {
    task_id: String,
    item_type: String,
    #[allow(dead_code)]
    description: String,
    #[allow(dead_code)]
    parent_item_id: String,
    current_marker: String,
}

// Representation Ban: domain logic operates on vocabulary-resolved concept identity.
// resolve_type maps any stored representation (alias or canonical) to the concept
// before the status set lookup. No string literal representing a vocabulary concept
// appears in domain logic below this boundary.
fn vocabulary_allows_status(schema: &ProjectSchema, item_type: &str, status: &str) -> bool {
    let canonical = match resolve_type(schema, item_type) {
        Some(t) => t,
        None => return false,
    };
    schema
        .page_types
        .get(canonical)
        .map(|def| !def.allowed_statuses.is_empty() && def.allowed_statuses.iter().any(|s| s == status))
        .unwrap_or(false)
}

const VALID_PRIORITIES: &[&str] = &["high", "medium", "low"];

fn emit_event(event_type: &str, correlation_id: &str, payload: Value) {
    project_schema::emit_event(Path::new(EVENTS_FILE), EventEnvelope {
        source_module: SOURCE_MODULE,
        event_type,
        correlation_id,
        payload,
    });
}

/// Emit a task_model event during sync. source_module is "task_model" so that
/// discovered tasks and marker updates are indistinguishable from direct-command
/// task_model events (contract indistinguishability invariant).
fn emit_task_event(event_type: &str, correlation_id: &str, payload: Value) {
    project_schema::emit_event(Path::new(EVENTS_FILE), EventEnvelope {
        source_module: "task_model",
        event_type,
        correlation_id,
        payload,
    });
}

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
    let accepted  = accepted_ids.unwrap_or_default();

    let items = raw_items
        .into_iter()
        .filter(|item| {
            item["item_id"].as_str()
                .map(|id| accepted.contains(&id.to_string()))
                .unwrap_or(false)
        })
        .map(|item| RecordedItem {
            item_id:   item["item_id"].as_str().unwrap_or("").to_string(),
            item_type: item["item_type"].as_str().unwrap_or("").to_string(),
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
    // Task instances
    if Path::new(EVENTS_FILE).exists() {
        let file = fs::File::open(EVENTS_FILE).context("opening events file")?;
        for line in std::io::BufReader::new(file).lines() {
            let line = line.context("reading events file")?;
            if line.trim().is_empty() { continue; }
            let event: Value = serde_json::from_str(&line).context("parsing event line")?;
            if event["source_module"].as_str() == Some("task_model")
                && event["event_type"].as_str() == Some("TaskAdded")
            {
                let p = &event["payload"];
                let task_id = p["task_id"].as_str().unwrap_or("").to_string();
                let item_type = p["item_type"].as_str().unwrap_or("").to_string();
                if !task_id.is_empty() && !item_type.is_empty() {
                    all.push(RecordedItem { item_id: task_id, item_type });
                }
            }
        }
    }
    Ok(all)
}

/// Read all task instances with their current markers.
fn read_all_task_records() -> Result<Vec<TaskRecord>> {
    if !Path::new(EVENTS_FILE).exists() {
        return Ok(vec![]);
    }
    let file = fs::File::open(EVENTS_FILE).context("opening events file")?;
    let events: Vec<Value> = std::io::BufReader::new(file)
        .lines()
        .filter_map(|l| l.ok())
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str(&l).ok())
        .collect();

    let mut tasks: Vec<TaskRecord> = events.iter()
        .filter(|e| {
            e["source_module"].as_str() == Some("task_model")
                && e["event_type"].as_str() == Some("TaskAdded")
        })
        .filter_map(|e| {
            let p = &e["payload"];
            let task_id = p["task_id"].as_str()?.to_string();
            let item_type = p["item_type"].as_str()?.to_string();
            let parent_item_id = p["parent_item_id"].as_str()?.to_string();
            let current_marker = p["initial_marker"].as_str().unwrap_or("TODO").to_string();
            Some(TaskRecord {
                task_id,
                item_type,
                description: p["description"].as_str().unwrap_or("").to_string(),
                parent_item_id,
                current_marker,
            })
        })
        .collect();

    // Apply TaskMarkerUpdated events
    for e in &events {
        if e["source_module"].as_str() == Some("task_model")
            && e["event_type"].as_str() == Some("TaskMarkerUpdated")
        {
            if let Some(task_id) = e["payload"]["task_id"].as_str() {
                if let Some(task) = tasks.iter_mut().find(|t| t.task_id == task_id) {
                    if let Some(new_marker) = e["payload"]["new_marker"].as_str() {
                        task.current_marker = new_marker.to_string();
                    }
                }
            }
        }
    }

    Ok(tasks)
}

/// Parse task block lines from Logseq page content.
/// Returns (marker, task_id, description) tuples.
/// Format: `    - <MARKER> task-id: <uuid> <description>`
fn parse_task_block_lines(content: &str) -> Vec<(String, String, String)> {
    let mut results = Vec::new();
    for line in content.lines() {
        // Look for task-id: annotation anywhere in the line
        if let Some(tid_pos) = line.find("task-id: ") {
            let after_tid = &line[tid_pos + "task-id: ".len()..];
            let mut parts = after_tid.splitn(2, ' ');
            let task_id = parts.next().unwrap_or("").trim().to_string();
            let description = parts.next().unwrap_or("").trim().to_string();
            if task_id.is_empty() { continue; }
            // Extract marker: first uppercase token after `- ` (possibly with leading whitespace)
            let trimmed = line.trim_start();
            let marker = trimmed
                .strip_prefix("- ")
                .and_then(|rest| rest.split_whitespace().next())
                .unwrap_or("")
                .to_string();
            if !marker.is_empty() {
                results.push((marker, task_id, description));
            }
        }
    }
    results
}

/// Most recent explicit status for item_id, from item_status or logseq_sync.
fn current_explicit_status(item_id: &str) -> Result<Option<String>> {
    if !Path::new(EVENTS_FILE).exists() { return Ok(None); }
    let file = fs::File::open(EVENTS_FILE).context("opening events file")?;
    let mut last = None;
    for line in std::io::BufReader::new(file).lines() {
        let line = line.context("reading events file")?;
        if line.trim().is_empty() { continue; }
        let event: Value = serde_json::from_str(&line).context("parsing event line")?;
        let src = event["source_module"].as_str().unwrap_or("");
        if (src == "item_status" || src == "logseq_sync")
            && event["event_type"].as_str() == Some("ItemStatusUpdated")
            && event["payload"]["item_id"].as_str() == Some(item_id)
        {
            last = event["payload"]["new_status"].as_str().map(String::from);
        }
    }
    Ok(last)
}

/// Most recent explicit priority for item_id, from item_status or logseq_sync.
fn current_explicit_priority(item_id: &str) -> Result<Option<String>> {
    if !Path::new(EVENTS_FILE).exists() { return Ok(None); }
    let file = fs::File::open(EVENTS_FILE).context("opening events file")?;
    let mut last = None;
    for line in std::io::BufReader::new(file).lines() {
        let line = line.context("reading events file")?;
        if line.trim().is_empty() { continue; }
        let event: Value = serde_json::from_str(&line).context("parsing event line")?;
        let src = event["source_module"].as_str().unwrap_or("");
        if (src == "item_status" || src == "logseq_sync")
            && event["event_type"].as_str() == Some("ItemPriorityUpdated")
            && event["payload"]["item_id"].as_str() == Some(item_id)
        {
            last = event["payload"]["new_priority"].as_str().map(String::from);
        }
    }
    Ok(last)
}

/// Proposed status/priority from a confirmed extraction, if present.
fn proposed_values(item_id: &str) -> Result<(Option<String>, Option<String>)> {
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
        let items   = e["payload"]["items"].as_array()?;
        let item    = items.iter().find(|i| i["item_id"].as_str() == Some(item_id))?;
        Some((
            corr_id.to_string(),
            item["proposed_status"].as_str().map(String::from),
            item["proposed_priority"].as_str().map(String::from),
        ))
    });

    let (corr_id, proposed_status, proposed_priority) = match candidate {
        Some(c) => c,
        None    => return Ok((None, None)),
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

    if confirmed { Ok((proposed_status, proposed_priority)) } else { Ok((None, None)) }
}

fn effective_status(item_id: &str) -> Result<Option<String>> {
    let explicit = current_explicit_status(item_id)?;
    if explicit.is_some() { return Ok(explicit); }
    let (proposed, _) = proposed_values(item_id)?;
    Ok(proposed)
}

fn effective_priority(item_id: &str) -> Result<Option<String>> {
    let explicit = current_explicit_priority(item_id)?;
    if explicit.is_some() { return Ok(explicit); }
    let (_, proposed) = proposed_values(item_id)?;
    Ok(proposed)
}

/// Scan all .md files in pages_dir and build a UUID → file path map by reading
/// the `- item-id: <uuid>` bullet from each page.
fn build_item_page_map(pages_dir: &Path) -> HashMap<String, PathBuf> {
    let mut map = HashMap::new();
    let entries = match fs::read_dir(pages_dir) {
        Ok(e) => e,
        Err(_) => return map,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        for line in content.lines() {
            if let Some(rest) = line.strip_prefix("- item-id: ") {
                let uuid = rest.trim().to_string();
                if !uuid.is_empty() {
                    map.insert(uuid, path.clone());
                    break;
                }
            }
        }
    }
    map
}

/// Parse `status::` and `priority::` property values from a Logseq page.
/// Returns (status, priority); "not-set" is treated as absent.
fn parse_page_properties(content: &str) -> (Option<String>, Option<String>) {
    let mut status   = None;
    let mut priority = None;
    for line in content.lines() {
        if let Some(val) = line.strip_prefix("status:: ") {
            let val = val.trim();
            if val != "not-set" { status = Some(val.to_string()); }
        }
        if let Some(val) = line.strip_prefix("priority:: ") {
            let val = val.trim();
            if val != "not-set" { priority = Some(val.to_string()); }
        }
    }
    (status, priority)
}

fn cmd_sync(graph_dir: &str) -> Result<()> {
    let correlation_id = Uuid::new_v4().to_string();

    emit_event("SyncRequested", &correlation_id, json!({
        "graph_dir": graph_dir,
    }));

    // Contract failure: SchemaLoadFailed (R9)
    // load_and_validate emits cross-module project_schema events on error.
    let schema = match load_and_validate(Path::new("."), Path::new(EVENTS_FILE), &correlation_id) {
        Some(s) => s,
        None => {
            emit_event("SyncFailedSchemaInvalid", &correlation_id, json!({
                "failure_reason": "schema_load_failed",
            }));
            return Ok(());
        }
    };

    // Contract failure: GraphNotAccessible
    let pages_dir = PathBuf::from(graph_dir).join("pages");
    if fs::read_dir(&pages_dir).is_err() {
        eprintln!("Logseq graph not accessible: '{}'", pages_dir.display());
        emit_event("SyncFailedGraphNotAccessible", &correlation_id, json!({
            "failure_reason": "graph_not_accessible",
            "graph_dir": graph_dir,
        }));
        return Ok(());
    }

    // Read project record
    let items = read_all_record_items()?;

    // Contract failure: ProjectRecordEmpty
    if items.is_empty() {
        eprintln!("Project record is empty. Nothing to sync.");
        emit_event("SyncFailedEmptyRecord", &correlation_id, json!({
            "failure_reason": "empty_project_record",
        }));
        return Ok(());
    }

    let mut changes_applied: u32 = 0;
    let mut items_skipped:   u32 = 0;
    let mut any_difference        = false;

    let page_map = build_item_page_map(&pages_dir);

    for item in &items {
        let page_path = match page_map.get(&item.item_id) {
            Some(p) => p.clone(),
            None    => continue, // page not yet exported — silently skip
        };

        let content = fs::read_to_string(&page_path)
            .with_context(|| format!("reading page {}", page_path.display()))?;

        let (logseq_status, logseq_priority) = parse_page_properties(&content);
        let eff_status   = effective_status(&item.item_id)?;
        let eff_priority = effective_priority(&item.item_id)?;

        // Status sync
        if let Some(ref ls) = logseq_status {
            if Some(ls.as_str()) != eff_status.as_deref() {
                any_difference = true;
                if !vocabulary_allows_status(&schema, &item.item_type, ls.as_str()) {
                    eprintln!(
                        "Skipping {}: '{}' is not valid for type '{}'",
                        &item.item_id[..8.min(item.item_id.len())], ls, item.item_type
                    );
                    emit_event("ItemSyncSkippedInvalidStatus", &correlation_id, json!({
                        "failure_reason": "invalid_status_for_type",
                        "item_id":        item.item_id,
                        "item_type":      item.item_type,
                        "rejected_status": ls,
                    }));
                    items_skipped += 1;
                } else {
                    emit_event("ItemStatusUpdated", &correlation_id, json!({
                        "item_id":         item.item_id,
                        "item_type":       item.item_type,
                        "new_status":      ls,
                        "previous_status": eff_status,
                    }));
                    changes_applied += 1;
                    println!(
                        "Updated status: {} ({}) → {}",
                        &item.item_id[..8.min(item.item_id.len())], item.item_type, ls
                    );
                }
            }
        }

        // Priority sync — invalid priority silently skipped (not a contract failure)
        if let Some(ref lp) = logseq_priority {
            if Some(lp.as_str()) != eff_priority.as_deref() {
                any_difference = true;
                if VALID_PRIORITIES.contains(&lp.as_str()) {
                    emit_event("ItemPriorityUpdated", &correlation_id, json!({
                        "item_id":           item.item_id,
                        "item_type":         item.item_type,
                        "new_priority":      lp,
                        "previous_priority": eff_priority,
                    }));
                    changes_applied += 1;
                    println!(
                        "Updated priority: {} ({}) → {}",
                        &item.item_id[..8.min(item.item_id.len())], item.item_type, lp
                    );
                }
            }
        }
    }

    // Scan task block lines in each page (logseq_sync task amendment).
    // Representation Ban: is_block_type + marker_to_status resolve via vocabulary API.
    let task_records = read_all_task_records()?;
    let known_task_ids: std::collections::HashSet<String> =
        task_records.iter().map(|t| t.task_id.clone()).collect();

    // Build item_id → parent_item_id map for discovering tasks' parent context
    let item_page_map_keys: Vec<String> = page_map.keys().cloned().collect();

    for parent_item_id in &item_page_map_keys {
        let page_path = match page_map.get(parent_item_id.as_str()) {
            Some(p) => p.clone(),
            None => continue,
        };
        let content = fs::read_to_string(&page_path)
            .with_context(|| format!("reading page {}", page_path.display()))?;

        let task_lines = parse_task_block_lines(&content);

        for (marker, task_id, description) in task_lines {
            // Contract: identifier-less lines silently skipped (already excluded by parse fn)
            // Representation Ban: marker_to_status uses vocabulary API
            if marker_to_status(&schema, &marker).is_none() {
                // TaskMarkerSyncSkipped — marker not vocabulary-recognized; task unchanged
                continue;
            }

            if known_task_ids.contains(&task_id) {
                // Known task — check if marker changed
                if let Some(task) = task_records.iter().find(|t| t.task_id == task_id) {
                    if task.current_marker != marker {
                        any_difference = true;
                        emit_task_event("TaskMarkerUpdated", &correlation_id, json!({
                            "task_id":         task_id,
                            "previous_marker": task.current_marker,
                            "new_marker":      marker,
                        }));
                        changes_applied += 1;
                        println!(
                            "Updated task marker: {}... {} → {}",
                            &task_id[..8.min(task_id.len())],
                            task.current_marker,
                            marker
                        );
                    }
                }
            } else {
                // Unknown task_id — discover as new task instance (HP6: discovery)
                // Determine canonical task type from the vocabulary
                let canonical_type = task_records.iter()
                    .find(|t| is_block_type(&schema, &t.item_type))
                    .map(|t| t.item_type.clone())
                    .or_else(|| {
                        schema.block_types.keys()
                            .find(|_| true)
                            .map(|k| k.to_string())
                    });

                if let Some(item_type) = canonical_type {
                    any_difference = true;
                    emit_task_event("TaskAdded", &correlation_id, json!({
                        "task_id":        task_id,
                        "item_type":      item_type,
                        "description":    description,
                        "parent_item_id": parent_item_id,
                        "initial_marker": marker,
                    }));
                    changes_applied += 1;
                    println!(
                        "Discovered task: {}... under {}...",
                        &task_id[..8.min(task_id.len())],
                        &parent_item_id[..8.min(parent_item_id.len())]
                    );
                }
            }
        }
    }

    if !any_difference {
        println!("No changes detected. Project record is up to date.");
        emit_event("SyncCompletedNoChanges", &correlation_id, json!({
            "graph_dir":     graph_dir,
            "items_checked": items.len() as u64,
        }));
    } else {
        println!(
            "Sync complete: {} change(s) applied, {} item(s) skipped.",
            changes_applied, items_skipped
        );
        emit_event("SyncCompleted", &correlation_id, json!({
            "graph_dir":       graph_dir,
            "changes_applied": changes_applied,
            "items_skipped":   items_skipped,
        }));
    }

    Ok(())
}

fn main() {
    let cli = Cli::parse();
    if let Err(e) = cmd_sync(&cli.graph) {
        eprintln!("Error: {:#}", e);
        std::process::exit(1);
    }
}

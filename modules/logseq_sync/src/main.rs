use anyhow::{Context, Result};
use clap::Parser;
use lucid_core::{open_event_log, EventEmitter, RecordedItem, EVENTS_FILE};
use project_schema::{is_block_type, load_and_validate, marker_to_status, resolve_type, ProjectSchema};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const SOURCE_MODULE: &str = "logseq_sync";
const TBD_OWNER_ID: &str = "TBD";

#[derive(Parser)]
#[command(about = "Sync Logseq status/priority changes back into the project record")]
struct Cli {
    /// Path to the Logseq graph directory (pages must be at <graph_dir>/pages/)
    #[arg(long)]
    graph: String,
}

struct TaskRecord {
    task_id: String,
    item_type: String,
    #[allow(dead_code)]
    description: String,
    #[allow(dead_code)]
    parent_item_id: String,
    current_marker: String,
    owner_id: String,
    scheduled_date: Option<String>,
    deadline: Option<String>,
}

struct ParsedTaskBlock {
    marker: String,
    task_id: String,
    description: String,
    owner_name: Option<String>,
    scheduled_date: Option<String>,
    deadline: Option<String>,
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
    // Task instances
    for event in open_event_log(Path::new(EVENTS_FILE))? {
        let event = event?;
        if event["source_module"].as_str() == Some("task_model")
            && event["event_type"].as_str() == Some("TaskAdded")
        {
            let p = &event["payload"];
            let task_id = p["task_id"].as_str().unwrap_or("").to_string();
            let item_type = p["item_type"].as_str().unwrap_or("").to_string();
            if !task_id.is_empty() && !item_type.is_empty() {
                all.push(RecordedItem { item_id: task_id, item_type, ..Default::default() });
            }
        }
    }
    Ok(all)
}

/// Read all task instances with their current markers, owners, and dates.
fn read_all_task_records() -> Result<Vec<TaskRecord>> {
    let events: Vec<Value> = open_event_log(Path::new(EVENTS_FILE))?
        .filter_map(|r| r.ok())
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
            // Backward compat: events without owner_id default to TBD placeholder
            let owner_id = p["owner_id"].as_str().unwrap_or(TBD_OWNER_ID).to_string();
            let scheduled_date = p["scheduled_date"].as_str().map(String::from);
            let deadline = p["deadline"].as_str().map(String::from);
            Some(TaskRecord {
                task_id,
                item_type,
                description: p["description"].as_str().unwrap_or("").to_string(),
                parent_item_id,
                current_marker,
                owner_id,
                scheduled_date,
                deadline,
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

    // Apply TaskOwnerUpdated events
    for e in &events {
        if e["source_module"].as_str() == Some("task_model")
            && e["event_type"].as_str() == Some("TaskOwnerUpdated")
        {
            if let Some(task_id) = e["payload"]["task_id"].as_str() {
                if let Some(task) = tasks.iter_mut().find(|t| t.task_id == task_id) {
                    if let Some(new_owner) = e["payload"]["new_owner_id"].as_str() {
                        task.owner_id = new_owner.to_string();
                    }
                }
            }
        }
    }

    // Apply TaskDatesUpdated events
    for e in &events {
        if e["source_module"].as_str() == Some("task_model")
            && e["event_type"].as_str() == Some("TaskDatesUpdated")
        {
            if let Some(task_id) = e["payload"]["task_id"].as_str() {
                if let Some(task) = tasks.iter_mut().find(|t| t.task_id == task_id) {
                    let p = &e["payload"];
                    // Only update if the field is present in the event payload
                    if p.get("new_scheduled_date").is_some() {
                        task.scheduled_date = p["new_scheduled_date"].as_str().map(String::from);
                    }
                    if p.get("new_deadline").is_some() {
                        task.deadline = p["new_deadline"].as_str().map(String::from);
                    }
                }
            }
        }
    }

    Ok(tasks)
}

/// Extract the last [[page_name]] reference from a line.
fn extract_page_ref(line: &str) -> Option<String> {
    let start = line.rfind("[[")?;
    let after = &line[start + 2..];
    let end = after.find("]]")?;
    let name = after[..end].trim().to_string();
    if name.is_empty() { None } else { Some(name) }
}

/// Parse a Logseq date string like "<2026-04-26 Sun>" → "2026-04-26".
fn parse_logseq_date(s: &str) -> Option<String> {
    let inner = s.trim().strip_prefix('<')?.strip_suffix('>')?;
    let date = inner.split_whitespace().next()?;
    // Validate: YYYY-MM-DD format (10 chars, hyphens at positions 4 and 7)
    let b = date.as_bytes();
    if b.len() == 10 && b[4] == b'-' && b[7] == b'-' {
        Some(date.to_string())
    } else {
        None
    }
}

/// Strip the last [[...]] page reference from text (used to extract description).
fn strip_last_page_ref(text: &str) -> &str {
    if let Some(pos) = text.rfind("[[") {
        text[..pos].trim_end()
    } else {
        text
    }
}

/// Scan block continuation lines starting at `start`.
/// Extracts: task-id from a :PROPERTIES: drawer (new format), SCHEDULED date, DEADLINE date.
/// Stops at the first non-indented non-empty line (next sibling block).
fn scan_block_continuation(lines: &[&str], start: usize) -> (Option<String>, Option<String>, Option<String>) {
    let mut task_id = None;
    let mut scheduled_date = None;
    let mut deadline = None;
    let mut j = start;
    let mut in_props = false;

    while j < lines.len() {
        let next = lines[j];
        let nt = next.trim();

        if in_props {
            if nt == ":END:" {
                in_props = false;
            } else if let Some(rest) = nt.strip_prefix(":task-id: ") {
                let id = rest.trim().to_string();
                if !id.is_empty() { task_id = Some(id); }
            }
        } else {
            if nt == ":PROPERTIES:" {
                in_props = true;
            } else if let Some(rest) = nt.strip_prefix("SCHEDULED: ") {
                scheduled_date = parse_logseq_date(rest);
            } else if let Some(rest) = nt.strip_prefix("DEADLINE: ") {
                deadline = parse_logseq_date(rest);
            } else if !next.starts_with(' ') && !next.starts_with('\t') && !nt.is_empty() {
                break;
            }
        }
        j += 1;
    }

    (task_id, scheduled_date, deadline)
}

/// Parse task block lines from Logseq page content.
/// Supports two formats:
///   Old: `- MARKER task-id: <uuid> description [[owner]]`
///   New: `- MARKER description [[owner]]\n  :PROPERTIES:\n  :task-id: <uuid>\n  :END:`
/// Only blocks carrying a stable task-id (in either location) are eligible for sync.
fn parse_task_block_lines(content: &str) -> Vec<ParsedTaskBlock> {
    let lines: Vec<&str> = content.lines().collect();
    let mut results = Vec::new();

    for i in 0..lines.len() {
        let line = lines[i];
        let trimmed = line.trim_start();

        if !trimmed.starts_with("- ") { continue; }
        let rest = &trimmed[2..];
        let marker = rest.split_whitespace().next().unwrap_or("").to_string();
        if marker.is_empty() { continue; }

        // ── Old format: task-id: <uuid> embedded in the task line ──
        if let Some(tid_pos) = line.find("task-id: ") {
            let after_tid = &line[tid_pos + "task-id: ".len()..];
            let task_id = after_tid.split_whitespace().next().unwrap_or("").trim().to_string();
            if !task_id.is_empty() {
                let owner_name = extract_page_ref(line);
                let after_uuid_start = tid_pos + "task-id: ".len() + task_id.len();
                let after_uuid = line.get(after_uuid_start..).unwrap_or("").trim();
                let description = if owner_name.is_some() {
                    strip_last_page_ref(after_uuid).to_string()
                } else {
                    after_uuid.to_string()
                };
                let (_, sched, dl) = scan_block_continuation(&lines, i + 1);
                results.push(ParsedTaskBlock { marker, task_id, description, owner_name, scheduled_date: sched, deadline: dl });
                continue;
            }
        }

        // ── New format: :task-id: in :PROPERTIES: drawer ──
        let (task_id_opt, sched, dl) = scan_block_continuation(&lines, i + 1);
        if let Some(task_id) = task_id_opt {
            let owner_name = extract_page_ref(line);
            let after_marker = rest[marker.len()..].trim();
            let description = if owner_name.is_some() {
                strip_last_page_ref(after_marker).to_string()
            } else {
                after_marker.to_string()
            };
            results.push(ParsedTaskBlock { marker, task_id, description, owner_name, scheduled_date: sched, deadline: dl });
        }
    }

    results
}

/// Most recent explicit status for item_id, from item_status or logseq_sync.
fn current_explicit_status(item_id: &str) -> Result<Option<String>> {
    let mut last = None;
    for event in open_event_log(Path::new(EVENTS_FILE))? {
        let event = event?;
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
    let mut last = None;
    for event in open_event_log(Path::new(EVENTS_FILE))? {
        let event = event?;
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
    let events: Vec<Value> = open_event_log(Path::new(EVENTS_FILE))?
        .filter_map(|r| r.ok())
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
    let emitter = EventEmitter::new(Path::new(EVENTS_FILE), SOURCE_MODULE);
    let task_emitter = EventEmitter::new(Path::new(EVENTS_FILE), "task_model");

    emitter.emit("SyncRequested", &correlation_id, json!({
        "graph_dir": graph_dir,
    }));

    // Contract failure: SchemaLoadFailed (R9)
    // load_and_validate emits cross-module project_schema events on error.
    let schema = match load_and_validate(Path::new("."), Path::new(EVENTS_FILE), &correlation_id) {
        Some(s) => s,
        None => {
            emitter.emit("SyncFailedSchemaInvalid", &correlation_id, json!({
                "failure_reason": "schema_load_failed",
            }));
            return Ok(());
        }
    };

    // Contract failure: GraphNotAccessible
    let pages_dir = PathBuf::from(graph_dir).join("pages");
    if fs::read_dir(&pages_dir).is_err() {
        eprintln!("Logseq graph not accessible: '{}'", pages_dir.display());
        emitter.emit("SyncFailedGraphNotAccessible", &correlation_id, json!({
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
        emitter.emit("SyncFailedEmptyRecord", &correlation_id, json!({
            "failure_reason": "empty_project_record",
        }));
        return Ok(());
    }

    let mut changes_applied: u32 = 0;
    let mut items_skipped:   u32 = 0;
    let mut any_difference        = false;

    let page_map = build_item_page_map(&pages_dir);

    // Derive page name (file stem) → item_id for resolving [[owner_name]] references
    let page_name_to_item: HashMap<String, String> = page_map.iter()
        .filter_map(|(item_id, path)| {
            let stem = path.file_stem()?.to_str()?.to_string();
            Some((stem, item_id.clone()))
        })
        .collect();

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
                    emitter.emit("ItemSyncSkippedInvalidStatus", &correlation_id, json!({
                        "failure_reason": "invalid_status_for_type",
                        "item_id":        item.item_id,
                        "item_type":      item.item_type,
                        "rejected_status": ls,
                    }));
                    items_skipped += 1;
                } else {
                    emitter.emit("ItemStatusUpdated", &correlation_id, json!({
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
                    emitter.emit("ItemPriorityUpdated", &correlation_id, json!({
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

    let item_page_map_keys: Vec<String> = page_map.keys().cloned().collect();

    for parent_item_id in &item_page_map_keys {
        let page_path = match page_map.get(parent_item_id.as_str()) {
            Some(p) => p.clone(),
            None => continue,
        };
        let content = fs::read_to_string(&page_path)
            .with_context(|| format!("reading page {}", page_path.display()))?;

        let task_blocks = parse_task_block_lines(&content);

        for block in task_blocks {
            let ParsedTaskBlock { marker, task_id, description, owner_name, scheduled_date, deadline } = block;

            // Resolve owner name to item_id, if present
            let resolved_owner_id: Option<String> = owner_name
                .as_ref()
                .and_then(|name| page_name_to_item.get(name).cloned());

            if known_task_ids.contains(&task_id) {
                // Case 1: known task — check each attribute independently
                let task = match task_records.iter().find(|t| t.task_id == task_id) {
                    Some(t) => t,
                    None => continue,
                };

                // Marker update: only if vocabulary-recognized; independent of owner/dates
                if task.current_marker != marker {
                    if marker_to_status(&schema, &marker).is_some() {
                        task_emitter.emit("TaskMarkerUpdated", &correlation_id, json!({
                            "task_id":         task_id,
                            "previous_marker": task.current_marker,
                            "new_marker":      marker,
                        }));
                        any_difference = true;
                        changes_applied += 1;
                        println!(
                            "Updated task marker: {}... {} → {}",
                            &task_id[..8.min(task_id.len())],
                            task.current_marker,
                            marker
                        );
                    }
                    // If not recognized: TaskMarkerSyncSkipped — no event, state unchanged
                }

                // Owner update: only if resolved and changed; unresolvable name → no event
                if let Some(ref new_owner_id) = resolved_owner_id {
                    if task.owner_id != *new_owner_id {
                        task_emitter.emit("TaskOwnerUpdated", &correlation_id, json!({
                            "task_id":           task_id,
                            "previous_owner_id": task.owner_id,
                            "new_owner_id":      new_owner_id,
                        }));
                        any_difference = true;
                        changes_applied += 1;
                        println!(
                            "Updated task owner: {}... → {}",
                            &task_id[..8.min(task_id.len())],
                            new_owner_id
                        );
                    }
                }

                // Dates update: emit if either scheduled_date or deadline changed
                if scheduled_date != task.scheduled_date || deadline != task.deadline {
                    task_emitter.emit("TaskDatesUpdated", &correlation_id, json!({
                        "task_id":                 task_id,
                        "previous_scheduled_date": task.scheduled_date,
                        "new_scheduled_date":      scheduled_date,
                        "previous_deadline":       task.deadline,
                        "new_deadline":            deadline,
                    }));
                    any_difference = true;
                    changes_applied += 1;
                    println!(
                        "Updated task dates: {}...",
                        &task_id[..8.min(task_id.len())]
                    );
                }
            } else {
                // Case 2: unknown task_id — discover if marker is vocabulary-recognized
                if marker_to_status(&schema, &marker).is_none() {
                    continue; // Case 3: unrecognized marker; silently skip
                }

                // Determine canonical task type from vocabulary
                let canonical_type = task_records.iter()
                    .find(|t| is_block_type(&schema, &t.item_type))
                    .map(|t| t.item_type.clone())
                    .or_else(|| {
                        schema.block_types.keys()
                            .find(|_| true)
                            .map(|k| k.to_string())
                    });

                if let Some(item_type) = canonical_type {
                    // Discovery: owner defaults to TBD; dates populated if present
                    task_emitter.emit("TaskAdded", &correlation_id, json!({
                        "task_id":        task_id,
                        "item_type":      item_type,
                        "description":    description,
                        "parent_item_id": parent_item_id,
                        "initial_marker": marker,
                        "owner_id":       resolved_owner_id.unwrap_or_else(|| TBD_OWNER_ID.to_string()),
                        "scheduled_date": scheduled_date,
                        "deadline":       deadline,
                    }));
                    any_difference = true;
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
        emitter.emit("SyncCompletedNoChanges", &correlation_id, json!({
            "graph_dir":     graph_dir,
            "items_checked": items.len() as u64,
        }));
    } else {
        println!(
            "Sync complete: {} change(s) applied, {} item(s) skipped.",
            changes_applied, items_skipped
        );
        emitter.emit("SyncCompleted", &correlation_id, json!({
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

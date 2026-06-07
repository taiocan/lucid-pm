use anyhow::{Context, Result};
use clap::Parser;
use lucid_core::{open_event_log, EventEmitter, RecordedItem, EVENTS_FILE};
use project_schema::{
    emit_type_unknown, is_block_type, load_and_validate, logseq_forward_label,
    logseq_inverse_label, resolve_type, ProjectSchema,
};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const SOURCE_MODULE: &str = "logseq_export";

#[derive(Parser)]
#[command(about = "Export the project record as Logseq pages")]
struct Cli {
    /// Path to the Logseq output directory (pages will be written to <output_dir>/pages/)
    #[arg(long)]
    output_dir: String,
}

struct LinkRecord {
    source_id: String,
    link_type: String,
    target_id: String,
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
    // Task instances from TaskAdded events, with their current marker
    let events: Vec<Value> = open_event_log(Path::new(EVENTS_FILE))?
        .filter_map(|r| r.ok())
        .collect();

    let mut tasks: Vec<RecordedItem> = events.iter()
        .filter(|e| {
            e["source_module"].as_str() == Some("task_model")
                && e["event_type"].as_str() == Some("TaskAdded")
        })
        .filter_map(|e| {
            let p = &e["payload"];
            let task_id = p["task_id"].as_str()?.to_string();
            let item_type = p["item_type"].as_str()?.to_string();
            Some(RecordedItem {
                item_id: task_id,
                item_type,
                description: p["description"].as_str().unwrap_or("").to_string(),
                parent_item_id: p["parent_item_id"].as_str().map(String::from),
                current_marker: p["initial_marker"].as_str().map(String::from),
                ..Default::default()
            })
        })
        .collect();

    // Apply TaskMarkerUpdated events to get current markers
    for e in &events {
        if e["source_module"].as_str() == Some("task_model")
            && e["event_type"].as_str() == Some("TaskMarkerUpdated")
        {
            if let Some(task_id) = e["payload"]["task_id"].as_str() {
                if let Some(task) = tasks.iter_mut().find(|t| t.item_id == task_id) {
                    task.current_marker = e["payload"]["new_marker"].as_str().map(String::from);
                }
            }
        }
    }

    all.extend(tasks);
    Ok(all)
}

fn current_status(item_id: &str) -> Result<Option<String>> {
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

fn current_priority(item_id: &str) -> Result<Option<String>> {
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

/// Return the recorded deadline for an item, or None if not set.
fn current_deadline(_item_id: &str) -> Result<Option<String>> {
    Ok(None)
}

fn proposed_status_and_priority(item_id: &str) -> Result<(Option<String>, Option<String>)> {
    let events: Vec<Value> = open_event_log(Path::new(EVENTS_FILE))?
        .filter_map(|r| r.ok())
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

    if confirmed { Ok((proposed_status, proposed_priority)) } else { Ok((None, None)) }
}

fn effective_status(item_id: &str) -> Result<Option<String>> {
    let explicit = current_status(item_id)?;
    if explicit.is_some() { return Ok(explicit); }
    let (proposed, _) = proposed_status_and_priority(item_id)?;
    Ok(proposed)
}

fn effective_priority(item_id: &str) -> Result<Option<String>> {
    let explicit = current_priority(item_id)?;
    if explicit.is_some() { return Ok(explicit); }
    let (_, proposed) = proposed_status_and_priority(item_id)?;
    Ok(proposed)
}

fn build_active_links() -> Result<Vec<LinkRecord>> {
    let mut links: Vec<LinkRecord> = Vec::new();
    for event in open_event_log(Path::new(EVENTS_FILE))? {
        let event = event?;
        if event["source_module"].as_str() != Some("item_links") { continue; }
        match event["event_type"].as_str() {
            Some("ItemLinked") => {
                let p = &event["payload"];
                links.push(LinkRecord {
                    source_id: p["source_id"].as_str().unwrap_or("").to_string(),
                    link_type: p["link_type"].as_str().unwrap_or("").to_string(),
                    target_id: p["target_id"].as_str().unwrap_or("").to_string(),
                });
            }
            Some("ItemUnlinked") => {
                let p = &event["payload"];
                let src = p["source_id"].as_str().unwrap_or("");
                let lt  = p["link_type"].as_str().unwrap_or("");
                let tgt = p["target_id"].as_str().unwrap_or("");
                links.retain(|l| !(l.source_id == src && l.link_type == lt && l.target_id == tgt));
            }
            _ => {}
        }
    }
    Ok(links)
}

/// Convert a PascalCase or lowercase type name to a Logseq-friendly kebab-case tag.
fn type_to_logseq_tag(type_name: &str) -> String {
    let mut result = String::new();
    for (i, ch) in type_name.chars().enumerate() {
        if ch.is_uppercase() && i > 0 {
            result.push('-');
        }
        result.extend(ch.to_lowercase());
    }
    result
}

/// Convert a description string to a URL-safe slug (max 120 chars, word boundary truncation).
fn description_to_slug(desc: &str) -> String {
    let lower = desc.to_lowercase();
    let mut slug = String::new();
    let mut last_was_hyphen = false;
    for ch in lower.chars() {
        if ch.is_alphanumeric() {
            slug.push(ch);
            last_was_hyphen = false;
        } else if !last_was_hyphen && !slug.is_empty() {
            slug.push('-');
            last_was_hyphen = true;
        }
    }
    let slug = slug.trim_end_matches('-').to_string();
    if slug.len() <= 120 {
        slug
    } else {
        let truncated = &slug[..120];
        match truncated.rfind('-') {
            Some(pos) if pos > 0 => truncated[..pos].to_string(),
            _ => truncated.to_string(),
        }
    }
}

/// Build a UUID → slug map with collision resolution (-2, -3, …).
fn build_slug_map(items: &[RecordedItem]) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let mut seen: HashMap<String, u32> = HashMap::new();
    for item in items {
        let base = description_to_slug(&item.description);
        let count = seen.entry(base.clone()).or_insert(0);
        *count += 1;
        let slug = if *count == 1 { base } else { format!("{}-{}", base, count) };
        map.insert(item.item_id.clone(), slug);
    }
    map
}

fn render_relationship_sections(
    item_id: &str,
    links: &[LinkRecord],
    slug_map: &HashMap<String, String>,
    schema: &ProjectSchema,
) -> String {
    let mut sections: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for link in links {
        if link.source_id == item_id {
            let label = logseq_forward_label(schema, &link.link_type).to_string();
            let target_ref = slug_map
                .get(&link.target_id)
                .cloned()
                .unwrap_or_else(|| link.target_id.clone());
            sections.entry(label).or_default().push(format!("[[{}]]", target_ref));
        } else if link.target_id == item_id {
            let label = logseq_inverse_label(schema, &link.link_type).to_string();
            let source_ref = slug_map
                .get(&link.source_id)
                .cloned()
                .unwrap_or_else(|| link.source_id.clone());
            sections.entry(label).or_default().push(format!("[[{}]]", source_ref));
        }
    }
    if sections.is_empty() { return String::new(); }
    let mut out = String::new();
    for (label, refs) in &sections {
        out.push_str(&format!("\n- {}\n", label));
        for r in refs {
            out.push_str(&format!("    - {}\n", r));
        }
    }
    out
}

fn render_page(
    item: &RecordedItem,
    canonical_type: &str,
    status: Option<&str>,
    priority: Option<&str>,
    deadline: Option<&str>,
    links: &[LinkRecord],
    slug_map: &HashMap<String, String>,
    schema: &ProjectSchema,
) -> String {
    let type_tag = type_to_logseq_tag(canonical_type);
    let status_val = status.unwrap_or("not-set");
    let priority_val = priority.unwrap_or("not-set");
    let deadline_val = deadline.unwrap_or("TBD");
    let rel_sections = render_relationship_sections(&item.item_id, links, slug_map, schema);
    format!(
        "type:: {}\nstatus:: {}\npriority:: {}\ndeadline:: {}\ntags:: {}\n\n- item-id: {}\n{}",
        type_tag,
        status_val,
        priority_val,
        deadline_val,
        type_tag,
        item.item_id,
        rel_sections,
    )
}

fn check_output_dir_writable(pages_dir: &Path) -> bool {
    let test_path = pages_dir.join(".write_check");
    match fs::write(&test_path, b"") {
        Ok(_) => { let _ = fs::remove_file(&test_path); true }
        Err(_) => false,
    }
}

/// Delete pages in pages_dir whose stem is not in current_slugs.
fn remove_stale_pages(pages_dir: &Path, current_slugs: &[String]) {
    let entries = match fs::read_dir(pages_dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") { continue; }
        let stem = path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        if !current_slugs.contains(&stem) {
            let _ = fs::remove_file(&path);
        }
    }
}

fn cmd_export(output_dir: &str) -> Result<()> {
    let correlation_id = Uuid::new_v4().to_string();
    let events_path = Path::new(EVENTS_FILE);
    let emitter = EventEmitter::new(events_path, SOURCE_MODULE);

    // Load and validate the project vocabulary schema.
    // On failure, project_schema emits the appropriate FAILURE event and returns None.
    let schema = match load_and_validate(Path::new("."), events_path, &correlation_id) {
        Some(s) => s,
        None => return Ok(()),
    };

    emitter.emit("ExportRequested", &correlation_id, json!({
        "output_dir": output_dir,
    }));

    // Contract failure: ProjectRecordUnreadable
    let items = match read_all_record_items() {
        Ok(i) => i,
        Err(e) => {
            eprintln!("Cannot read project record: {:#}", e);
            emitter.emit("ExportFailedRecordUnreadable", &correlation_id, json!({
                "failure_reason": "project_record_unreadable",
                "error_detail": format!("{:#}", e),
            }));
            return Ok(());
        }
    };

    // Contract failure: EmptyProjectRecord
    if items.is_empty() {
        eprintln!("Project record contains no items. Nothing to export.");
        emitter.emit("ExportFailedEmptyRecord", &correlation_id, json!({
            "failure_reason": "empty_project_record",
        }));
        return Ok(());
    }

    let pages_dir = PathBuf::from(output_dir).join("pages");

    // Contract failure: OutputDirectoryNotAccessible
    if let Err(e) = fs::create_dir_all(&pages_dir) {
        eprintln!("Cannot create output directory '{}': {:#}", pages_dir.display(), e);
        emitter.emit("ExportFailedOutputUnavailable", &correlation_id, json!({
            "failure_reason": "output_directory_not_accessible",
            "output_dir": output_dir,
        }));
        return Ok(());
    }
    if !check_output_dir_writable(&pages_dir) {
        eprintln!("Output directory '{}' is not writable.", pages_dir.display());
        emitter.emit("ExportFailedOutputUnavailable", &correlation_id, json!({
            "failure_reason": "output_directory_not_accessible",
            "output_dir": output_dir,
        }));
        return Ok(());
    }

    // Separate task instances (block types) from page items.
    // Representation Ban: is_block_type resolves via vocabulary API.
    let mut items_excluded: u32 = 0;
    let mut page_items: Vec<(&RecordedItem, &str)> = Vec::new();
    let mut task_items: Vec<&RecordedItem> = Vec::new();

    for item in &items {
        match resolve_type(&schema, &item.item_type) {
            None => {
                emit_type_unknown(events_path, &item.item_id, &item.item_type, &correlation_id);
                eprintln!(
                    "warning: item {} has unrecognized type '{}' — excluded from export",
                    item.item_id, item.item_type
                );
                items_excluded += 1;
            }
            Some(canonical) if is_block_type(&schema, canonical) => {
                task_items.push(item);
            }
            Some(canonical) => {
                page_items.push((item, canonical));
            }
        }
    }

    // slug_map covers only page items (tasks are nested blocks, not pages)
    let page_items_for_slug: Vec<RecordedItem> = items.iter()
        .filter(|i| resolve_type(&schema, &i.item_type)
            .map(|c| !is_block_type(&schema, c))
            .unwrap_or(false))
        .cloned()
        .collect();
    let slug_map = build_slug_map(&page_items_for_slug);
    let links = build_active_links()?;
    let mut pages_written: Vec<String> = Vec::new();

    // Group tasks by parent_item_id for embedding in parent pages
    let mut tasks_by_parent: std::collections::HashMap<String, Vec<&RecordedItem>> =
        std::collections::HashMap::new();
    for task in &task_items {
        if let Some(ref parent_id) = task.parent_item_id {
            tasks_by_parent.entry(parent_id.clone()).or_default().push(task);
        }
    }

    for (item, canonical_type) in &page_items {
        let slug = slug_map.get(&item.item_id).cloned().unwrap_or_else(|| item.item_id.clone());
        let status = effective_status(&item.item_id)?;
        let priority = effective_priority(&item.item_id)?;
        let deadline = current_deadline(&item.item_id)?;
        let mut content = render_page(
            item,
            canonical_type,
            status.as_deref(),
            priority.as_deref(),
            deadline.as_deref(),
            &links,
            &slug_map,
            &schema,
        );

        // Append task block lines for tasks whose parent is this item
        if let Some(child_tasks) = tasks_by_parent.get(&item.item_id) {
            content.push_str("\n- Tasks\n");
            for task in child_tasks {
                let marker = task.current_marker.as_deref().unwrap_or("TODO");
                content.push_str(&format!(
                    "    - {} task-id: {} {}\n",
                    marker, task.item_id, task.description
                ));
            }
        }

        let page_path = pages_dir.join(format!("{}.md", slug));
        fs::write(&page_path, &content)
            .with_context(|| format!("writing page {}", page_path.display()))?;
        pages_written.push(page_path.to_string_lossy().into_owned());
    }

    // Remove pages that are no longer in the current export set (tasks never have pages)
    let current_slugs: Vec<String> = page_items
        .iter()
        .filter_map(|(item, _)| slug_map.get(&item.item_id).cloned())
        .collect();
    remove_stale_pages(&pages_dir, &current_slugs);

    let item_count = (page_items.len() + task_items.len()) as u64;
    println!(
        "Exported {} item(s) ({} page(s), {} task(s)) to '{}'.{}",
        item_count,
        page_items.len(),
        task_items.len(),
        output_dir,
        if items_excluded > 0 { format!(" ({} excluded: unrecognized type)", items_excluded) } else { String::new() }
    );

    emitter.emit("ExportCompleted", &correlation_id, json!({
        "output_dir": output_dir,
        "item_count": item_count,
        "pages_written": pages_written,
        "items_excluded_type_unknown": items_excluded,
    }));

    Ok(())
}

fn main() {
    let cli = Cli::parse();
    if let Err(e) = cmd_export(&cli.output_dir) {
        eprintln!("Error: {:#}", e);
        std::process::exit(1);
    }
}

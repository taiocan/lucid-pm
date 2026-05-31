use anyhow::{Context, Result};
use clap::Parser;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const EVENTS_FILE: &str = "events/runtime_events.jsonl";
const SOURCE_MODULE: &str = "logseq_export";

#[derive(Parser)]
#[command(about = "Export the project record as Logseq pages")]
struct Cli {
    /// Path to the Logseq output directory (pages will be written to <output_dir>/pages/)
    #[arg(long)]
    output_dir: String,
}

struct RecordedItem {
    item_id: String,
    item_type: String,
    description: String,
}

struct LinkRecord {
    source_id: String,
    link_type: String,
    target_id: String,
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

fn read_all_record_items() -> Result<Vec<RecordedItem>> {
    let sessions = read_incorporated_sessions()?;
    let mut all = Vec::new();
    for sid in sessions {
        all.extend(find_confirmed_items(&sid)?);
    }
    Ok(all)
}

fn current_status(item_id: &str) -> Result<Option<String>> {
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

fn current_priority(item_id: &str) -> Result<Option<String>> {
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

fn proposed_status_and_priority(item_id: &str) -> Result<(Option<String>, Option<String>)> {
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
    if !Path::new(EVENTS_FILE).exists() { return Ok(vec![]); }
    let file = fs::File::open(EVENTS_FILE).context("opening events file")?;
    let mut links: Vec<LinkRecord> = Vec::new();
    for line in std::io::BufReader::new(file).lines() {
        let line = line.context("reading events file")?;
        if line.trim().is_empty() { continue; }
        let event: Value = serde_json::from_str(&line).context("parsing event line")?;
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

fn forward_label(link_type: &str) -> &'static str {
    match link_type {
        "blocks"       => "Blocks",
        "affects"      => "Affects",
        "assigned_to"  => "Assigned To",
        "mitigated_by" => "Mitigated By",
        "escalates_to" => "Escalated To",
        "related_to"   => "Related To",
        _              => "Linked To",
    }
}

fn inverse_label(link_type: &str) -> &'static str {
    match link_type {
        "blocks"       => "Blocked By",
        "affects"      => "Affected By",
        "assigned_to"  => "Owns",
        "mitigated_by" => "Mitigates",
        "escalates_to" => "Escalations",
        "related_to"   => "Related To",
        _              => "Linked From",
    }
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
) -> String {
    let mut sections: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for link in links {
        if link.source_id == item_id {
            let label = forward_label(&link.link_type).to_string();
            let target_ref = slug_map
                .get(&link.target_id)
                .cloned()
                .unwrap_or_else(|| link.target_id.clone());
            sections.entry(label).or_default().push(format!("[[{}]]", target_ref));
        } else if link.target_id == item_id {
            let label = inverse_label(&link.link_type).to_string();
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
    status: Option<&str>,
    priority: Option<&str>,
    links: &[LinkRecord],
    slug_map: &HashMap<String, String>,
) -> String {
    let status_val = status.unwrap_or("not-set");
    let priority_val = priority.unwrap_or("not-set");
    let rel_sections = render_relationship_sections(&item.item_id, links, slug_map);
    format!(
        "type:: {}\nstatus:: {}\npriority:: {}\ntags:: {}\n\n- item-id: {}\n{}",
        item.item_type,
        status_val,
        priority_val,
        item.item_type,
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

    emit_event("ExportRequested", &correlation_id, json!({
        "output_dir": output_dir,
    }));

    // Contract failure: ProjectRecordUnreadable
    let items = match read_all_record_items() {
        Ok(i) => i,
        Err(e) => {
            eprintln!("Cannot read project record: {:#}", e);
            emit_event("ExportFailedRecordUnreadable", &correlation_id, json!({
                "failure_reason": "project_record_unreadable",
                "error_detail": format!("{:#}", e),
            }));
            return Ok(());
        }
    };

    // Contract failure: EmptyProjectRecord
    if items.is_empty() {
        eprintln!("Project record contains no items. Nothing to export.");
        emit_event("ExportFailedEmptyRecord", &correlation_id, json!({
            "failure_reason": "empty_project_record",
        }));
        return Ok(());
    }

    let pages_dir = PathBuf::from(output_dir).join("pages");

    // Contract failure: OutputDirectoryNotAccessible
    if let Err(e) = fs::create_dir_all(&pages_dir) {
        eprintln!("Cannot create output directory '{}': {:#}", pages_dir.display(), e);
        emit_event("ExportFailedOutputUnavailable", &correlation_id, json!({
            "failure_reason": "output_directory_not_accessible",
            "output_dir": output_dir,
        }));
        return Ok(());
    }
    if !check_output_dir_writable(&pages_dir) {
        eprintln!("Output directory '{}' is not writable.", pages_dir.display());
        emit_event("ExportFailedOutputUnavailable", &correlation_id, json!({
            "failure_reason": "output_directory_not_accessible",
            "output_dir": output_dir,
        }));
        return Ok(());
    }

    let slug_map = build_slug_map(&items);
    let links = build_active_links()?;
    let mut pages_written: Vec<String> = Vec::new();

    for item in &items {
        let slug = slug_map.get(&item.item_id).cloned().unwrap_or_else(|| item.item_id.clone());
        let status = effective_status(&item.item_id)?;
        let priority = effective_priority(&item.item_id)?;
        let content = render_page(item, status.as_deref(), priority.as_deref(), &links, &slug_map);
        let page_path = pages_dir.join(format!("{}.md", slug));
        fs::write(&page_path, &content)
            .with_context(|| format!("writing page {}", page_path.display()))?;
        pages_written.push(page_path.to_string_lossy().into_owned());
    }

    // Remove pages that are no longer in the current export set
    let current_slugs: Vec<String> = slug_map.values().cloned().collect();
    remove_stale_pages(&pages_dir, &current_slugs);

    let item_count = items.len() as u64;
    println!("Exported {} item(s) to '{}'.", item_count, output_dir);

    emit_event("ExportCompleted", &correlation_id, json!({
        "output_dir": output_dir,
        "item_count": item_count,
        "pages_written": pages_written,
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

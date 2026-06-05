use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

use project_schema::{
    load_and_validate, logseq_forward_label, logseq_inverse_label, resolve_type,
};

const EVENTS_FILE: &str = "events/runtime_events.jsonl";
const SOURCE_MODULE: &str = "item_links";

#[derive(Parser)]
#[command(about = "LucidPM item relationship manager")]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Record a typed link from one item to another
    Add {
        source_id: String,
        link_type: String,
        target_id: String,
    },
    /// Remove an existing link
    Remove {
        source_id: String,
        link_type: String,
        target_id: String,
    },
    /// List links in the project record
    List {
        /// Scope to a specific item (shows outgoing and incoming links)
        #[arg(long)]
        item: Option<String>,
    },
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
        "event_id":       Uuid::new_v4().to_string(),
        "event_type":     event_type,
        "timestamp":      timestamp_ms(),
        "correlation_id": correlation_id,
        "source_module":  SOURCE_MODULE,
        "payload":        payload,
    });
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(EVENTS_FILE)
        .expect("Failed to open events file");
    writeln!(file, "{}", event).expect("Failed to write event");
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

fn incorporated_sessions(events: &[Value]) -> Vec<(String, u64)> {
    events.iter()
        .filter(|e| {
            e["source_module"].as_str() == Some("project_state")
                && e["event_type"].as_str() == Some("ItemsIncorporated")
        })
        .filter_map(|e| {
            let sid = e["payload"]["session_id"].as_str()?.to_string();
            let ts  = e["timestamp"].as_u64().unwrap_or(0);
            Some((sid, ts))
        })
        .collect()
}

fn confirmed_items_for_session(events: &[Value], session_id: &str) -> Vec<(String, String, String)> {
    let mut items_extracted: Option<Vec<Value>> = None;
    let mut accepted_ids:    Option<Vec<String>> = None;

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

    let raw      = items_extracted.unwrap_or_default();
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

fn build_item_registry(events: &[Value]) -> HashMap<String, (String, String)> {
    let sessions = incorporated_sessions(events);
    let mut registry = HashMap::new();
    for (sid, _) in &sessions {
        for (id, ty, desc) in confirmed_items_for_session(events, sid) {
            registry.insert(id, (ty, desc));
        }
    }
    // Task instances from TaskAdded events
    for e in events {
        if e["source_module"].as_str() == Some("task_model")
            && e["event_type"].as_str() == Some("TaskAdded")
        {
            let p = &e["payload"];
            let task_id = p["task_id"].as_str().unwrap_or("").to_string();
            let item_type = p["item_type"].as_str().unwrap_or("").to_string();
            if !task_id.is_empty() && !item_type.is_empty() {
                registry.insert(
                    task_id,
                    (item_type, p["description"].as_str().unwrap_or("").to_string()),
                );
            }
        }
    }
    registry
}

fn build_links(events: &[Value]) -> Vec<LinkRecord> {
    let mut links: Vec<LinkRecord> = Vec::new();
    for e in events {
        if e["source_module"].as_str() != Some(SOURCE_MODULE) { continue; }
        match e["event_type"].as_str() {
            Some("ItemLinked") => {
                let src = e["payload"]["source_id"].as_str().unwrap_or("").to_string();
                let lt  = e["payload"]["link_type"].as_str().unwrap_or("").to_string();
                let tgt = e["payload"]["target_id"].as_str().unwrap_or("").to_string();
                if !src.is_empty() && !lt.is_empty() && !tgt.is_empty() {
                    links.push(LinkRecord { source_id: src, link_type: lt, target_id: tgt });
                }
            }
            Some("ItemUnlinked") => {
                let src = e["payload"]["source_id"].as_str().unwrap_or("");
                let lt  = e["payload"]["link_type"].as_str().unwrap_or("");
                let tgt = e["payload"]["target_id"].as_str().unwrap_or("");
                links.retain(|l| !(l.source_id == src && l.link_type == lt && l.target_id == tgt));
            }
            _ => {}
        }
    }
    links
}

fn display_item(id: &str, registry: &HashMap<String, (String, String)>) -> String {
    match registry.get(id) {
        Some((ty, desc)) => format!("[{}] {} ({}...)", ty, desc, &id[..8.min(id.len())]),
        None             => format!("(unknown) ({}...)", &id[..8.min(id.len())]),
    }
}

fn cmd_add(source_id: &str, link_type: &str, target_id: &str) -> Result<()> {
    let correlation_id = Uuid::new_v4().to_string();

    // Schema load before any link operation event — abort if schema invalid.
    // project_schema emits the schema failure event and prints to stderr.
    let schema = match load_and_validate(Path::new("."), Path::new(EVENTS_FILE), &correlation_id) {
        Some(s) => s,
        None => return Ok(()),
    };

    emit_event("LinkAddRequested", &correlation_id, json!({
        "source_id": source_id,
        "link_type": link_type,
        "target_id": target_id,
    }));

    let events   = read_events()?;
    let registry = build_item_registry(&events);
    let links    = build_links(&events);

    // Validate source exists
    let (source_type, _) = match registry.get(source_id) {
        Some(info) => info,
        None => {
            eprintln!("Item '{}' not found in project record.", source_id);
            emit_event("LinkFailedItemNotFound", &correlation_id, json!({
                "failure_reason":  "item_not_found",
                "operation":       "add",
                "missing_item_id": source_id,
            }));
            return Ok(());
        }
    };

    // Validate target exists
    let (target_type, _) = match registry.get(target_id) {
        Some(info) => info,
        None => {
            eprintln!("Item '{}' not found in project record.", target_id);
            emit_event("LinkFailedItemNotFound", &correlation_id, json!({
                "failure_reason":  "item_not_found",
                "operation":       "add",
                "missing_item_id": target_id,
            }));
            return Ok(());
        }
    };

    // Check source entity type is recognized by the active vocabulary
    if resolve_type(&schema, source_type).is_none() {
        eprintln!(
            "Item '{}' has entity type '{}' not recognized by the active vocabulary.",
            source_id, source_type
        );
        emit_event("LinkFailedItemTypeUnrecognized", &correlation_id, json!({
            "failure_reason": "item_type_unrecognized",
            "item_id":        source_id,
            "item_type":      source_type,
            "role":           "source",
        }));
        return Ok(());
    }

    // Check target entity type is recognized by the active vocabulary
    if resolve_type(&schema, target_type).is_none() {
        eprintln!(
            "Item '{}' has entity type '{}' not recognized by the active vocabulary.",
            target_id, target_type
        );
        emit_event("LinkFailedItemTypeUnrecognized", &correlation_id, json!({
            "failure_reason": "item_type_unrecognized",
            "item_id":        target_id,
            "item_type":      target_type,
            "role":           "target",
        }));
        return Ok(());
    }

    // Check relation type is defined in the active vocabulary
    if !schema.relations.contains_key(link_type) {
        eprintln!(
            "Relation type '{}' is not defined in the active vocabulary.",
            link_type
        );
        emit_event("LinkFailedRelationTypeUnrecognized", &correlation_id, json!({
            "failure_reason": "relation_type_unrecognized",
            "relation_type":  link_type,
        }));
        return Ok(());
    }

    // Check for duplicate
    if links.iter().any(|l| {
        l.source_id == source_id && l.link_type == link_type && l.target_id == target_id
    }) {
        eprintln!("Link already exists: {} --[{}]--> {}", source_id, link_type, target_id);
        emit_event("LinkFailedDuplicateLink", &correlation_id, json!({
            "failure_reason": "duplicate_link",
            "source_id":      source_id,
            "link_type":      link_type,
            "target_id":      target_id,
        }));
        return Ok(());
    }

    emit_event("ItemLinked", &correlation_id, json!({
        "source_id":   source_id,
        "source_type": source_type,
        "link_type":   link_type,
        "target_id":   target_id,
        "target_type": target_type,
    }));

    println!(
        "Linked: {}  --[{}]-->  {}",
        display_item(source_id, &registry),
        link_type,
        display_item(target_id, &registry),
    );
    Ok(())
}

fn cmd_remove(source_id: &str, link_type: &str, target_id: &str) -> Result<()> {
    let correlation_id = Uuid::new_v4().to_string();

    // Schema load is required by contract: all link commands perform schema validation
    // before execution, even though link removal itself does not use vocabulary data.
    // Vocabulary is intentionally not consulted for removal — the contract invariant
    // "Vocabulary evolution never prevents removal of an existing link" ensures that
    // a schema change can never leave the project record in an uncleanable state.
    let _schema = match load_and_validate(Path::new("."), Path::new(EVENTS_FILE), &correlation_id) {
        Some(s) => s,
        None => return Ok(()),
    };

    emit_event("LinkRemoveRequested", &correlation_id, json!({
        "source_id": source_id,
        "link_type": link_type,
        "target_id": target_id,
    }));

    let events   = read_events()?;
    let registry = build_item_registry(&events);
    let links    = build_links(&events);

    if !registry.contains_key(source_id) {
        eprintln!("Item '{}' not found in project record.", source_id);
        emit_event("LinkFailedItemNotFound", &correlation_id, json!({
            "failure_reason":  "item_not_found",
            "operation":       "remove",
            "missing_item_id": source_id,
        }));
        return Ok(());
    }

    if !registry.contains_key(target_id) {
        eprintln!("Item '{}' not found in project record.", target_id);
        emit_event("LinkFailedItemNotFound", &correlation_id, json!({
            "failure_reason":  "item_not_found",
            "operation":       "remove",
            "missing_item_id": target_id,
        }));
        return Ok(());
    }

    if !links.iter().any(|l| {
        l.source_id == source_id && l.link_type == link_type && l.target_id == target_id
    }) {
        eprintln!("Link not found: {} --[{}]--> {}", source_id, link_type, target_id);
        emit_event("LinkFailedLinkNotFound", &correlation_id, json!({
            "failure_reason": "link_not_found",
            "source_id":      source_id,
            "link_type":      link_type,
            "target_id":      target_id,
        }));
        return Ok(());
    }

    emit_event("ItemUnlinked", &correlation_id, json!({
        "source_id": source_id,
        "link_type": link_type,
        "target_id": target_id,
    }));

    println!(
        "Unlinked: {}  --[{}]-->  {}",
        display_item(source_id, &registry),
        link_type,
        display_item(target_id, &registry),
    );
    Ok(())
}

fn cmd_list(item_id: Option<&str>) -> Result<()> {
    let correlation_id = Uuid::new_v4().to_string();

    // Schema load before any link operation event — abort if schema invalid.
    let schema = match load_and_validate(Path::new("."), Path::new(EVENTS_FILE), &correlation_id) {
        Some(s) => s,
        None => return Ok(()),
    };

    emit_event("LinkListRequested", &correlation_id, json!({
        "item_id": item_id,
    }));

    let events   = read_events()?;
    let registry = build_item_registry(&events);
    let links    = build_links(&events);

    let mut link_entries: Vec<Value> = Vec::new();
    let mut excluded_count: u32 = 0;

    let candidate_links: Vec<(&LinkRecord, &str)> = match item_id {
        None => links.iter().map(|l| (l, "outgoing")).collect(),
        Some(iid) => links.iter().filter_map(|l| {
            if l.source_id == iid {
                Some((l, "outgoing"))
            } else if l.target_id == iid {
                Some((l, "incoming"))
            } else {
                None
            }
        }).collect(),
    };

    for (l, direction) in candidate_links {
        // Exclude links with unrecognized relation types, emitting one
        // LinkRelationTypeUnknown event per excluded link.
        if !schema.relations.contains_key(l.link_type.as_str()) {
            emit_event("LinkRelationTypeUnknown", &correlation_id, json!({
                "source_id": l.source_id,
                "link_type": l.link_type,
                "target_id": l.target_id,
            }));
            excluded_count += 1;
            eprintln!(
                "warning: link relation type '{}' not recognized by active vocabulary \
                 (source: {}, target: {}); excluded from output",
                l.link_type, l.source_id, l.target_id
            );
            continue;
        }

        let src_ty = registry.get(&l.source_id).map(|(t, _)| t.as_str()).unwrap_or("unknown");
        let tgt_ty = registry.get(&l.target_id).map(|(t, _)| t.as_str()).unwrap_or("unknown");

        let display_label = if direction == "outgoing" {
            logseq_forward_label(&schema, &l.link_type)
        } else {
            logseq_inverse_label(&schema, &l.link_type)
        };

        link_entries.push(json!({
            "source_id":     l.source_id,
            "source_type":   src_ty,
            "link_type":     l.link_type,
            "target_id":     l.target_id,
            "target_type":   tgt_ty,
            "direction":     direction,
            "display_label": display_label,
        }));
    }

    let link_count = link_entries.len();

    emit_event("LinkListReturned", &correlation_id, json!({
        "item_id":                         item_id,
        "link_count":                      link_count,
        "links":                           &link_entries,
        "links_excluded_relation_unknown": excluded_count,
    }));

    if link_count == 0 && excluded_count == 0 {
        println!("No links found.");
    } else {
        if link_count > 0 {
            println!("{} link(s):\n", link_count);
            for e in &link_entries {
                let label  = e["display_label"].as_str().unwrap_or("");
                let src_id = e["source_id"].as_str().unwrap_or("");
                let tgt_id = e["target_id"].as_str().unwrap_or("");
                let dir    = e["direction"].as_str().unwrap_or("outgoing");
                if dir == "outgoing" {
                    println!(
                        "  [{:<14}]  {}  -->  {}",
                        label,
                        display_item(src_id, &registry),
                        display_item(tgt_id, &registry),
                    );
                } else {
                    println!(
                        "  [{:<14}]  {}  <--  {}",
                        label,
                        display_item(tgt_id, &registry),
                        display_item(src_id, &registry),
                    );
                }
            }
        }
        if excluded_count > 0 {
            println!("\n{} link(s) excluded: relation type not in active vocabulary.", excluded_count);
        }
    }

    Ok(())
}

fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        Cmd::Add    { source_id, link_type, target_id } => cmd_add(&source_id, &link_type, &target_id),
        Cmd::Remove { source_id, link_type, target_id } => cmd_remove(&source_id, &link_type, &target_id),
        Cmd::List   { item }                            => cmd_list(item.as_deref()),
    };
    if let Err(e) = result {
        eprintln!("Error: {:#}", e);
        std::process::exit(1);
    }
}

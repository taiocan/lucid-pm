use std::collections::HashSet;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::Path;
use lucid_core::{EventEmitter, EVENTS_FILE};
use project_schema::{load_and_validate, ProjectSchema};
use serde_json::{json, Value};
use uuid::Uuid;

mod extractor;
use extractor::{extract_items, WpRecord};

const SOURCE_MODULE: &str = "pm_structuring";

fn has_yes_flag() -> bool {
    std::env::args().any(|a| a == "--yes" || a == "-y")
}

fn get_folder_arg() -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    for i in 0..args.len() {
        if args[i] == "--folder" {
            return args.get(i + 1).cloned();
        }
    }
    None
}

fn already_processed_files() -> HashSet<String> {
    let mut set = HashSet::new();
    let content = match fs::read_to_string(EVENTS_FILE) {
        Ok(c) => c,
        Err(_) => return set,
    };
    for line in content.lines() {
        if line.is_empty() { continue; }
        if let Ok(event) = serde_json::from_str::<Value>(line) {
            if event["event_type"] == "ItemsExtracted" {
                if let Some(sf) = event["payload"]["source_file"].as_str() {
                    set.insert(sf.to_string());
                }
            }
        }
    }
    set
}

// Read all WP-equivalent items from the project record (via incorporated sessions).
// WP type is resolved from schema alias "workpackage" — never hardcoded.
fn read_wp_items(schema: &ProjectSchema) -> Vec<WpRecord> {
    use project_schema::resolve_type;

    let wp_canonical = match resolve_type(schema, "workpackage") {
        Some(c) => c.to_string(),
        None => return Vec::new(),
    };

    let incorporated: Vec<String> = {
        let content = match fs::read_to_string(EVENTS_FILE) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };
        let mut sessions = Vec::new();
        for line in content.lines() {
            if line.is_empty() { continue; }
            if let Ok(ev) = serde_json::from_str::<Value>(line) {
                if ev["event_type"] == "ItemsIncorporated" {
                    if let Some(sid) = ev["payload"]["session_id"].as_str() {
                        sessions.push(sid.to_string());
                    }
                }
            }
        }
        sessions
    };

    let content = match fs::read_to_string(EVENTS_FILE) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let mut wp_items: Vec<WpRecord> = Vec::new();

    for session_id in &incorporated {
        let mut extracted_items: Vec<Value> = Vec::new();
        let mut accepted_ids: Vec<String> = Vec::new();

        for line in content.lines() {
            if line.is_empty() { continue; }
            let ev: Value = match serde_json::from_str(line) {
                Ok(v) => v,
                Err(_) => continue,
            };
            if ev["correlation_id"].as_str() != Some(session_id) { continue; }
            match ev["event_type"].as_str() {
                Some("ItemsExtracted") => {
                    if let Some(arr) = ev["payload"]["items"].as_array() {
                        extracted_items = arr.clone();
                    }
                }
                Some("ExtractionConfirmed") => {
                    if let Some(arr) = ev["payload"]["accepted_item_ids"].as_array() {
                        accepted_ids = arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect();
                    }
                }
                _ => {}
            }
        }

        for item in &extracted_items {
            let id = match item["item_id"].as_str() { Some(s) => s, None => continue };
            if !accepted_ids.contains(&id.to_string()) { continue; }
            let item_type = item["item_type"].as_str().unwrap_or("");
            if resolve_type(schema, item_type) == Some(&wp_canonical) {
                if let Some(desc) = item["description"].as_str() {
                    wp_items.push(WpRecord {
                        uuid: id.to_string(),
                        description: desc.to_string(),
                    });
                }
            }
        }
    }

    wp_items
}

// Runs the full extraction pipeline for a single block of text.
// Schema is validated before this function is called; it is passed as a reference.
async fn run_extraction(source_text: String, source_file: Option<&str>, auto_confirm: bool, schema: &ProjectSchema, wp_items: &[WpRecord]) {
    let correlation_id = Uuid::new_v4().to_string();
    let emitter = EventEmitter::new(Path::new(EVENTS_FILE), SOURCE_MODULE);

    emitter.emit("TextSubmitted", &correlation_id, json!({
        "source_text": source_text,
        "input_length": source_text.len(),
    }));

    if source_text.trim().is_empty() {
        eprintln!("Error: Input text is required.");
        emitter.emit("ExtractionFailedEmptyInput", &correlation_id, json!({
            "failure_reason": "empty_input",
        }));
        std::process::exit(1);
    }

    let items = match extract_items(&source_text, schema, wp_items).await {
        Ok(items) => items,
        Err(e) => {
            eprintln!("Error: API request failed: {}", e);
            emitter.emit("ExtractionFailedApiRequest", &correlation_id, json!({
                "failure_reason": "api_request_failed",
                "error_detail": e.to_string(),
            }));
            std::process::exit(1);
        }
    };

    if items.is_empty() {
        println!("No project management elements were found in the provided text.");
        emitter.emit("ExtractionFailedNoContent", &correlation_id, json!({
            "failure_reason": "no_extractable_content",
            "source_text_length": source_text.len(),
        }));
        return;
    }

    let uncertain_count = items.iter().filter(|i| i.uncertain).count() as u64;
    let items_payload: Vec<Value> = items.iter().map(|i| json!({
        "item_id": i.item_id,
        "item_type": i.item_type,
        "description": i.description,
        "uncertain": i.uncertain,
        "uncertainty_reason": i.uncertainty_reason,
        "proposed_status": i.proposed_status,
        "proposed_priority": i.proposed_priority,
        "parent_item_id": i.parent_item_id,
        "initial_marker": i.initial_marker,
    })).collect();

    emitter.emit("ItemsExtracted", &correlation_id, json!({
        "items": items_payload,
        "item_count": items.len(),
        "uncertain_count": uncertain_count,
        "source_file": source_file,
    }));

    println!("\n--- Extracted Project Management Items ---\n");
    for (i, item) in items.iter().enumerate() {
        let uncertainty_marker = if item.uncertain { " [UNCERTAIN]" } else { "" };
        println!("{}. [{}]{}", i + 1, item.item_type.to_uppercase(), uncertainty_marker);
        println!("   {}", item.description);
        if let Some(reason) = &item.uncertainty_reason {
            println!("   Uncertainty: {}", reason);
        }
        let status_hint   = item.proposed_status.as_deref().unwrap_or("—");
        let priority_hint = item.proposed_priority.as_deref().unwrap_or("—");
        println!("   Proposed: status={} priority={}", status_hint, priority_hint);
    }
    println!("\n------------------------------------------");
    println!("Total: {} items ({} uncertain)\n", items.len(), uncertain_count);

    let confirmed = if auto_confirm {
        println!("Auto-confirming (--yes).");
        true
    } else {
        print!("Confirm extracted items? [y/N]: ");
        io::stdout().flush().unwrap();
        let mut decision = String::new();
        io::stdin().read_line(&mut decision).expect("Failed to read decision");
        let d = decision.trim().to_lowercase();
        d == "y" || d == "yes"
    };

    if confirmed {
        let accepted_ids: Vec<String> = items.iter().map(|i| i.item_id.clone()).collect();
        let accepted_count = accepted_ids.len();
        emitter.emit("ExtractionConfirmed", &correlation_id, json!({
            "accepted_item_ids": accepted_ids,
            "accepted_count": accepted_count,
        }));
        println!("Extraction confirmed. {} items accepted.", accepted_count);
    } else {
        emitter.emit("ExtractionRejected", &correlation_id, json!({}));
        println!("Extraction rejected. No items accepted.");
    }
}

async fn cmd_folder(folder_path: String, auto_confirm: bool, schema: &ProjectSchema, wp_items: &[WpRecord]) {
    let folder_correlation_id = Uuid::new_v4().to_string();
    let emitter = EventEmitter::new(Path::new(EVENTS_FILE), SOURCE_MODULE);

    emitter.emit("FolderScanRequested", &folder_correlation_id, json!({
        "folder_path": folder_path,
        "auto_confirm": auto_confirm,
    }));

    if !Path::new(&folder_path).exists() {
        eprintln!("Error: Folder '{}' not found.", folder_path);
        emitter.emit("ExtractionFailedFolderNotFound", &folder_correlation_id, json!({
            "failure_reason": "folder_not_found",
            "folder_path": folder_path,
        }));
        std::process::exit(1);
    }

    let mut filenames: Vec<String> = fs::read_dir(&folder_path)
        .expect("Failed to read folder")
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            if name.ends_with(".md") || name.ends_with(".txt") { Some(name) } else { None }
        })
        .collect();
    filenames.sort_unstable();

    let files_found = filenames.len();
    let processed_set = already_processed_files();
    let mut files_skipped = 0usize;
    let mut files_processed = 0usize;

    if files_found == 0 {
        println!("No eligible files found in '{}'.", folder_path);
    } else {
        for filename in &filenames {
            if processed_set.contains(filename) {
                files_skipped += 1;
                continue;
            }
            let file_path = Path::new(&folder_path).join(filename);
            let content = match fs::read_to_string(&file_path) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Warning: could not read '{}': {}", filename, e);
                    files_skipped += 1;
                    continue;
                }
            };
            if content.trim().is_empty() {
                files_skipped += 1;
                continue;
            }
            println!("\n=== Processing: {} ===", filename);
            run_extraction(content, Some(filename), auto_confirm, schema, wp_items).await;
            files_processed += 1;
        }

        if files_processed == 0 && files_skipped == files_found {
            println!("No new files to process in '{}'.", folder_path);
        }
    }

    emitter.emit("FolderScanCompleted", &folder_correlation_id, json!({
        "folder_path": folder_path,
        "files_found": files_found,
        "files_skipped": files_skipped,
        "files_processed": files_processed,
    }));
}

#[tokio::main]
async fn main() {
    let auto_confirm = has_yes_flag();

    // Schema load before any command event — abort if schema invalid (FP1: SchemaInvalid).
    // project_schema emits the failure event and prints to stderr; we just exit.
    let schema_check_cid = Uuid::new_v4().to_string();
    let schema = match load_and_validate(Path::new("."), Path::new(EVENTS_FILE), &schema_check_cid) {
        Some(s) => s,
        None => std::process::exit(1),
    };

    let wp_items = read_wp_items(&schema);

    if let Some(folder_path) = get_folder_arg() {
        cmd_folder(folder_path, auto_confirm, &schema, &wp_items).await;
        return;
    }

    let stdin = io::stdin();
    let mut source_text = String::new();
    println!("Enter or paste the text to analyze (press Ctrl+D when done):");
    for line in stdin.lock().lines() {
        let line = line.expect("Failed to read line");
        source_text.push_str(&line);
        source_text.push('\n');
    }
    let source_text = source_text.trim_end().to_string();
    run_extraction(source_text, None, auto_confirm, &schema, &wp_items).await;
}

use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use serde_json::{json, Value};
use uuid::Uuid;

mod extractor;
use extractor::extract_items;

const EVENTS_FILE: &str = "events/runtime_events.jsonl";
const SOURCE_MODULE: &str = "pm_structuring";

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

// Returns filenames already recorded in ItemsExtracted.source_file across all prior runs.
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

// Runs the full extraction pipeline for a single block of text.
// source_file is None for stdin sessions, Some(filename) for folder-mode runs.
// Exits the process on unrecoverable failures (empty input, API failure).
async fn run_extraction(source_text: String, source_file: Option<&str>, auto_confirm: bool) {
    let correlation_id = Uuid::new_v4().to_string();

    emit_event("TextSubmitted", &correlation_id, json!({
        "source_text": source_text,
        "input_length": source_text.len(),
    }));

    if source_text.trim().is_empty() {
        eprintln!("Error: Input text is required.");
        emit_event("ExtractionFailedEmptyInput", &correlation_id, json!({
            "failure_reason": "empty_input",
        }));
        std::process::exit(1);
    }

    let items = match extract_items(&source_text).await {
        Ok(items) => items,
        Err(e) => {
            eprintln!("Error: API request failed: {}", e);
            emit_event("ExtractionFailedApiRequest", &correlation_id, json!({
                "failure_reason": "api_request_failed",
                "error_detail": e.to_string(),
            }));
            std::process::exit(1);
        }
    };

    if items.is_empty() {
        println!("No project management elements were found in the provided text.");
        emit_event("ExtractionFailedNoContent", &correlation_id, json!({
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
    })).collect();

    emit_event("ItemsExtracted", &correlation_id, json!({
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
        emit_event("ExtractionConfirmed", &correlation_id, json!({
            "accepted_item_ids": accepted_ids,
            "accepted_count": accepted_count,
        }));
        println!("Extraction confirmed. {} items accepted.", accepted_count);
    } else {
        emit_event("ExtractionRejected", &correlation_id, json!({}));
        println!("Extraction rejected. No items accepted.");
    }
}

async fn cmd_folder(folder_path: String, auto_confirm: bool) {
    let folder_correlation_id = Uuid::new_v4().to_string();

    emit_event("FolderScanRequested", &folder_correlation_id, json!({
        "folder_path": folder_path,
        "auto_confirm": auto_confirm,
    }));

    if !Path::new(&folder_path).exists() {
        eprintln!("Error: Folder '{}' not found.", folder_path);
        emit_event("ExtractionFailedFolderNotFound", &folder_correlation_id, json!({
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
            run_extraction(content, Some(filename), auto_confirm).await;
            files_processed += 1;
        }

        if files_processed == 0 && files_skipped == files_found {
            println!("No new files to process in '{}'.", folder_path);
        }
    }

    emit_event("FolderScanCompleted", &folder_correlation_id, json!({
        "folder_path": folder_path,
        "files_found": files_found,
        "files_skipped": files_skipped,
        "files_processed": files_processed,
    }));
}

#[tokio::main]
async fn main() {
    let auto_confirm = has_yes_flag();

    if let Some(folder_path) = get_folder_arg() {
        cmd_folder(folder_path, auto_confirm).await;
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
    run_extraction(source_text, None, auto_confirm).await;
}

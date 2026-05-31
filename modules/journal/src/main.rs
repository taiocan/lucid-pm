use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde_json::{json, Value};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const EVENTS_FILE: &str = "events/runtime_events.jsonl";
const JOURNAL_DIR: &str = "journal";
const SOURCE_MODULE: &str = "journal";

#[derive(Parser)]
#[command(about = "LucidPM journal — free-form notes alongside the project record")]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Create a new dated journal entry
    New {
        /// Entry title (used to name the file)
        #[arg(long, default_value = "")]
        title: String,
        /// File extension: md or txt
        #[arg(long, default_value = "md")]
        ext: String,
    },
    /// List all journal entries, most recent first
    List,
    /// Print the path to a journal entry
    Open {
        /// Filename of the entry to open
        filename: String,
    },
}

fn timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

fn emit(event_type: &str, correlation_id: &str, payload: Value) {
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

// Returns "YYYY-MM-DD" from the current system time.
fn today_str() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let days_since_epoch = secs / 86400;
    // Use a simple date calculation from epoch (1970-01-01)
    epoch_days_to_date(days_since_epoch)
}

// Minimal date calculation: days since Unix epoch → "YYYY-MM-DD".
fn epoch_days_to_date(days: u64) -> String {
    let mut remaining = days;
    let mut year = 1970u32;
    loop {
        let days_in_year = if is_leap(year) { 366 } else { 365 };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        year += 1;
    }
    let month_days: &[u32] = if is_leap(year) {
        &[31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        &[31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut month = 1u32;
    for &md in month_days {
        if remaining < md as u64 {
            break;
        }
        remaining -= md as u64;
        month += 1;
    }
    let day = remaining as u32 + 1;
    format!("{:04}-{:02}-{:02}", year, month, day)
}

fn is_leap(year: u32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

// Converts a title to a filename slug: lowercase, spaces→hyphens, strip non-alphanumeric.
fn slugify(title: &str) -> String {
    let slug: String = title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    // Collapse runs of hyphens and trim
    let mut result = String::new();
    let mut prev_hyphen = false;
    for c in slug.chars() {
        if c == '-' {
            if !prev_hyphen && !result.is_empty() {
                result.push('-');
            }
            prev_hyphen = true;
        } else {
            result.push(c);
            prev_hyphen = false;
        }
    }
    result.trim_end_matches('-').to_string()
}

// Parses title and created_at from a journal filename.
// Format: YYYY-MM-DD[-slug].ext
fn parse_entry(filename: &str) -> Option<(String, String)> {
    let stem = filename.rsplit_once('.').map(|(s, _)| s).unwrap_or(filename);
    if stem.len() < 10 {
        return None;
    }
    let created_at = &stem[..10];
    // Validate date prefix
    if !created_at.chars().enumerate().all(|(i, c)| {
        if i == 4 || i == 7 { c == '-' } else { c.is_ascii_digit() }
    }) {
        return None;
    }
    let title = if stem.len() > 11 {
        stem[11..].replace('-', " ")
    } else {
        created_at.to_string()
    };
    Some((title, created_at.to_string()))
}

fn journal_path() -> PathBuf {
    PathBuf::from(JOURNAL_DIR)
}

fn cmd_new(title: String, ext: String) -> Result<()> {
    let correlation_id = Uuid::new_v4().to_string();
    let date = today_str();

    let ext = match ext.trim_start_matches('.') {
        "txt" => "txt",
        _ => "md",
    };

    let effective_title = if title.is_empty() { date.clone() } else { title.clone() };
    let slug = slugify(&effective_title);
    let filename = format!("{}-{}.{}", date, slug, ext);

    let dir = journal_path();
    fs::create_dir_all(&dir).context("creating journal directory")?;

    let file_path = dir.join(&filename);
    if !file_path.exists() {
        fs::File::create(&file_path).context("creating journal entry file")?;
    }

    emit(
        "JournalEntryCreated",
        &correlation_id,
        json!({
            "filename":   filename,
            "title":      effective_title,
            "created_at": date,
        }),
    );

    let abs = fs::canonicalize(&file_path)
        .unwrap_or_else(|_| file_path.clone());
    println!("{}", abs.display());

    Ok(())
}

fn cmd_list() -> Result<()> {
    let correlation_id = Uuid::new_v4().to_string();

    emit("JournalListRequested", &correlation_id, json!({}));

    let dir = journal_path();
    let mut entries: Vec<Value> = Vec::new();

    if dir.exists() {
        let mut filenames: Vec<String> = fs::read_dir(&dir)
            .context("reading journal directory")?
            .filter_map(|e| e.ok())
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                if name.ends_with(".md") || name.ends_with(".txt") {
                    Some(name)
                } else {
                    None
                }
            })
            .collect();

        // Sort descending by filename (date prefix gives chronological order)
        filenames.sort_unstable_by(|a, b| b.cmp(a));

        for filename in &filenames {
            if let Some((title, created_at)) = parse_entry(filename) {
                entries.push(json!({
                    "filename":   filename,
                    "title":      title,
                    "created_at": created_at,
                }));
            }
        }
    }

    let entry_count = entries.len() as u64;
    emit(
        "JournalListReturned",
        &correlation_id,
        json!({
            "entry_count": entry_count,
            "entries":     entries.clone(),
        }),
    );

    if entries.is_empty() {
        println!("(no journal entries)");
    } else {
        for e in &entries {
            println!("{:12}  {}  {}",
                e["created_at"].as_str().unwrap_or(""),
                e["filename"].as_str().unwrap_or(""),
                e["title"].as_str().unwrap_or(""),
            );
        }
    }

    Ok(())
}

fn cmd_open(filename: String) -> Result<()> {
    let correlation_id = Uuid::new_v4().to_string();

    emit(
        "JournalOpenRequested",
        &correlation_id,
        json!({ "filename": filename }),
    );

    let file_path = journal_path().join(&filename);

    if !file_path.exists() {
        emit(
            "JournalOpenFailedEntryNotFound",
            &correlation_id,
            json!({
                "failure_reason": "entry_not_found",
                "filename":       filename,
            }),
        );
        eprintln!("Error: entry '{}' not found.", filename);
        std::process::exit(1);
    }

    let abs = fs::canonicalize(&file_path)
        .unwrap_or_else(|_| file_path.clone());
    let path_str = abs.to_string_lossy().to_string();

    emit(
        "JournalEntryOpened",
        &correlation_id,
        json!({
            "filename": filename,
            "path":     path_str.clone(),
        }),
    );

    println!("{}", path_str);

    Ok(())
}

fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        Cmd::New { title, ext } => cmd_new(title, ext),
        Cmd::List => cmd_list(),
        Cmd::Open { filename } => cmd_open(filename),
    };
    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

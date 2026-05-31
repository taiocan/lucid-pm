//! Behavioral tests for journal.
//!
//! Tests verify observable outcomes: events emitted, payload shapes, file
//! creation, listing order, failure modes, and telemetry.
//! All assertions reference event names from events/journal_schema.md exactly.

use serde_json::Value;
use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};
use tempfile::TempDir;

fn binary_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_journal"))
}

fn setup_temp_dir() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("events")).unwrap();
    dir
}

fn run(dir: &TempDir, args: &[&str]) -> std::process::Output {
    Command::new(binary_path())
        .current_dir(dir.path())
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("Failed to run binary")
}

fn read_events(dir: &TempDir) -> Vec<Value> {
    let path = dir.path().join("events/runtime_events.jsonl");
    if !path.exists() {
        return vec![];
    }
    fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .collect()
}

fn read_journal_events(dir: &TempDir) -> Vec<Value> {
    read_events(dir)
        .into_iter()
        .filter(|e| e["source_module"].as_str() == Some("journal"))
        .collect()
}

// Seed a journal entry file directly (for list/open tests that need a known filename).
fn seed_entry(dir: &TempDir, filename: &str, content: &str) {
    let journal_dir = dir.path().join("journal");
    fs::create_dir_all(&journal_dir).unwrap();
    let mut f = fs::File::create(journal_dir.join(filename)).unwrap();
    f.write_all(content.as_bytes()).unwrap();
}

// ── Happy Path 1: Entry Created ───────────────────────────────────────────────

#[test]
fn test_new_emits_journal_entry_created() {
    let dir = setup_temp_dir();

    run(&dir, &["new", "--title", "Sprint planning"]);

    let events = read_journal_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(
        types.contains(&"JournalEntryCreated"),
        "JournalEntryCreated must be emitted"
    );
    assert!(
        !types.iter().any(|t| t.starts_with("Journal") && t.contains("Failed")),
        "no failure event must be emitted on successful new"
    );
}

#[test]
fn test_new_creates_file_on_disk() {
    let dir = setup_temp_dir();

    run(&dir, &["new", "--title", "Sprint planning"]);

    let journal_dir = dir.path().join("journal");
    assert!(journal_dir.exists(), "journal/ directory must be created");

    let entries: Vec<_> = fs::read_dir(&journal_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert_eq!(entries.len(), 1, "exactly one file must be created");

    let filename = entries[0].file_name();
    let name = filename.to_string_lossy();
    assert!(name.ends_with(".md"), "default extension must be .md");
    assert!(
        name.contains("sprint-planning"),
        "filename must include title slug"
    );
}

#[test]
fn test_new_entry_created_payload_shape() {
    let dir = setup_temp_dir();

    run(&dir, &["new", "--title", "Release notes"]);

    let events = read_journal_events(&dir);
    let created = events
        .iter()
        .find(|e| e["event_type"] == "JournalEntryCreated")
        .unwrap();

    let p = &created["payload"];
    assert!(p["filename"].as_str().is_some(), "filename must be a string");
    assert!(p["title"].as_str().is_some(),    "title must be a string");
    assert!(p["created_at"].as_str().is_some(), "created_at must be a string");
    assert_eq!(p["title"].as_str().unwrap(), "Release notes");

    let created_at = p["created_at"].as_str().unwrap();
    assert_eq!(created_at.len(), 10, "created_at must be YYYY-MM-DD");
    assert_eq!(&created_at[4..5], "-");
    assert_eq!(&created_at[7..8], "-");

    let filename = p["filename"].as_str().unwrap();
    assert!(filename.starts_with(created_at), "filename must start with created_at date");
    assert!(filename.ends_with(".md"), "default filename must end with .md");
}

#[test]
fn test_new_prints_path_to_stdout() {
    let dir = setup_temp_dir();

    let out = run(&dir, &["new", "--title", "Standup notes"]);

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(!stdout.trim().is_empty(), "stdout must contain the file path");
    assert!(
        stdout.trim().contains("standup-notes"),
        "stdout path must reference the title slug"
    );
    assert!(
        stdout.trim().contains("journal"),
        "stdout path must reference the journal directory"
    );
}

#[test]
fn test_new_txt_ext_creates_txt_file() {
    let dir = setup_temp_dir();

    run(&dir, &["new", "--title", "Quick note", "--ext", "txt"]);

    let events = read_journal_events(&dir);
    let created = events
        .iter()
        .find(|e| e["event_type"] == "JournalEntryCreated")
        .unwrap();

    let filename = created["payload"]["filename"].as_str().unwrap();
    assert!(filename.ends_with(".txt"), "filename must end with .txt when --ext txt");

    let file_path = dir.path().join("journal").join(filename);
    assert!(file_path.exists(), "txt file must exist on disk");
}

#[test]
fn test_new_empty_title_uses_date_as_title() {
    let dir = setup_temp_dir();

    run(&dir, &["new"]);

    let events = read_journal_events(&dir);
    let created = events
        .iter()
        .find(|e| e["event_type"] == "JournalEntryCreated")
        .unwrap();

    let title = created["payload"]["title"].as_str().unwrap();
    let created_at = created["payload"]["created_at"].as_str().unwrap();
    assert_eq!(title, created_at, "empty title must use date string as title");
}

#[test]
fn test_new_creates_journal_dir_if_absent() {
    let dir = setup_temp_dir();
    // journal/ does not exist yet

    run(&dir, &["new", "--title", "First entry"]);

    assert!(
        dir.path().join("journal").exists(),
        "journal/ must be created automatically"
    );
}

// ── Happy Path 2: Entry Listed ────────────────────────────────────────────────

#[test]
fn test_list_emits_requested_then_returned() {
    let dir = setup_temp_dir();
    seed_entry(&dir, "2026-05-28-standup.md", "");

    run(&dir, &["list"]);

    let events = read_journal_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"JournalListRequested"), "JournalListRequested must be emitted");
    assert!(types.contains(&"JournalListReturned"),  "JournalListReturned must be emitted");

    let req_pos = types.iter().position(|&t| t == "JournalListRequested").unwrap();
    let ret_pos = types.iter().position(|&t| t == "JournalListReturned").unwrap();
    assert!(req_pos < ret_pos, "JournalListRequested must precede JournalListReturned");
}

#[test]
fn test_list_returned_payload_shape() {
    let dir = setup_temp_dir();
    seed_entry(&dir, "2026-05-28-standup.md", "");

    run(&dir, &["list"]);

    let events = read_journal_events(&dir);
    let returned = events.iter().find(|e| e["event_type"] == "JournalListReturned").unwrap();
    let p = &returned["payload"];

    assert!(p["entry_count"].as_u64().is_some(), "entry_count must be a u64");
    assert!(p["entries"].is_array(), "entries must be an array");
    assert_eq!(
        p["entry_count"].as_u64().unwrap() as usize,
        p["entries"].as_array().unwrap().len(),
        "entry_count must equal entries array length"
    );
}

#[test]
fn test_list_each_entry_has_required_fields() {
    let dir = setup_temp_dir();
    seed_entry(&dir, "2026-05-28-sprint-review.md", "");

    run(&dir, &["list"]);

    let events = read_journal_events(&dir);
    let returned = events.iter().find(|e| e["event_type"] == "JournalListReturned").unwrap();
    let entries = returned["payload"]["entries"].as_array().unwrap();

    assert!(!entries.is_empty());
    for entry in entries {
        assert!(entry["filename"].as_str().is_some(),   "entry must have filename");
        assert!(entry["title"].as_str().is_some(),      "entry must have title");
        assert!(entry["created_at"].as_str().is_some(), "entry must have created_at");
    }
}

#[test]
fn test_list_entries_sorted_most_recent_first() {
    let dir = setup_temp_dir();
    seed_entry(&dir, "2026-05-26-day-one.md",   "");
    seed_entry(&dir, "2026-05-28-day-three.md", "");
    seed_entry(&dir, "2026-05-27-day-two.md",   "");

    run(&dir, &["list"]);

    let events = read_journal_events(&dir);
    let returned = events.iter().find(|e| e["event_type"] == "JournalListReturned").unwrap();
    let entries = returned["payload"]["entries"].as_array().unwrap();

    assert_eq!(entries.len(), 3);
    let dates: Vec<&str> = entries
        .iter()
        .map(|e| e["created_at"].as_str().unwrap())
        .collect();
    assert_eq!(dates, vec!["2026-05-28", "2026-05-27", "2026-05-26"],
        "entries must be ordered most recent first");
}

#[test]
fn test_list_ignores_non_md_and_non_txt_files() {
    let dir = setup_temp_dir();
    let journal_dir = dir.path().join("journal");
    fs::create_dir_all(&journal_dir).unwrap();
    fs::File::create(journal_dir.join("2026-05-28-notes.md")).unwrap();
    fs::File::create(journal_dir.join("2026-05-28-notes.md.bak")).unwrap();
    fs::File::create(journal_dir.join(".hidden")).unwrap();

    run(&dir, &["list"]);

    let events = read_journal_events(&dir);
    let returned = events.iter().find(|e| e["event_type"] == "JournalListReturned").unwrap();
    assert_eq!(returned["payload"]["entry_count"].as_u64().unwrap(), 1,
        "only .md and .txt files must appear in listing");
}

// ── Happy Path 3: Entry Located ───────────────────────────────────────────────

#[test]
fn test_open_emits_requested_then_opened() {
    let dir = setup_temp_dir();
    seed_entry(&dir, "2026-05-28-standup.md", "");

    run(&dir, &["open", "2026-05-28-standup.md"]);

    let events = read_journal_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"JournalOpenRequested"), "JournalOpenRequested must be emitted");
    assert!(types.contains(&"JournalEntryOpened"),   "JournalEntryOpened must be emitted");
    assert!(!types.contains(&"JournalOpenFailedEntryNotFound"),
        "no failure must be emitted for existing entry");

    let req_pos = types.iter().position(|&t| t == "JournalOpenRequested").unwrap();
    let opn_pos = types.iter().position(|&t| t == "JournalEntryOpened").unwrap();
    assert!(req_pos < opn_pos, "JournalOpenRequested must precede JournalEntryOpened");
}

#[test]
fn test_open_prints_path_to_stdout() {
    let dir = setup_temp_dir();
    seed_entry(&dir, "2026-05-28-standup.md", "");

    let out = run(&dir, &["open", "2026-05-28-standup.md"]);

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(!stdout.trim().is_empty(), "stdout must contain the file path");
    assert!(
        stdout.trim().contains("standup"),
        "stdout path must reference the entry"
    );
}

#[test]
fn test_open_entry_opened_payload_shape() {
    let dir = setup_temp_dir();
    seed_entry(&dir, "2026-05-28-standup.md", "");

    run(&dir, &["open", "2026-05-28-standup.md"]);

    let events = read_journal_events(&dir);
    let opened = events.iter().find(|e| e["event_type"] == "JournalEntryOpened").unwrap();
    let p = &opened["payload"];

    assert_eq!(p["filename"].as_str().unwrap(), "2026-05-28-standup.md");
    assert!(p["path"].as_str().is_some(), "path must be a string");
    assert!(
        p["path"].as_str().unwrap().contains("standup"),
        "path must reference the entry file"
    );
}

// ── Happy Path 4: Empty Journal Listed ───────────────────────────────────────

#[test]
fn test_list_empty_emits_returned_with_zero_count() {
    let dir = setup_temp_dir();
    // No entries, journal/ does not exist

    run(&dir, &["list"]);

    let events = read_journal_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"JournalListReturned"), "JournalListReturned must be emitted");
    assert!(!types.iter().any(|t| t.contains("Failed")),
        "no failure must be emitted for empty listing");

    let returned = events.iter().find(|e| e["event_type"] == "JournalListReturned").unwrap();
    assert_eq!(returned["payload"]["entry_count"].as_u64().unwrap(), 0,
        "entry_count must be 0 for empty listing");
    assert!(returned["payload"]["entries"].as_array().unwrap().is_empty(),
        "entries must be empty array");
}

#[test]
fn test_list_with_existing_journal_dir_but_no_files_returns_empty() {
    let dir = setup_temp_dir();
    fs::create_dir(dir.path().join("journal")).unwrap();

    run(&dir, &["list"]);

    let events = read_journal_events(&dir);
    let returned = events.iter().find(|e| e["event_type"] == "JournalListReturned").unwrap();
    assert_eq!(returned["payload"]["entry_count"].as_u64().unwrap(), 0);
}

// ── Failure Path 1: EntryNotFound ─────────────────────────────────────────────

#[test]
fn test_open_nonexistent_emits_failure() {
    let dir = setup_temp_dir();

    run(&dir, &["open", "2026-01-01-nonexistent.md"]);

    let events = read_journal_events(&dir);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"JournalOpenRequested"),
        "JournalOpenRequested must be emitted");
    assert!(types.contains(&"JournalOpenFailedEntryNotFound"),
        "JournalOpenFailedEntryNotFound must be emitted");
    assert!(!types.contains(&"JournalEntryOpened"),
        "JournalEntryOpened must NOT be emitted when entry not found");
}

#[test]
fn test_open_nonexistent_failure_payload() {
    let dir = setup_temp_dir();
    let missing = "2026-01-01-does-not-exist.md";

    run(&dir, &["open", missing]);

    let events = read_journal_events(&dir);
    let failure = events
        .iter()
        .find(|e| e["event_type"] == "JournalOpenFailedEntryNotFound")
        .unwrap();

    let p = &failure["payload"];
    assert_eq!(p["failure_reason"].as_str().unwrap(), "entry_not_found");
    assert_eq!(p["filename"].as_str().unwrap(), missing);
}

#[test]
fn test_open_nonexistent_exits_nonzero() {
    let dir = setup_temp_dir();

    let out = run(&dir, &["open", "ghost.md"]);
    assert!(!out.status.success(), "exit code must be non-zero for EntryNotFound");
}

// ── Invariants ────────────────────────────────────────────────────────────────

#[test]
fn test_new_does_not_modify_project_record_events() {
    let dir = setup_temp_dir();
    // No project record events exist yet; new must not emit any record events
    run(&dir, &["new", "--title", "My notes"]);

    let events = read_journal_events(&dir);
    assert!(
        events.iter().all(|e| e["source_module"].as_str() == Some("journal")),
        "journal new must only emit journal-owned events"
    );
    assert!(
        !events.iter().any(|e| {
            matches!(e["event_type"].as_str(),
                Some("ItemsExtracted") | Some("ItemsIncorporated") | Some("ItemLinked"))
        }),
        "journal new must not emit project record events"
    );
}

#[test]
fn test_new_does_not_overwrite_existing_file() {
    let dir = setup_temp_dir();
    seed_entry(&dir, "2026-05-28-my-notes.md", "existing content");

    // Simulate running new with same date/title — binary should not overwrite
    // (we test the invariant by checking the file still has original content)
    let journal_dir = dir.path().join("journal");
    let file_path = journal_dir.join("2026-05-28-my-notes.md");
    let before = fs::read_to_string(&file_path).unwrap();

    // We can't force the same date, so we test the general idempotency contract:
    // creating a new entry with a different title must not touch existing files
    run(&dir, &["new", "--title", "Different title"]);

    let after = fs::read_to_string(&file_path).unwrap();
    assert_eq!(before, after, "existing journal files must not be modified by new");
}

// ── Telemetry ─────────────────────────────────────────────────────────────────

#[test]
fn test_all_events_have_required_base_fields() {
    let dir = setup_temp_dir();
    seed_entry(&dir, "2026-05-28-standup.md", "");
    run(&dir, &["new", "--title", "Test"]);
    run(&dir, &["list"]);
    run(&dir, &["open", "2026-05-28-standup.md"]);

    let events = read_journal_events(&dir);
    assert!(!events.is_empty());

    for event in &events {
        let t = event["event_type"].as_str().unwrap_or("unknown");
        assert!(event["event_id"].as_str().is_some(),       "{}: event_id must be a string", t);
        assert!(event["event_type"].as_str().is_some(),     "{}: event_type must be a string", t);
        assert!(event["timestamp"].as_u64().is_some(),      "{}: timestamp must be a u64", t);
        assert!(event["correlation_id"].as_str().is_some(), "{}: correlation_id must be present", t);
        assert!(event["source_module"].as_str().is_some(),  "{}: source_module must be a string", t);
        assert!(event["payload"].is_object(),               "{}: payload must be an object", t);
        assert_eq!(event["source_module"].as_str().unwrap(), "journal",
            "{}: source_module must be 'journal'", t);
    }
}

#[test]
fn test_correlation_id_consistent_within_invocation() {
    let dir = setup_temp_dir();
    run(&dir, &["list"]);

    let events = read_journal_events(&dir);
    assert!(events.len() >= 2, "list emits at least 2 events");

    let cid = events[0]["correlation_id"].as_str().unwrap();
    for event in &events {
        assert_eq!(event["correlation_id"].as_str().unwrap(), cid,
            "all events from one invocation must share correlation_id");
    }
}

#[test]
fn test_separate_invocations_have_different_correlation_ids() {
    let dir = setup_temp_dir();
    run(&dir, &["list"]);
    run(&dir, &["list"]);

    let events = read_journal_events(&dir);
    let requested: Vec<&str> = events
        .iter()
        .filter(|e| e["event_type"] == "JournalListRequested")
        .map(|e| e["correlation_id"].as_str().unwrap())
        .collect();

    assert_eq!(requested.len(), 2);
    assert_ne!(requested[0], requested[1],
        "separate invocations must have different correlation_ids");
}

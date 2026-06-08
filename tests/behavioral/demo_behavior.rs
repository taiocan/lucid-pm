//! Behavioral tests for demo.
//!
//! Tests verify the demo content satisfies the contract invariants by
//! inspecting the walkthrough document, file structure, and graph-record
//! consistency. No events are expected — demo has a structural null event
//! spine (see events/demo_schema.md).
//!
//! All assertions reference contract clauses from contracts/demo_contract.md.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap() // modules/
        .parent().unwrap() // LucidPM/
        .to_path_buf()
}

fn demo_dir() -> PathBuf {
    repo_root().join("demo")
}

fn walkthrough() -> String {
    fs::read_to_string(demo_dir().join("WALKTHROUGH.md"))
        .expect("WALKTHROUGH.md must exist in demo/")
}

/// Returns the first line index (0-based) where a code-block command starting
/// with the given prefix appears in the walkthrough. Code-block lines are those
/// whose trimmed content starts with `lucid <cmd>` — not prose inline references.
fn first_command_line(wt: &str, cmd: &str) -> Option<usize> {
    let prefix = format!("lucid {cmd}");
    wt.lines().position(|line| {
        let t = line.trim();
        t == prefix || t.starts_with(&format!("{prefix} ")) || t.starts_with(&format!("{prefix}\t"))
    })
}

// ── Deliverable: self-contained ───────────────────────────────────────────────

#[test]
fn test_notes_directory_present() {
    assert!(demo_dir().join("notes").is_dir(), "demo/notes/ must exist");
}

#[test]
fn test_notes_files_present() {
    let notes = demo_dir().join("notes");
    let files: Vec<_> = fs::read_dir(&notes).unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert!(!files.is_empty(), "demo/notes/ must contain at least one input file");
}

#[test]
fn test_project_record_present() {
    assert!(
        demo_dir().join("events/runtime_events.jsonl").is_file(),
        "demo/events/runtime_events.jsonl must exist"
    );
}

#[test]
fn test_schema_present() {
    assert!(
        demo_dir().join("project-schema.yaml").is_file(),
        "demo/project-schema.yaml must exist"
    );
}

#[test]
fn test_logseq_graph_present() {
    assert!(demo_dir().join("logseq").is_dir(), "demo/logseq/ must exist");
    assert!(
        demo_dir().join("logseq/pages").is_dir(),
        "demo/logseq/pages/ must exist"
    );
    let pages: Vec<_> = fs::read_dir(demo_dir().join("logseq/pages")).unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("md"))
        .collect();
    assert!(!pages.is_empty(), "demo/logseq/pages/ must contain at least one page");
}

#[test]
fn test_walkthrough_present() {
    assert!(
        demo_dir().join("WALKTHROUGH.md").is_file(),
        "demo/WALKTHROUGH.md must exist"
    );
}

#[test]
fn test_journal_present() {
    assert!(demo_dir().join("journal").is_dir(), "demo/journal/ must exist");
    let entries: Vec<_> = fs::read_dir(demo_dir().join("journal")).unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert!(!entries.is_empty(), "demo/journal/ must contain at least one entry");
}

// ── Deliverable: all major features represented ───────────────────────────────

#[test]
fn test_all_non_optional_features_in_walkthrough() {
    let wt = walkthrough();
    for cmd in NON_OPTIONAL_COMMANDS {
        let pattern = format!("`lucid {cmd}");
        assert!(
            wt.contains(&pattern),
            "WALKTHROUGH.md must contain a `lucid {cmd}` invocation (non-optional feature)"
        );
    }
}

#[test]
fn test_suggest_present_and_marked_optional() {
    let wt = walkthrough();
    assert!(wt.contains("`lucid suggest"), "WALKTHROUGH.md must mention lucid suggest");
    let suggest_idx = wt.find("`lucid suggest").unwrap();
    // "optional" or "requires" must appear within 300 chars of the suggest mention
    let context = &wt[suggest_idx.saturating_sub(50)..
        (suggest_idx + 400).min(wt.len())];
    assert!(
        context.to_lowercase().contains("optional") || context.contains("requires"),
        "lucid suggest must be labelled as optional or requiring an API key"
    );
}

// ── Deliverable: from-scratch path (extraction first) ────────────────────────

/// LUC-DEMO-IF-04
/// Falsifies: walkthrough begins from a pre-built record, skipping extraction.
#[test]
fn test_extraction_precedes_state_commands() {
    let wt = walkthrough();
    // Use line-based matching on code-block commands to avoid false matches
    // against prose inline references (e.g. "`lucid state view`" in body text).
    let extract_line = first_command_line(&wt, "extract")
        .expect("WALKTHROUGH.md must contain a code-block `lucid extract` invocation");
    let state_line = first_command_line(&wt, "state")
        .expect("WALKTHROUGH.md must contain a code-block `lucid state` invocation");
    assert!(
        extract_line < state_line,
        "`lucid extract` (line {}) must appear before `lucid state` (line {}) in the walkthrough",
        extract_line, state_line
    );
    if let Some(export_line) = first_command_line(&wt, "export") {
        assert!(
            extract_line < export_line,
            "`lucid extract` (line {}) must appear before `lucid export` (line {})",
            extract_line, export_line
        );
    }
}

// ── Deliverable: Logseq as primary output surface ────────────────────────────

/// LUC-DEMO-IF-05
/// Falsifies: walkthrough shows terminal output only with no Logseq step.
#[test]
fn test_logseq_mentioned_after_export() {
    let wt = walkthrough();
    let export_pos = wt.find("`lucid export")
        .expect("WALKTHROUGH.md must contain `lucid export`");
    let logseq_after = wt[export_pos..].to_lowercase().contains("logseq");
    assert!(
        logseq_after,
        "WALKTHROUGH.md must mention Logseq after the export step"
    );
}

#[test]
fn test_logseq_sync_in_walkthrough() {
    let wt = walkthrough();
    assert!(
        wt.contains("`lucid sync"),
        "WALKTHROUGH.md must include the Logseq sync step"
    );
}

// ── Deliverable: graph-record consistency ────────────────────────────────────

/// LUC-DEMO-IF-03
/// Runs lucid export against the demo record and diffs against committed pages.
/// Falsifies: pre-exported graph was authored separately or drifted from the record.
#[test]
fn test_graph_record_consistency() {
    let temp = TempDir::new().unwrap();

    // Set up a temp project dir with the demo record and schema
    let events_dir = temp.path().join("events");
    fs::create_dir(&events_dir).unwrap();
    fs::copy(
        demo_dir().join("events/runtime_events.jsonl"),
        events_dir.join("runtime_events.jsonl"),
    ).unwrap();
    fs::copy(
        demo_dir().join("project-schema.yaml"),
        temp.path().join("project-schema.yaml"),
    ).unwrap();

    // Run the installed `lucid` (on PATH) — uses real feature binaries, not bin/lucid
    // which would look in bin/ where feature binaries are absent.
    let out = Command::new("lucid")
        .arg("export")
        .arg("--output-dir")
        .arg("logseq")
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "lucid export failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // Compare each exported page against the committed page
    let committed_pages = demo_dir().join("logseq/pages");
    let exported_pages = temp.path().join("logseq/pages");

    let committed: Vec<_> = fs::read_dir(&committed_pages).unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("md"))
        .collect();

    assert!(!committed.is_empty(), "committed pages must not be empty");

    for entry in &committed {
        let filename = entry.file_name();
        let exported_path = exported_pages.join(&filename);
        assert!(
            exported_path.exists(),
            "exported pages missing committed page: {}",
            filename.to_string_lossy()
        );
        let committed_content = fs::read_to_string(entry.path()).unwrap();
        let exported_content = fs::read_to_string(&exported_path).unwrap();
        assert_eq!(
            committed_content, exported_content,
            "page {} differs between committed graph and fresh export",
            filename.to_string_lossy()
        );
    }

    // Check no extra pages were exported that aren't committed
    let exported: Vec<_> = fs::read_dir(&exported_pages).unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("md"))
        .collect();
    assert_eq!(
        committed.len(), exported.len(),
        "committed {} pages but export produced {} — sets must match",
        committed.len(), exported.len()
    );
}

// ── Invariant Falsification ───────────────────────────────────────────────────
// Tests IDs: DEMO-IF-01 through DEMO-IF-05

/// DEMO-IF-01 — self-contained: all referenced files exist in demo/
/// Falsifies: a walkthrough step references a file outside demo/.
#[test]
fn test_walkthrough_references_no_external_files() {
    let wt = walkthrough();
    // Grep for path-like strings that point outside demo/
    // A simple heuristic: no absolute paths and no "../" in file references
    assert!(
        !wt.contains("../"),
        "WALKTHROUGH.md must not reference paths outside demo/ using ../"
    );
    // Also check that notes files referenced exist
    if wt.contains("notes/01-kickoff-meeting.md") {
        assert!(
            demo_dir().join("notes/01-kickoff-meeting.md").exists(),
            "notes/01-kickoff-meeting.md referenced in walkthrough but absent"
        );
    }
    if wt.contains("notes/02-risk-and-issue-review.md") {
        assert!(
            demo_dir().join("notes/02-risk-and-issue-review.md").exists(),
            "notes/02-risk-and-issue-review.md referenced in walkthrough but absent"
        );
    }
}

/// DEMO-IF-02 — all non-optional features appear in walkthrough
/// Falsifies: a feature was omitted when writing the walkthrough.
#[test]
fn test_walkthrough_feature_gap_detection() {
    let wt = walkthrough();
    let missing: Vec<&&str> = NON_OPTIONAL_COMMANDS.iter()
        .filter(|cmd| !wt.contains(&format!("`lucid {cmd}")))
        .collect();
    assert!(
        missing.is_empty(),
        "WalkthroughFeatureGap: these commands are missing from WALKTHROUGH.md: {:?}",
        missing
    );
}

/// DEMO-IF-03 — graph-record consistency (covered by test_graph_record_consistency above)
/// Falsifies: pre-exported pages were authored separately and drifted from the record.
/// (Test is the same function — referenced here for traceability.)

/// DEMO-IF-04 — from-scratch path starts with extraction
/// (Covered by test_extraction_precedes_state_commands above.)

/// DEMO-IF-05 — Logseq is primary output surface
/// (Covered by test_logseq_mentioned_after_export above.)

// ── Null event spine ─────────────────────────────────────────────────────────

#[test]
fn test_no_demo_events_in_record() {
    let content = fs::read_to_string(demo_dir().join("events/runtime_events.jsonl")).unwrap();
    for line in content.lines().filter(|l| !l.is_empty()) {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        assert_ne!(
            v["source_module"].as_str(), Some("demo"),
            "demo must not emit events — found source_module:demo in record"
        );
    }
}

// ── Major features table (authoritative for completeness tests) ───────────────

const NON_OPTIONAL_COMMANDS: &[&str] = &[
    "extract", "state", "status", "link", "export",
    "sync", "priority", "report", "schema", "task", "journal",
];

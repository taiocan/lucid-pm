//! Behavioral tests for logseq_export_links (F8).
//!
//! Tests verify observable outcomes of link rendering: relationship sections
//! written to Logseq pages, forward/inverse label placement, section omission
//! when no links exist, removed-link suppression, and idempotency.
//! All assertions reference event names from events/logseq_export_schema.md.

use serde_json::{json, Value};
use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};
use tempfile::TempDir;

fn binary_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_logseq_export"))
}

fn setup_temp_dir() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("events")).unwrap();
    dir
}

fn description_to_slug(desc: &str) -> String {
    let lower = desc.to_lowercase();
    let mut slug = String::new();
    let mut last_was_hyphen = false;
    for ch in lower.chars() {
        if ch.is_alphanumeric() { slug.push(ch); last_was_hyphen = false; }
        else if !last_was_hyphen && !slug.is_empty() { slug.push('-'); last_was_hyphen = true; }
    }
    let slug = slug.trim_end_matches('-').to_string();
    if slug.len() <= 120 { slug }
    else {
        let truncated = &slug[..120];
        match truncated.rfind('-') {
            Some(pos) if pos > 0 => truncated[..pos].to_string(),
            _ => truncated.to_string(),
        }
    }
}

fn seed_incorporated_items(dir: &TempDir, session_id: &str, items: &[(&str, &str, &str)]) {
    let items_json: Vec<Value> = items
        .iter()
        .map(|(id, typ, desc)| json!({
            "item_id": id,
            "item_type": typ,
            "description": desc,
            "uncertain": false,
            "uncertainty_reason": null,
            "proposed_status": null,
            "proposed_priority": null,
        }))
        .collect();

    let accepted_ids: Vec<&str> = items.iter().map(|(id, _, _)| *id).collect();

    let path = dir.path().join("events/runtime_events.jsonl");
    let mut file = fs::OpenOptions::new().create(true).append(true).open(&path).unwrap();

    writeln!(file, "{}", json!({
        "event_id": format!("seed-ext-{}", &session_id[..8]),
        "event_type": "ItemsExtracted",
        "timestamp": 1748000001000u64,
        "correlation_id": session_id,
        "source_module": "pm_structuring",
        "payload": { "items": items_json, "item_count": items.len(), "uncertain_count": 0 }
    })).unwrap();
    writeln!(file, "{}", json!({
        "event_id": format!("seed-conf-{}", &session_id[..8]),
        "event_type": "ExtractionConfirmed",
        "timestamp": 1748000002000u64,
        "correlation_id": session_id,
        "source_module": "pm_structuring",
        "payload": { "accepted_item_ids": accepted_ids, "accepted_count": items.len() }
    })).unwrap();
    writeln!(file, "{}", json!({
        "event_id": format!("seed-inc-{}", &session_id[..8]),
        "event_type": "ItemsIncorporated",
        "timestamp": 1748000003000u64,
        "correlation_id": "00000000-0000-0000-0000-000000000001",
        "source_module": "project_state",
        "payload": { "session_id": session_id, "incorporated_count": items.len(), "total_record_size": items.len() }
    })).unwrap();
}

fn seed_link_event(dir: &TempDir, source_id: &str, source_type: &str, link_type: &str, target_id: &str, target_type: &str) {
    let path = dir.path().join("events/runtime_events.jsonl");
    let mut file = fs::OpenOptions::new().create(true).append(true).open(&path).unwrap();
    writeln!(file, "{}", json!({
        "event_id": format!("seed-lnk-{}", &source_id[..8]),
        "event_type": "ItemLinked",
        "timestamp": 1748100001000u64,
        "correlation_id": "c0000000-0000-0000-0000-000000000001",
        "source_module": "item_links",
        "payload": {
            "source_id": source_id,
            "source_type": source_type,
            "link_type": link_type,
            "target_id": target_id,
            "target_type": target_type,
        }
    })).unwrap();
}

fn seed_unlink_event(dir: &TempDir, source_id: &str, link_type: &str, target_id: &str) {
    let path = dir.path().join("events/runtime_events.jsonl");
    let mut file = fs::OpenOptions::new().create(true).append(true).open(&path).unwrap();
    writeln!(file, "{}", json!({
        "event_id": format!("seed-ulk-{}", &source_id[..8]),
        "event_type": "ItemUnlinked",
        "timestamp": 1748100002000u64,
        "correlation_id": "c0000000-0000-0000-0000-000000000002",
        "source_module": "item_links",
        "payload": {
            "source_id": source_id,
            "link_type": link_type,
            "target_id": target_id,
        }
    })).unwrap();
}

fn run_export(dir: &TempDir, output_dir: &str) {
    Command::new(binary_path())
        .current_dir(dir.path())
        .args(["--output-dir", output_dir])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("Failed to run binary");
}

fn page_content(output_dir: &str, description: &str) -> String {
    let slug = description_to_slug(description);
    let path = std::path::PathBuf::from(output_dir)
        .join("pages")
        .join(format!("{}.md", slug));
    fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("Page for '{}' (slug: {}) must exist", description, slug))
}

const SESSION_A: &str = "a4ca3a7e-61eb-4f36-b59e-f3abd166e351";
const ITEM_TASK:      &str = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeee01";
const ITEM_MILESTONE: &str = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeee03";
const ITEM_RISK:      &str = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeee02";

// Descriptions used by tests; slugs derived by description_to_slug()
const DESC_TASK:      &str = "Fix critical bug";       // → fix-critical-bug
const DESC_MILESTONE: &str = "Q3 Release";             // → q3-release
const DESC_RISK:      &str = "Vendor delay";           // → vendor-delay

// ── HP1: Outgoing link → forward label on source page ────────────────────────

#[test]
fn test_outgoing_link_shows_forward_label_on_source_page() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[
        (ITEM_TASK, "task", DESC_TASK),
        (ITEM_MILESTONE, "milestone", DESC_MILESTONE),
    ]);
    seed_link_event(&dir, ITEM_TASK, "task", "blocks", ITEM_MILESTONE, "milestone");

    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();
    run_export(&dir, &output_dir);

    let content = page_content(&output_dir, DESC_TASK);
    assert!(content.contains("- Blocks"),
        "Source page must contain a '- Blocks' relationship section");
    assert!(content.contains("[[q3-release]]"),
        "Source page must reference the target item by its slug '[[q3-release]]'");
}

#[test]
fn test_outgoing_link_no_inverse_label_on_source_page() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[
        (ITEM_TASK, "task", DESC_TASK),
        (ITEM_MILESTONE, "milestone", DESC_MILESTONE),
    ]);
    seed_link_event(&dir, ITEM_TASK, "task", "blocks", ITEM_MILESTONE, "milestone");

    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();
    run_export(&dir, &output_dir);

    let content = page_content(&output_dir, DESC_TASK);
    assert!(!content.contains("- Blocked By"),
        "Source page must NOT contain inverse '- Blocked By' section");
}

// ── HP2: Incoming link → inverse label on target page ────────────────────────

#[test]
fn test_incoming_link_shows_inverse_label_on_target_page() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[
        (ITEM_TASK, "task", DESC_TASK),
        (ITEM_MILESTONE, "milestone", DESC_MILESTONE),
    ]);
    seed_link_event(&dir, ITEM_TASK, "task", "blocks", ITEM_MILESTONE, "milestone");

    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();
    run_export(&dir, &output_dir);

    let content = page_content(&output_dir, DESC_MILESTONE);
    assert!(content.contains("- Blocked By"),
        "Target page must contain a '- Blocked By' inverse relationship section");
    assert!(content.contains("[[fix-critical-bug]]"),
        "Target page must reference the source item by its slug '[[fix-critical-bug]]'");
}

#[test]
fn test_incoming_link_no_forward_label_on_target_page() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[
        (ITEM_TASK, "task", DESC_TASK),
        (ITEM_MILESTONE, "milestone", DESC_MILESTONE),
    ]);
    seed_link_event(&dir, ITEM_TASK, "task", "blocks", ITEM_MILESTONE, "milestone");

    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();
    run_export(&dir, &output_dir);

    let content = page_content(&output_dir, DESC_MILESTONE);
    assert!(!content.contains("- Blocks\n"),
        "Target page must NOT contain forward '- Blocks' section");
}

// ── HP3: Item with no links → no relationship sections ───────────────────────

#[test]
fn test_item_with_no_links_has_no_relationship_sections() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[
        (ITEM_TASK, "task", DESC_TASK),
        (ITEM_MILESTONE, "milestone", DESC_MILESTONE),
        (ITEM_RISK, "risk", DESC_RISK),
    ]);
    // Only link task → milestone; risk has no links
    seed_link_event(&dir, ITEM_TASK, "task", "blocks", ITEM_MILESTONE, "milestone");

    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();
    run_export(&dir, &output_dir);

    let content = page_content(&output_dir, DESC_RISK);
    assert!(!content.contains("- Blocks"),       "Unlinked item must not have Blocks section");
    assert!(!content.contains("- Blocked By"),   "Unlinked item must not have Blocked By section");
    assert!(!content.contains("- Affects"),      "Unlinked item must not have Affects section");
    assert!(!content.contains("- Related To"),   "Unlinked item must not have Related To section");
}

#[test]
fn test_item_with_no_links_has_type_and_uuid_traceability() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[
        (ITEM_RISK, "risk", DESC_RISK),
    ]);

    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();
    run_export(&dir, &output_dir);

    let content = page_content(&output_dir, DESC_RISK);
    assert!(content.contains("tags:: risk"),
        "Page must have 'tags:: risk' for type-based navigation");
    assert!(content.contains(&format!("- item-id: {}", ITEM_RISK)),
        "Page must have '- item-id: <uuid>' plain-text bullet for traceability");
}

// ── HP4: Removed link → not rendered ─────────────────────────────────────────

#[test]
fn test_removed_link_not_rendered_on_source_page() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[
        (ITEM_RISK, "risk", DESC_RISK),
        (ITEM_MILESTONE, "milestone", DESC_MILESTONE),
    ]);
    seed_link_event(&dir, ITEM_RISK, "risk", "affects", ITEM_MILESTONE, "milestone");
    seed_unlink_event(&dir, ITEM_RISK, "affects", ITEM_MILESTONE);

    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();
    run_export(&dir, &output_dir);

    let content = page_content(&output_dir, DESC_RISK);
    assert!(!content.contains("- Affects"),
        "Source page must not show Affects section after the link was removed");
}

#[test]
fn test_removed_link_not_rendered_on_target_page() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[
        (ITEM_RISK, "risk", DESC_RISK),
        (ITEM_MILESTONE, "milestone", DESC_MILESTONE),
    ]);
    seed_link_event(&dir, ITEM_RISK, "risk", "affects", ITEM_MILESTONE, "milestone");
    seed_unlink_event(&dir, ITEM_RISK, "affects", ITEM_MILESTONE);

    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();
    run_export(&dir, &output_dir);

    let content = page_content(&output_dir, DESC_MILESTONE);
    assert!(!content.contains("- Affected By"),
        "Target page must not show Affected By section after the link was removed");
}

// ── HP5: Multiple link types → separate sections ─────────────────────────────

#[test]
fn test_multiple_link_types_render_as_separate_sections() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[
        (ITEM_TASK, "task", DESC_TASK),
        (ITEM_MILESTONE, "milestone", DESC_MILESTONE),
        (ITEM_RISK, "risk", DESC_RISK),
    ]);
    seed_link_event(&dir, ITEM_TASK, "task", "blocks", ITEM_MILESTONE, "milestone");
    // risk affects task — so task gets an incoming link too
    seed_link_event(&dir, ITEM_RISK, "risk", "affects", ITEM_TASK, "task");

    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();
    run_export(&dir, &output_dir);

    let content = page_content(&output_dir, DESC_TASK);
    // task is the source of "blocks" and the target of "affects"
    assert!(content.contains("- Blocks"),      "Task page must have Blocks section (outgoing)");
    assert!(content.contains("- Affected By"), "Task page must have Affected By section (incoming)");

    let blocks_pos   = content.find("- Blocks").unwrap();
    let affected_pos = content.find("- Affected By").unwrap();
    assert_ne!(blocks_pos, affected_pos,
        "Blocks and Affected By must be separate sections at different offsets");
}

// ── HP6: Idempotent re-export ─────────────────────────────────────────────────

#[test]
fn test_reexport_with_links_produces_identical_pages() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[
        (ITEM_TASK, "task", DESC_TASK),
        (ITEM_MILESTONE, "milestone", DESC_MILESTONE),
    ]);
    seed_link_event(&dir, ITEM_TASK, "task", "blocks", ITEM_MILESTONE, "milestone");

    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();
    run_export(&dir, &output_dir);
    let first_task      = page_content(&output_dir, DESC_TASK);
    let first_milestone = page_content(&output_dir, DESC_MILESTONE);

    run_export(&dir, &output_dir);
    let second_task      = page_content(&output_dir, DESC_TASK);
    let second_milestone = page_content(&output_dir, DESC_MILESTONE);

    assert_eq!(first_task, second_task,
        "Task page must be identical on re-export with same link state");
    assert_eq!(first_milestone, second_milestone,
        "Milestone page must be identical on re-export with same link state");
}

// ── Label correctness for all link types ──────────────────────────────────────

#[test]
fn test_affects_label_and_inverse() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[
        (ITEM_RISK, "risk", DESC_RISK),
        (ITEM_MILESTONE, "milestone", DESC_MILESTONE),
    ]);
    seed_link_event(&dir, ITEM_RISK, "risk", "affects", ITEM_MILESTONE, "milestone");

    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();
    run_export(&dir, &output_dir);

    let risk_page = page_content(&output_dir, DESC_RISK);
    let ms_page   = page_content(&output_dir, DESC_MILESTONE);

    assert!(risk_page.contains("- Affects"),    "Source must have '- Affects'");
    assert!(ms_page.contains("- Affected By"),  "Target must have '- Affected By'");
}

#[test]
fn test_related_to_label_is_symmetric() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[
        (ITEM_TASK, "task", DESC_TASK),
        (ITEM_RISK, "risk", DESC_RISK),
    ]);
    seed_link_event(&dir, ITEM_TASK, "task", "related_to", ITEM_RISK, "risk");

    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();
    run_export(&dir, &output_dir);

    let task_page = page_content(&output_dir, DESC_TASK);
    let risk_page = page_content(&output_dir, DESC_RISK);

    assert!(task_page.contains("- Related To"), "Source must have '- Related To'");
    assert!(risk_page.contains("- Related To"), "Target must also have '- Related To' (symmetric label)");
}

// ── Invariant: base page content unchanged when links added ───────────────────

#[test]
fn test_link_rendering_does_not_alter_base_page_content() {
    let dir = setup_temp_dir();
    seed_incorporated_items(&dir, SESSION_A, &[
        (ITEM_TASK, "task", DESC_TASK),
        (ITEM_MILESTONE, "milestone", DESC_MILESTONE),
    ]);

    // Export without links first
    let output_dir = dir.path().join("logseq_out").to_string_lossy().into_owned();
    run_export(&dir, &output_dir);
    let content_no_links = page_content(&output_dir, DESC_TASK);

    // Add a link and re-export
    seed_link_event(&dir, ITEM_TASK, "task", "blocks", ITEM_MILESTONE, "milestone");
    run_export(&dir, &output_dir);
    let content_with_links = page_content(&output_dir, DESC_TASK);

    // Base properties must still be present
    assert!(content_with_links.contains(&format!("- item-id: {}", ITEM_TASK)),
        "item-id must still be present as plain text bullet");
    assert!(content_with_links.contains("type:: task"),
        "type property must still be present");
    assert!(content_with_links.contains("tags:: task"),
        "tags property must still be present");
    // The new relationship section is added
    assert!(content_with_links.contains("- Blocks"),
        "Blocks section must be added by the re-export with links");
    // Base content (before relationship sections) must be unchanged prefix
    assert!(content_with_links.starts_with(content_no_links.trim_end()),
        "Base page content must be unchanged prefix when links are added");
}

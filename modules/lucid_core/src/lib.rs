use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use serde_json::Value;

pub use project_schema::{EventEnvelope, SchemaError};

pub const EVENTS_FILE: &str = "events/runtime_events.jsonl";

// ── Event emission ─────────────────────────────────────────��──────────────────

pub struct EventEmitter {
    events_file: PathBuf,
    source_module: &'static str,
}

impl EventEmitter {
    pub fn new(events_file: &Path, source_module: &'static str) -> Self {
        Self {
            events_file: events_file.to_path_buf(),
            source_module,
        }
    }

    pub fn emit(&self, event_type: &str, correlation_id: &str, payload: Value) {
        project_schema::emit_event(
            &self.events_file,
            project_schema::EventEnvelope {
                source_module: self.source_module,
                event_type,
                correlation_id,
                payload,
            },
        );
    }
}

// ── Event log reading ─────────────────────────────────────────────────────────

pub struct EventLogIter {
    inner: Option<std::io::Lines<BufReader<fs::File>>>,
}

impl Iterator for EventLogIter {
    type Item = Result<Value>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let lines = self.inner.as_mut()?;
            match lines.next() {
                None => return None,
                Some(Err(e)) => {
                    return Some(Err(anyhow::Error::from(e).context("reading event log")))
                }
                Some(Ok(line)) if line.trim().is_empty() => continue,
                Some(Ok(line)) => {
                    return Some(serde_json::from_str(&line).context("parsing event line"));
                }
            }
        }
    }
}

/// Open the event log at `path` and return a line-by-line iterator of parsed JSON values.
/// If the file does not exist, returns an empty iterator rather than an error.
/// Empty lines are skipped. Malformed JSON lines surface as `Err` items.
pub fn open_event_log(path: &Path) -> Result<EventLogIter> {
    if !path.exists() {
        return Ok(EventLogIter { inner: None });
    }
    let file = fs::File::open(path)
        .with_context(|| format!("opening event log '{}'", path.display()))?;
    Ok(EventLogIter {
        inner: Some(BufReader::new(file).lines()),
    })
}

// ── Canonical project record DTO ──────────────────────────────────────────────

/// Raw event projection of a project record item. Every field is populated
/// directly from a single event payload. No derived or computed state.
#[derive(Clone, Default)]
pub struct RecordedItem {
    pub item_id:            String,
    pub item_type:          String,
    pub description:        String,
    pub uncertain:          bool,
    pub uncertainty_reason: Option<String>,
    pub session_id:         String,
    pub parent_item_id:     Option<String>,
    pub current_marker:     Option<String>,
    pub owner_id:           Option<String>,
    pub scheduled_date:     Option<String>,
    pub deadline:           Option<String>,
}

// ── open_event_log unit tests ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_log(content: &[u8]) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(content).unwrap();
        f.flush().unwrap();
        f
    }

    // Missing file → empty iterator (no error).
    #[test]
    fn test_missing_file_yields_empty_iterator() {
        let path = std::path::Path::new("/tmp/lucid_core_test_nonexistent_file_xyz.jsonl");
        assert!(!path.exists(), "precondition: test file must not exist");
        let mut iter = open_event_log(path).expect("open_event_log must not error on missing file");
        assert!(iter.next().is_none(), "missing file must yield no items");
    }

    // Empty file → empty iterator (no error).
    #[test]
    fn test_empty_file_yields_empty_iterator() {
        let f = write_log(b"");
        let mut iter = open_event_log(f.path()).unwrap();
        assert!(iter.next().is_none());
    }

    // File with only whitespace/blank lines → empty iterator.
    #[test]
    fn test_blank_lines_only_yields_empty_iterator() {
        let f = write_log(b"\n\n   \n\t\n");
        let mut iter = open_event_log(f.path()).unwrap();
        assert!(iter.next().is_none());
    }

    // Empty lines interspersed with valid JSON → only valid JSON items returned.
    #[test]
    fn test_empty_lines_mid_log_are_skipped() {
        let f = write_log(b"\n{\"a\":1}\n\n{\"b\":2}\n\n");
        let items: Vec<Value> = open_event_log(f.path())
            .unwrap()
            .map(|r| r.expect("all non-blank lines are valid JSON"))
            .collect();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0]["a"], 1);
        assert_eq!(items[1]["b"], 2);
    }

    // Valid multi-event log → all events parsed in order.
    #[test]
    fn test_valid_multi_event_log() {
        let content = b"{\"event_type\":\"A\",\"n\":1}\n{\"event_type\":\"B\",\"n\":2}\n{\"event_type\":\"C\",\"n\":3}\n";
        let f = write_log(content);
        let items: Vec<Value> = open_event_log(f.path())
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0]["event_type"], "A");
        assert_eq!(items[1]["event_type"], "B");
        assert_eq!(items[2]["event_type"], "C");
    }

    // Truncated last line (no trailing newline) → still parsed correctly.
    #[test]
    fn test_truncated_last_line_no_trailing_newline() {
        let f = write_log(b"{\"event_type\":\"X\"}\n{\"event_type\":\"Y\"}");
        let items: Vec<Value> = open_event_log(f.path())
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(items.len(), 2);
        assert_eq!(items[1]["event_type"], "Y");
    }

    // Malformed JSON line surfaces as Err, does NOT panic or skip silently.
    #[test]
    fn test_malformed_json_line_is_err() {
        let f = write_log(b"{\"ok\":true}\nnot-valid-json\n{\"also\":\"ok\"}\n");
        let mut iter = open_event_log(f.path()).unwrap();
        assert!(iter.next().unwrap().is_ok(), "first line is valid");
        let second = iter.next().unwrap();
        assert!(second.is_err(), "malformed line must be Err, not Ok");
        let err_msg = format!("{:#}", second.unwrap_err());
        assert!(
            err_msg.contains("parsing event line"),
            "error context must mention parsing: {}", err_msg
        );
        assert!(iter.next().unwrap().is_ok(), "subsequent valid lines still parsed");
    }

    // Malformed JSON mid-log: lenient collect with filter_map drops the Err.
    #[test]
    fn test_lenient_collect_drops_malformed_lines() {
        let f = write_log(b"{\"n\":1}\nbad\n{\"n\":3}\n");
        let ok_items: Vec<Value> = open_event_log(f.path())
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert_eq!(ok_items.len(), 2);
        assert_eq!(ok_items[0]["n"], 1);
        assert_eq!(ok_items[1]["n"], 3);
    }

    // Strict collect (?) propagates Err on malformed line.
    #[test]
    fn test_strict_collect_errors_on_malformed_line() {
        let f = write_log(b"{\"n\":1}\nbad\n{\"n\":3}\n");
        let result: Result<Vec<Value>> = open_event_log(f.path()).unwrap().collect();
        assert!(result.is_err(), "strict collect must fail on malformed line");
    }
}

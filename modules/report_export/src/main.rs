use anyhow::{Context, Result};
use clap::Parser;
use project_schema::{emit_type_unknown, load_and_validate, resolve_type, EventEnvelope, ProjectSchema};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::io::BufRead;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const EVENTS_FILE: &str = "events/runtime_events.jsonl";
const SOURCE_MODULE: &str = "report_export";
const VALID_TYPES: &[&str] = &["weekly", "risk-register", "stakeholders", "full"];

const SEVEN_DAYS_MS: u64 = 7 * 24 * 60 * 60 * 1000;

#[derive(Parser)]
#[command(about = "LucidPM project report generator")]
struct Cli {
    #[arg(long = "type", value_name = "TYPE")]
    report_type: String,
    #[arg(long = "graph", value_name = "PATH")]
    graph_path: Option<String>,
}

struct ItemRecord {
    item_id: String,
    item_type: String,
    description: String,
    session_id: String,
    status: Option<String>,
    priority: Option<String>,
}

fn timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

fn format_date(ms: u64) -> String {
    let days = ms / 1000 / 86400;
    let jd = days + 2440588;
    let a = jd + 32044;
    let b = (4 * a + 3) / 146097;
    let c = a - (146097 * b) / 4;
    let d = (4 * c + 3) / 1461;
    let e = c - (1461 * d) / 4;
    let m = (5 * e + 2) / 153;
    let day   = e - (153 * m + 2) / 5 + 1;
    let month = m + 3 - 12 * (m / 10);
    let year  = 100 * b + d - 4800 + m / 10;
    format!("{:04}-{:02}-{:02}", year, month, day)
}

fn emit_event(event_type: &str, correlation_id: &str, payload: Value) {
    project_schema::emit_event(Path::new(EVENTS_FILE), EventEnvelope {
        source_module: SOURCE_MODULE,
        event_type,
        correlation_id,
        payload,
    });
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
    let mut accepted_ids: Option<Vec<String>> = None;

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

fn proposed_values_for(events: &[Value], item_id: &str) -> (Option<String>, Option<String>) {
    let candidate = events.iter().find_map(|e| {
        if e["source_module"].as_str() != Some("pm_structuring") { return None; }
        if e["event_type"].as_str() != Some("ItemsExtracted") { return None; }
        let corr_id = e["correlation_id"].as_str()?;
        let items   = e["payload"]["items"].as_array()?;
        let item    = items.iter().find(|i| i["item_id"].as_str() == Some(item_id))?;
        Some((
            corr_id.to_string(),
            item["proposed_status"].as_str().map(String::from),
            item["proposed_priority"].as_str().map(String::from),
        ))
    });
    let (corr_id, prop_status, prop_priority) = match candidate {
        Some(c) => c,
        None    => return (None, None),
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
    if confirmed { (prop_status, prop_priority) } else { (None, None) }
}

fn effective_status_priority(events: &[Value], item_id: &str) -> (Option<String>, Option<String>) {
    let mut last_status   = None;
    let mut last_priority = None;
    for e in events {
        let src = e["source_module"].as_str().unwrap_or("");
        if (src == "item_status" || src == "logseq_sync")
            && e["payload"]["item_id"].as_str() == Some(item_id)
        {
            match e["event_type"].as_str() {
                Some("ItemStatusUpdated")   => { last_status   = e["payload"]["new_status"].as_str().map(String::from); }
                Some("ItemPriorityUpdated") => { last_priority = e["payload"]["new_priority"].as_str().map(String::from); }
                _ => {}
            }
        }
    }
    if last_status.is_none() || last_priority.is_none() {
        let (ps, pp) = proposed_values_for(events, item_id);
        if last_status.is_none()   { last_status   = ps; }
        if last_priority.is_none() { last_priority = pp; }
    }
    (last_status, last_priority)
}

fn build_items(events: &[Value]) -> Vec<ItemRecord> {
    let sessions = incorporated_sessions(events);
    let mut items: Vec<ItemRecord> = sessions.iter().flat_map(|(sid, _)| {
        confirmed_items_for_session(events, sid)
            .into_iter()
            .map(|(id, ty, desc)| {
                let (status, priority) = effective_status_priority(events, &id);
                ItemRecord { item_id: id, item_type: ty, description: desc,
                             session_id: sid.clone(), status, priority }
            })
    }).collect();
    // Task instances from TaskAdded events
    for e in events {
        if e["source_module"].as_str() == Some("task_model")
            && e["event_type"].as_str() == Some("TaskAdded")
        {
            let p = &e["payload"];
            let task_id = p["task_id"].as_str().unwrap_or("").to_string();
            let item_type = p["item_type"].as_str().unwrap_or("").to_string();
            if !task_id.is_empty() && !item_type.is_empty() {
                let (status, priority) = effective_status_priority(events, &task_id);
                items.push(ItemRecord {
                    item_id: task_id,
                    item_type,
                    description: p["description"].as_str().unwrap_or("").to_string(),
                    session_id: "task_model".to_string(),
                    status,
                    priority,
                });
            }
        }
    }
    items
}

fn pri_display(p: Option<&str>) -> &str { p.unwrap_or("-") }
fn sta_display(s: Option<&str>) -> &str { s.unwrap_or("-") }

fn md_table_row(cols: &[&str]) -> String {
    format!("| {} |", cols.join(" | "))
}

fn report_full(items: &[ItemRecord], schema: &ProjectSchema, now_ms: u64) -> (String, usize) {
    // Group by canonical type; items are pre-filtered so resolve_type always succeeds.
    let mut by_type: HashMap<String, Vec<&ItemRecord>> = HashMap::new();
    for item in items {
        let ct = resolve_type(schema, &item.item_type).unwrap_or(item.item_type.as_str());
        by_type.entry(ct.to_string()).or_default().push(item);
    }

    // Sort canonical types for consistent section ordering (HP4: empty sections omitted).
    let mut canonical_types: Vec<&String> = by_type.keys().collect();
    canonical_types.sort();

    let mut out = format!("# Full Project Report\nGenerated: {}\n\n", format_date(now_ms));
    out += "## Summary\n\n";
    out += "| Type | Count |\n|---|---|\n";
    for t in &canonical_types {
        out += &format!("| {} | {} |\n", t, by_type[*t].len());
    }

    for t in &canonical_types {
        let group = &by_type[*t];
        out += &format!("\n## {}s\n\n", capitalize(t));
        out += "| Priority | Status | Description |\n|---|---|---|\n";
        for item in group.iter() {
            out += &md_table_row(&[
                pri_display(item.priority.as_deref()),
                sta_display(item.status.as_deref()),
                &item.description,
            ]);
            out += "\n";
        }
    }

    let sessions = {
        let mut seen = std::collections::HashSet::new();
        items.iter()
            .filter(|i| seen.insert(i.session_id.clone()))
            .map(|i| i.session_id.clone())
            .collect::<Vec<_>>()
    };
    out += "\n## Session History\n\n";
    out += "| Session | Items |\n|---|---|\n";
    for sid in &sessions {
        let count = items.iter().filter(|i| &i.session_id == sid).count();
        out += &format!("| {} | {} |\n", &sid[..8.min(sid.len())], count);
    }

    (out, items.len())
}

fn report_risk_register(items: &[ItemRecord], schema: &ProjectSchema, now_ms: u64) -> (String, usize) {
    // Resolve "risk" through the schema so the comparison uses the actual canonical name,
    // regardless of how the schema author chose to capitalise it.
    let target = resolve_type(schema, "risk");
    let risks: Vec<&ItemRecord> = items.iter()
        .filter(|i| target.is_some() && resolve_type(schema, &i.item_type) == target)
        .collect();
    let mut out = format!("# Risk Register\nGenerated: {}\n\n", format_date(now_ms));
    out += "| Priority | Status | Description |\n|---|---|---|\n";
    for item in &risks {
        out += &md_table_row(&[
            pri_display(item.priority.as_deref()),
            sta_display(item.status.as_deref()),
            &item.description,
        ]);
        out += "\n";
    }
    if risks.is_empty() { out += "_No risks in project record._\n"; }
    (out, risks.len())
}

fn report_stakeholders(items: &[ItemRecord], schema: &ProjectSchema, now_ms: u64) -> (String, usize) {
    let target = resolve_type(schema, "stakeholder");
    let stakeholders: Vec<&ItemRecord> = items.iter()
        .filter(|i| target.is_some() && resolve_type(schema, &i.item_type) == target)
        .collect();
    let mut out = format!("# Stakeholder Map\nGenerated: {}\n\n", format_date(now_ms));
    out += "| Status | Name |\n|---|---|\n";
    for item in &stakeholders {
        out += &md_table_row(&[
            sta_display(item.status.as_deref()),
            &item.description,
        ]);
        out += "\n";
    }
    if stakeholders.is_empty() { out += "_No stakeholders in project record._\n"; }
    (out, stakeholders.len())
}

fn report_weekly(items: &[ItemRecord], events: &[Value], schema: &ProjectSchema, now_ms: u64) -> (String, usize) {
    let week_start_ms = now_ms.saturating_sub(SEVEN_DAYS_MS);

    // Resolve target type names through the schema so comparisons use actual
    // canonical names regardless of how the schema author capitalised them.
    let task_canonical = resolve_type(schema, "task");
    let risk_canonical = resolve_type(schema, "risk");
    let milestone_canonical = resolve_type(schema, "milestone");

    let open_tasks: Vec<&ItemRecord> = items.iter()
        .filter(|i| task_canonical.is_some()
            && resolve_type(schema, &i.item_type) == task_canonical
            && !matches!(i.status.as_deref(), Some("done") | Some("cancelled")))
        .collect();
    let open_risks: Vec<&ItemRecord> = items.iter()
        .filter(|i| risk_canonical.is_some()
            && resolve_type(schema, &i.item_type) == risk_canonical
            && !matches!(i.status.as_deref(), Some("mitigated") | Some("accepted") | Some("closed")))
        .collect();
    let milestones: Vec<&ItemRecord> = items.iter()
        .filter(|i| milestone_canonical.is_some()
            && resolve_type(schema, &i.item_type) == milestone_canonical)
        .collect();

    let recent_sessions: Vec<(String, usize)> = events.iter()
        .filter(|e| {
            e["source_module"].as_str() == Some("project_state")
                && e["event_type"].as_str() == Some("ItemsIncorporated")
                && e["timestamp"].as_u64().unwrap_or(0) >= week_start_ms
        })
        .filter_map(|e| {
            let sid   = e["payload"]["session_id"].as_str()?.to_string();
            let count = e["payload"]["incorporated_count"].as_u64().unwrap_or(0) as usize;
            Some((sid, count))
        })
        .collect();

    let recent_item_count: usize = recent_sessions.iter().map(|(_, c)| c).sum();

    let mut out = format!(
        "# Weekly Status\nGenerated: {}  (period: {} to {})\n\n",
        format_date(now_ms), format_date(week_start_ms), format_date(now_ms)
    );

    let item_header = "| Priority | Status | Description |\n|---|---|---|\n";

    out += "## Open Tasks\n\n";
    out += item_header;
    for item in &open_tasks {
        out += &md_table_row(&[pri_display(item.priority.as_deref()), sta_display(item.status.as_deref()), &item.description]);
        out += "\n";
    }
    if open_tasks.is_empty() { out += "_No open tasks._\n"; }

    out += "\n## Open Risks\n\n";
    out += item_header;
    for item in &open_risks {
        out += &md_table_row(&[pri_display(item.priority.as_deref()), sta_display(item.status.as_deref()), &item.description]);
        out += "\n";
    }
    if open_risks.is_empty() { out += "_No open risks._\n"; }

    out += "\n## Milestones\n\n";
    out += item_header;
    for item in &milestones {
        out += &md_table_row(&[pri_display(item.priority.as_deref()), sta_display(item.status.as_deref()), &item.description]);
        out += "\n";
    }
    if milestones.is_empty() { out += "_No milestones._\n"; }

    out += "\n## Recent Activity (last 7 days)\n\n";
    if recent_sessions.is_empty() {
        out += "_No sessions incorporated in the last 7 days._\n";
    } else {
        out += &format!("{} item(s) incorporated across {} session(s):\n\n", recent_item_count, recent_sessions.len());
        for (sid, count) in &recent_sessions {
            out += &format!("- Session {}: {} item(s)\n", &sid[..8.min(sid.len())], count);
        }
    }

    (out, items.len())
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None    => String::new(),
        Some(f) => f.to_uppercase().to_string() + c.as_str(),
    }
}

fn report_filename(report_type: &str) -> &'static str {
    match report_type {
        "weekly"       => "Weekly Status.md",
        "risk-register"=> "Risk Register.md",
        "stakeholders" => "Stakeholder Map.md",
        "full"         => "Full Project Report.md",
        _              => "Report.md",
    }
}

fn cmd_report(report_type: &str, graph_path: Option<&str>) -> Result<()> {
    let correlation_id = Uuid::new_v4().to_string();
    let now_ms = timestamp_ms();

    // Vocabulary loading gate: occurs before ReportRequested.
    // On parse/validation failure, project_schema emits the error event and we exit.
    let schema = match load_and_validate(Path::new("."), Path::new(EVENTS_FILE), &correlation_id) {
        Some(s) => s,
        None    => return Ok(()),
    };

    emit_event("ReportRequested", &correlation_id, json!({
        "report_type": report_type,
        "graph_path":  graph_path,
    }));

    // Contract failure: InvalidReportType
    if !VALID_TYPES.contains(&report_type) {
        eprintln!("Invalid --type '{}'. Valid values: {}", report_type, VALID_TYPES.join(", "));
        emit_event("ReportFailedInvalidType", &correlation_id, json!({
            "failure_reason": "invalid_report_type",
            "report_type":    report_type,
        }));
        return Ok(());
    }

    let events = read_events()?;
    let items  = build_items(&events);

    // Contract failure: EmptyRecord — fires before any vocabulary exclusion is applied.
    if items.is_empty() {
        eprintln!("No items in project record.");
        emit_event("ReportFailedEmptyRecord", &correlation_id, json!({
            "failure_reason": "empty_record",
        }));
        return Ok(());
    }

    // Contract failure: OutputNotFound
    if let Some(gp) = graph_path {
        if !Path::new(gp).is_dir() {
            eprintln!("Graph path '{}' does not exist or is not a directory.", gp);
            emit_event("ReportFailedOutputNotFound", &correlation_id, json!({
                "failure_reason": "output_not_found",
                "graph_path":     gp,
            }));
            return Ok(());
        }
    }

    // Exclude items with unrecognized entity types (HP3/HP4/HP5).
    // emit_type_unknown (source_module: "project_schema") emitted per excluded item.
    let recognized_items: Vec<ItemRecord> = items.into_iter()
        .filter_map(|item| {
            if resolve_type(&schema, &item.item_type).is_none() {
                emit_type_unknown(
                    Path::new(EVENTS_FILE),
                    &item.item_id,
                    &item.item_type,
                    &correlation_id,
                );
                return None;
            }
            Some(item)
        })
        .collect();

    let (content, item_count) = match report_type {
        "full"          => report_full(&recognized_items, &schema, now_ms),
        "risk-register" => report_risk_register(&recognized_items, &schema, now_ms),
        "stakeholders"  => report_stakeholders(&recognized_items, &schema, now_ms),
        "weekly"        => report_weekly(&recognized_items, &events, &schema, now_ms),
        _               => unreachable!(),
    };

    let (output_destination, report_file) = if let Some(gp) = graph_path {
        let filename  = report_filename(report_type);
        let file_path = format!("{}/{}", gp.trim_end_matches('/'), filename);
        fs::write(&file_path, &content)
            .with_context(|| format!("writing report to {}", file_path))?;
        println!("Report written to: {}", file_path);
        (gp.to_string(), Some(file_path))
    } else {
        print!("{}", content);
        ("stdout".to_string(), None)
    };

    emit_event("ReportGenerated", &correlation_id, json!({
        "report_type":        report_type,
        "output_destination": output_destination,
        "report_file":        report_file,
        "item_count":         item_count,
        "generated_at":       now_ms,
    }));

    Ok(())
}

fn main() {
    let cli = Cli::parse();
    let result = cmd_report(&cli.report_type, cli.graph_path.as_deref());
    if let Err(e) = result {
        eprintln!("Error: {:#}", e);
        std::process::exit(1);
    }
}

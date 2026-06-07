use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use project_schema::{
    emit_schema_failure, is_valid_status, load_schema, resolve_type, validate, EventEnvelope,
    ProjectSchema, SchemaError,
};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::io::BufRead;
use std::path::Path;
use uuid::Uuid;

mod suggester;

const EVENTS_FILE: &str = "events/runtime_events.jsonl";

#[derive(Parser)]
#[command(about = "LucidPM ontology enrichment via AI proposals")]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Analyse the project record and generate enrichment proposals
    Propose,
    /// Confirm accepted/rejected proposals from a prior analysis
    Confirm {
        /// The review_id from the prior OntologyReviewProposed event
        review_id: String,
        /// Proposal IDs to accept (comma-separated or repeated flags)
        #[arg(long = "accept", value_delimiter = ',')]
        accept: Vec<String>,
        /// Proposal IDs to reject (comma-separated or repeated flags)
        #[arg(long = "reject", value_delimiter = ',')]
        reject: Vec<String>,
        /// Accept all proposals in the review
        #[arg(long)]
        accept_all: bool,
    },
}

fn emit(event_type: &str, source_module: &str, correlation_id: &str, payload: Value) {
    project_schema::emit_event(Path::new(EVENTS_FILE), EventEnvelope {
        source_module,
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

fn incorporated_sessions(events: &[Value]) -> Vec<String> {
    events
        .iter()
        .filter(|e| {
            e["source_module"].as_str() == Some("project_state")
                && e["event_type"].as_str() == Some("ItemsIncorporated")
        })
        .filter_map(|e| e["payload"]["session_id"].as_str().map(|s| s.to_string()))
        .collect()
}

fn items_for_session(events: &[Value], session_id: &str) -> Vec<(String, String, String)> {
    let mut extracted: Option<Vec<Value>> = None;
    let mut accepted: Option<Vec<String>> = None;
    for e in events {
        if e["correlation_id"].as_str() != Some(session_id) {
            continue;
        }
        match e["event_type"].as_str() {
            Some("ItemsExtracted") => {
                extracted = e["payload"]["items"].as_array().cloned();
            }
            Some("ExtractionConfirmed") => {
                accepted = e["payload"]["accepted_item_ids"].as_array().map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                });
            }
            _ => {}
        }
    }
    let raw = extracted.unwrap_or_default();
    let ids = accepted.unwrap_or_default();
    raw.into_iter()
        .filter(|item| {
            item["item_id"]
                .as_str()
                .map(|id| ids.contains(&id.to_string()))
                .unwrap_or(false)
        })
        .map(|item| {
            (
                item["item_id"].as_str().unwrap_or("").to_string(),
                item["item_type"].as_str().unwrap_or("").to_string(),
                item["description"].as_str().unwrap_or("").to_string(),
            )
        })
        .collect()
}

fn build_item_registry(events: &[Value]) -> HashMap<String, (String, String)> {
    let sessions = incorporated_sessions(events);
    let mut registry: HashMap<String, (String, String)> = HashMap::new();
    for sid in &sessions {
        for (id, ty, desc) in items_for_session(events, sid) {
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

fn build_active_links(events: &[Value]) -> Vec<(String, String, String)> {
    let mut links: Vec<(String, String, String)> = Vec::new();
    for event in events {
        let et = event["event_type"].as_str().unwrap_or("");
        let sm = event["source_module"].as_str().unwrap_or("");
        if et == "ItemLinked" && sm == "item_links" {
            if let (Some(src), Some(lt), Some(tgt)) = (
                event["payload"]["source_id"].as_str(),
                event["payload"]["link_type"].as_str(),
                event["payload"]["target_id"].as_str(),
            ) {
                links.push((src.to_string(), lt.to_string(), tgt.to_string()));
            }
        } else if et == "ItemUnlinked" && sm == "item_links" {
            if let (Some(src), Some(lt), Some(tgt)) = (
                event["payload"]["source_id"].as_str(),
                event["payload"]["link_type"].as_str(),
                event["payload"]["target_id"].as_str(),
            ) {
                links.retain(|(s, l, t)| !(s == src && l == lt && t == tgt));
            }
        }
    }
    links
}

fn current_status(events: &[Value], item_id: &str) -> Option<String> {
    let mut last: Option<String> = None;
    for event in events {
        if event["event_type"].as_str() == Some("ItemStatusUpdated") {
            if event["payload"]["item_id"].as_str() == Some(item_id) {
                last = event["payload"]["new_status"].as_str().map(|s| s.to_string());
            }
        }
    }
    last
}

fn current_priority(events: &[Value], item_id: &str) -> Option<String> {
    let mut last: Option<String> = None;
    for event in events {
        if event["event_type"].as_str() == Some("ItemPriorityUpdated") {
            if event["payload"]["item_id"].as_str() == Some(item_id) {
                last = event["payload"]["new_priority"].as_str().map(|s| s.to_string());
            }
        }
    }
    last
}

// ─── Hardcoded vocabulary fallback (used when no schema is configured) ────────

fn is_valid_link_type_hardcoded(link_type: &str, source_type: &str, target_type: &str) -> bool {
    match link_type {
        "blocks" => {
            matches!(source_type, "task" | "issue")
                && matches!(target_type, "task" | "milestone")
        }
        "affects" => {
            matches!(source_type, "risk" | "issue")
                && matches!(target_type, "task" | "milestone" | "stakeholder")
        }
        "assigned_to" => {
            matches!(source_type, "task" | "issue") && target_type == "stakeholder"
        }
        "mitigated_by" => source_type == "risk" && target_type == "task",
        "escalates_to" => {
            matches!(source_type, "risk" | "issue") && target_type == "stakeholder"
        }
        "related_to" => true,
        _ => false,
    }
}

fn valid_statuses_hardcoded(item_type: &str) -> &'static [&'static str] {
    match item_type {
        "task" => &["todo", "doing", "done", "waiting", "cancelled"],
        "milestone" => &["pending", "achieved", "missed"],
        "risk" => &["open", "mitigated", "accepted", "closed"],
        "issue" => &["open", "in_progress", "resolved", "closed"],
        "stakeholder" => &["active", "inactive"],
        _ => &[],
    }
}

// ─── Vocabulary-driven helpers ────────────────────────────────────────────────

/// Check whether `link_type` is valid for the given canonical source and target
/// types according to the active vocabulary.
/// Both `canonical_source` and `canonical_target` must already be resolved
/// canonical names (not aliases).
fn is_valid_link_type_vocab(
    schema: &ProjectSchema,
    link_type: &str,
    canonical_source: &str,
    canonical_target: &str,
) -> bool {
    if let Some(rel) = schema.relations.get(link_type) {
        let src_ok = rel.source.is_empty()
            || rel.source.iter().any(|t| t == canonical_source);
        let tgt_ok = rel.target.is_empty()
            || rel.target.iter().any(|t| t == canonical_target);
        src_ok && tgt_ok
    } else {
        false
    }
}

/// Returns items whose entity type resolves to a recognized vocabulary concept.
fn filter_recognized_items(
    registry: &HashMap<String, (String, String)>,
    schema: &ProjectSchema,
) -> HashMap<String, (String, String)> {
    registry
        .iter()
        .filter(|(_, (item_type, _))| resolve_type(schema, item_type).is_some())
        .map(|(id, val)| (id.clone(), val.clone()))
        .collect()
}

/// Build the vocabulary constraint string for the LLM prompt from the active
/// schema. The returned string replaces the hardcoded constraint block.
fn build_vocab_context(schema: &ProjectSchema) -> String {
    let mut s = String::new();

    s.push_str("\nOnly propose links that follow these valid type pairs:\n");
    let mut relations: Vec<(&String, _)> = schema.relations.iter().collect();
    relations.sort_by_key(|(n, _)| n.as_str());
    if relations.is_empty() {
        s.push_str("(no link types defined in vocabulary)\n");
    } else {
        for (name, rel) in &relations {
            let src = if rel.source.is_empty() {
                "any".to_string()
            } else {
                rel.source.join("|")
            };
            let tgt = if rel.target.is_empty() {
                "any".to_string()
            } else {
                rel.target.join("|")
            };
            s.push_str(&format!("- {}: ({}) -> ({})\n", name, src, tgt));
        }
    }

    s.push_str("\nOnly propose statuses valid for each item type:\n");
    let mut page_types: Vec<(&String, _)> = schema.page_types.iter().collect();
    page_types.sort_by_key(|(n, _)| n.as_str());
    for (type_name, def) in &page_types {
        let statuses = if def.allowed_statuses.is_empty() {
            let mut global: Vec<&str> = schema.statuses.keys().map(|s| s.as_str()).collect();
            global.sort();
            global.join(", ")
        } else {
            def.allowed_statuses.join(", ")
        };
        if !statuses.is_empty() {
            s.push_str(&format!("- {}: {}\n", type_name, statuses));
        }
    }

    s
}

/// Attempt to load and validate the project schema.
///
/// Returns:
///   Ok(Some(schema)) — schema loaded and valid
///   Ok(None)         — no schema configured (SchemaNotFound); caller uses
///                      hardcoded fallback (HP2 backward-compatibility)
///   Err(())          — schema present but invalid; OntologyReviewFailedSchemaInvalid
///                      and project_schema cross-module events already emitted
fn try_load_schema(correlation_id: &str) -> std::result::Result<Option<ProjectSchema>, ()> {
    match load_schema(Path::new(".")) {
        Err(SchemaError::NotFound { .. }) => Ok(None),
        Err(e) => {
            emit_schema_failure(Path::new(EVENTS_FILE), &e, correlation_id);
            emit(
                "OntologyReviewFailedSchemaInvalid",
                "ontology_suggest",
                correlation_id,
                json!({
                    "failure_reason": "schema_invalid",
                    "error_detail":   e.message(),
                }),
            );
            Err(())
        }
        Ok(schema) => match validate(&schema) {
            Err(e) => {
                emit_schema_failure(Path::new(EVENTS_FILE), &e, correlation_id);
                emit(
                    "OntologyReviewFailedSchemaInvalid",
                    "ontology_suggest",
                    correlation_id,
                    json!({
                        "failure_reason": "schema_invalid",
                        "error_detail":   e.message(),
                    }),
                );
                Err(())
            }
            Ok(()) => Ok(Some(schema)),
        },
    }
}

// ─── Snapshot ─────────────────────────────────────────────────────────────────

fn build_snapshot(
    registry: &HashMap<String, (String, String)>,
    links: &[(String, String, String)],
    events: &[Value],
) -> String {
    let mut out = String::from("# Project Record Snapshot\n\n## Items\n");
    let mut ids: Vec<&String> = registry.keys().collect();
    ids.sort();
    for id in &ids {
        let (itype, desc) = &registry[*id];
        let status = current_status(events, id).unwrap_or_else(|| "none".to_string());
        let priority = current_priority(events, id).unwrap_or_else(|| "none".to_string());
        out.push_str(&format!(
            "- id={} type={} status={} priority={} description={}\n",
            id, itype, status, priority, desc
        ));
    }
    out.push_str("\n## Existing Links\n");
    if links.is_empty() {
        out.push_str("(none)\n");
    } else {
        for (src, lt, tgt) in links {
            out.push_str(&format!("- {} --{}--> {}\n", src, lt, tgt));
        }
    }
    out
}

// ─── Proposal validation ──────────────────────────────────────────────────────

/// Validate a single proposal. Returns None if valid, Some(reason) if invalid.
///
/// When `schema` is Some, uses vocabulary-driven type/status validation.
/// When `schema` is None, uses hardcoded fallback (pre-R11 behaviour).
/// Validation is per-proposal; callers must not batch-invalidate on any result.
fn validate_proposal(
    proposal: &Value,
    registry: &HashMap<String, (String, String)>,
    active_links: &[(String, String, String)],
    events: &[Value],
    schema: Option<&ProjectSchema>,
) -> Option<String> {
    let ptype = proposal["type"].as_str().unwrap_or("");
    match ptype {
        "link" => {
            let src = proposal["source_id"].as_str().unwrap_or("");
            let lt = proposal["link_type"].as_str().unwrap_or("");
            let tgt = proposal["target_id"].as_str().unwrap_or("");
            if src.is_empty() || lt.is_empty() || tgt.is_empty() {
                return Some("link proposal missing required fields".to_string());
            }
            let src_stored_type = match registry.get(src) {
                Some((t, _)) => t.as_str(),
                None => return Some(format!("source item {} not found", src)),
            };
            let tgt_stored_type = match registry.get(tgt) {
                Some((t, _)) => t.as_str(),
                None => return Some(format!("target item {} not found", tgt)),
            };
            let link_valid = match schema {
                Some(s) => {
                    // Resolve stored types to canonical concepts before comparison
                    // (Representation Ban: no alias strings in domain logic)
                    match (
                        resolve_type(s, src_stored_type),
                        resolve_type(s, tgt_stored_type),
                    ) {
                        (Some(src_c), Some(tgt_c)) => {
                            is_valid_link_type_vocab(s, lt, src_c, tgt_c)
                        }
                        _ => false,
                    }
                }
                None => is_valid_link_type_hardcoded(lt, src_stored_type, tgt_stored_type),
            };
            if !link_valid {
                return Some(format!(
                    "invalid link type {} for {}->{}",
                    lt, src_stored_type, tgt_stored_type
                ));
            }
            if active_links.iter().any(|(s, l, t)| s == src && l == lt && t == tgt) {
                return Some("link already exists".to_string());
            }
            None
        }
        "status" => {
            let item_id = proposal["item_id"].as_str().unwrap_or("");
            let proposed = proposal["proposed_status"].as_str().unwrap_or("");
            if item_id.is_empty() || proposed.is_empty() {
                return Some("status proposal missing required fields".to_string());
            }
            let itype = match registry.get(item_id) {
                Some((t, _)) => t.as_str(),
                None => return Some(format!("item {} not found", item_id)),
            };
            let status_valid = match schema {
                Some(s) => {
                    // Resolve stored type to canonical concept before status lookup
                    match resolve_type(s, itype) {
                        Some(canonical) => is_valid_status(s, canonical, proposed),
                        None => false,
                    }
                }
                None => valid_statuses_hardcoded(itype).contains(&proposed),
            };
            if !status_valid {
                return Some(format!("invalid status {} for type {}", proposed, itype));
            }
            if current_status(events, item_id).as_deref() == Some(proposed) {
                return Some("status already set to proposed value".to_string());
            }
            None
        }
        "priority" => {
            let item_id = proposal["item_id"].as_str().unwrap_or("");
            let proposed = proposal["proposed_priority"].as_str().unwrap_or("");
            if item_id.is_empty() || proposed.is_empty() {
                return Some("priority proposal missing required fields".to_string());
            }
            if registry.get(item_id).is_none() {
                return Some(format!("item {} not found", item_id));
            }
            if !["high", "medium", "low"].contains(&proposed) {
                return Some(format!("invalid priority {}", proposed));
            }
            if current_priority(events, item_id).as_deref() == Some(proposed) {
                return Some("priority already set to proposed value".to_string());
            }
            None
        }
        other => Some(format!("unknown proposal type {}", other)),
    }
}

// ─── Commands ─────────────────────────────────────────────────────────────────

async fn cmd_propose() -> Result<()> {
    let events = read_events()?;
    let full_registry = build_item_registry(&events);
    let total_item_count = full_registry.len() as u64;
    let correlation_id = Uuid::new_v4().to_string();

    // OntologyReviewRequested is always emitted first, before any failure checks.
    // item_count reflects total record size at trigger time.
    emit(
        "OntologyReviewRequested",
        "ontology_suggest",
        &correlation_id,
        json!({ "item_count": total_item_count }),
    );

    // Failure check 1: SchemaLoadFailed (terminal)
    let schema_opt = match try_load_schema(&correlation_id) {
        Ok(s) => s,
        Err(()) => {
            eprintln!("Error: schema load failed.");
            std::process::exit(1);
        }
    };

    // Failure check 2: EmptyProjectRecord (terminal)
    if full_registry.is_empty() {
        emit(
            "OntologyReviewFailedEmptyRecord",
            "ontology_suggest",
            &correlation_id,
            json!({ "failure_reason": "empty_project_record" }),
        );
        eprintln!("Error: project record contains no items.");
        std::process::exit(1);
    }

    // Determine the analysis registry and vocabulary context.
    // When schema is loaded: filter to recognized items; build dynamic prompt.
    // When no schema (SchemaNotFound): use full registry; use hardcoded prompt.
    let (registry, vocab_context) = match &schema_opt {
        Some(schema) => {
            let recognized = filter_recognized_items(&full_registry, schema);
            let vocab = build_vocab_context(schema);
            (recognized, Some(vocab))
        }
        None => (full_registry, None),
    };

    // Failure check 3: NoRecognizedItems (terminal; only applies when schema loaded)
    if schema_opt.is_some() && registry.is_empty() {
        emit(
            "OntologyReviewFailedNoRecognizedItems",
            "ontology_suggest",
            &correlation_id,
            json!({
                "failure_reason": "no_recognized_items",
                "item_count":     total_item_count,
            }),
        );
        eprintln!("Error: no items with recognized entity types.");
        std::process::exit(1);
    }

    let active_links = build_active_links(&events);
    let snapshot = build_snapshot(&registry, &active_links, &events);

    // LLM call — only reached when all three failure checks pass
    let raw_proposals =
        match suggester::suggest_proposals(&snapshot, vocab_context.as_deref()).await {
            Ok(p) => p,
            Err(e) => {
                emit(
                    "OntologyReviewFailedLLMUnavailable",
                    "ontology_suggest",
                    &correlation_id,
                    json!({
                        "failure_reason": "llm_unavailable",
                        "error_detail":   e.to_string(),
                    }),
                );
                eprintln!("Error: LLM unavailable — {}", e);
                std::process::exit(1);
            }
        };

    // Post-generation filtering: per-proposal vocabulary validation.
    // Each invalid proposal is discarded individually (contract invariant).
    let generated_count = raw_proposals.len() as u64;
    let valid_proposals: Vec<Value> = raw_proposals
        .into_iter()
        .filter(|p| {
            validate_proposal(p, &registry, &active_links, &events, schema_opt.as_ref())
                .is_none()
        })
        .collect();

    let review_id = Uuid::new_v4().to_string();
    let proposal_count = valid_proposals.len() as u64;
    let rejected_count = generated_count - proposal_count;

    // Schema invariants enforced by construction:
    //   proposal_count == valid_proposals.len()
    //   generated_count == proposal_count + rejected_count
    emit(
        "OntologyReviewProposed",
        "ontology_suggest",
        &correlation_id,
        json!({
            "review_id":       review_id,
            "generated_count": generated_count,
            "proposal_count":  proposal_count,
            "rejected_count":  rejected_count,
            "proposals":       valid_proposals.clone(),
        }),
    );

    println!("review_id: {}", review_id);
    println!("proposals: {}", proposal_count);
    for p in &valid_proposals {
        println!();
        println!("  [{}] type={}", p["proposal_id"], p["type"]);
        match p["type"].as_str().unwrap_or("") {
            "link" => println!(
                "  {} --{}--> {}",
                p["source_id"], p["link_type"], p["target_id"]
            ),
            "status" => println!(
                "  item={} {} -> {}",
                p["item_id"], p["current_status"], p["proposed_status"]
            ),
            "priority" => println!(
                "  item={} {} -> {}",
                p["item_id"], p["current_priority"], p["proposed_priority"]
            ),
            _ => {}
        }
        println!("  rationale: {}", p["rationale"]);
    }

    Ok(())
}

fn cmd_confirm(
    review_id: &str,
    accept: Vec<String>,
    reject: Vec<String>,
    accept_all: bool,
) -> Result<()> {
    let events = read_events()?;
    let correlation_id = Uuid::new_v4().to_string();

    let review_event = events.iter().find(|e| {
        e["event_type"].as_str() == Some("OntologyReviewProposed")
            && e["payload"]["review_id"].as_str() == Some(review_id)
    });

    let review_event = match review_event {
        Some(e) => e.clone(),
        None => {
            emit(
                "OntologyConfirmFailedReviewNotFound",
                "ontology_suggest",
                &correlation_id,
                json!({
                    "failure_reason": "review_not_found",
                    "review_id":      review_id,
                }),
            );
            eprintln!("Error: review_id {} not found.", review_id);
            std::process::exit(1);
        }
    };

    let all_proposals = review_event["payload"]["proposals"]
        .as_array()
        .cloned()
        .unwrap_or_default();

    let accepted_ids: Vec<String> = if accept_all {
        all_proposals
            .iter()
            .filter_map(|p| p["proposal_id"].as_str().map(|s| s.to_string()))
            .collect()
    } else {
        accept.clone()
    };

    let rejected_ids: Vec<String> = reject.clone();

    emit(
        "OntologyConfirmRequested",
        "ontology_suggest",
        &correlation_id,
        json!({
            "review_id":    review_id,
            "accepted_ids": accepted_ids,
            "rejected_ids": rejected_ids,
        }),
    );

    // Re-read events after emitting OntologyConfirmRequested so confirm-time
    // validation sees current state.
    let events_at_confirm = read_events()?;
    let registry = build_item_registry(&events_at_confirm);
    let active_links = build_active_links(&events_at_confirm);

    let mut applied_ids: Vec<String> = Vec::new();
    let mut skipped_ids: Vec<String> = Vec::new();

    for pid in &accepted_ids {
        let proposal = match all_proposals
            .iter()
            .find(|p| p["proposal_id"].as_str() == Some(pid.as_str()))
        {
            Some(p) => p.clone(),
            None => {
                skipped_ids.push(pid.clone());
                continue;
            }
        };

        // Confirm-time validation uses hardcoded fallback (None schema).
        // Vocabulary enforcement at confirm time delegates to the owning
        // modules (item_links R4, item_status R5) per the scope boundary.
        if let Some(reason) =
            validate_proposal(&proposal, &registry, &active_links, &events_at_confirm, None)
        {
            eprintln!("Skipping {}: {}", pid, reason);
            skipped_ids.push(pid.clone());
            continue;
        }

        match proposal["type"].as_str().unwrap_or("") {
            "link" => {
                let src = proposal["source_id"].as_str().unwrap_or("");
                let lt = proposal["link_type"].as_str().unwrap_or("");
                let tgt = proposal["target_id"].as_str().unwrap_or("");
                let src_type = registry
                    .get(src)
                    .map(|(t, _)| t.as_str())
                    .unwrap_or("unknown");
                let tgt_type = registry
                    .get(tgt)
                    .map(|(t, _)| t.as_str())
                    .unwrap_or("unknown");
                emit(
                    "ItemLinked",
                    "item_links",
                    &correlation_id,
                    json!({
                        "source_id":   src,
                        "source_type": src_type,
                        "link_type":   lt,
                        "target_id":   tgt,
                        "target_type": tgt_type,
                    }),
                );
                applied_ids.push(pid.clone());
            }
            "status" => {
                let item_id = proposal["item_id"].as_str().unwrap_or("");
                let (itype, _) = registry
                    .get(item_id)
                    .map(|(t, d)| (t.as_str(), d.as_str()))
                    .unwrap_or(("unknown", ""));
                let previous = current_status(&events_at_confirm, item_id);
                let new_status = proposal["proposed_status"].as_str().unwrap_or("");
                emit(
                    "ItemStatusUpdated",
                    "item_status",
                    &correlation_id,
                    json!({
                        "item_id":         item_id,
                        "item_type":       itype,
                        "new_status":      new_status,
                        "previous_status": previous,
                    }),
                );
                applied_ids.push(pid.clone());
            }
            "priority" => {
                let item_id = proposal["item_id"].as_str().unwrap_or("");
                let previous = current_priority(&events_at_confirm, item_id);
                let new_priority = proposal["proposed_priority"].as_str().unwrap_or("");
                emit(
                    "ItemPriorityUpdated",
                    "item_status",
                    &correlation_id,
                    json!({
                        "item_id":          item_id,
                        "new_priority":     new_priority,
                        "previous_priority": previous,
                    }),
                );
                applied_ids.push(pid.clone());
            }
            _ => {
                skipped_ids.push(pid.clone());
            }
        }
    }

    emit(
        "OntologyReviewConfirmed",
        "ontology_suggest",
        &correlation_id,
        json!({
            "review_id":      review_id,
            "accepted_count": applied_ids.len() as u64,
            "rejected_count": rejected_ids.len() as u64,
            "skipped_count":  skipped_ids.len() as u64,
            "accepted_ids":   applied_ids,
            "rejected_ids":   rejected_ids,
            "skipped_ids":    skipped_ids,
        }),
    );

    println!("Confirmed review {}.", review_id);

    Ok(())
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        Cmd::Propose => cmd_propose().await,
        Cmd::Confirm {
            review_id,
            accept,
            reject,
            accept_all,
        } => cmd_confirm(&review_id, accept, reject, accept_all),
    };
    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

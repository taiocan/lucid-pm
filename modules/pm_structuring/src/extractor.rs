use anyhow::{Context, Result};
use project_schema::{canonical_task_block_type, is_block_type, resolve_type, ProjectSchema};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashSet;
use uuid::Uuid;

pub struct WpRecord {
    pub uuid: String,
    pub description: String,
}

pub struct ExtractedItem {
    pub item_id: String,
    pub item_type: String,
    pub description: String,
    pub uncertain: bool,
    pub uncertainty_reason: Option<String>,
    pub proposed_status: Option<String>,
    pub proposed_priority: Option<String>,
    pub parent_item_id: Option<String>,
    pub initial_marker: Option<String>,
}

#[derive(Deserialize)]
struct ExtractionResult {
    items: Vec<RawItem>,
}

#[derive(Deserialize)]
struct RawItem {
    item_type: String,
    description: String,
    uncertain: bool,
    uncertainty_reason: Option<String>,
    #[serde(default)]
    proposed_status: Option<String>,
    #[serde(default)]
    proposed_priority: Option<String>,
    #[serde(default)]
    parent_item_id: Option<String>,
}

const VALID_PRIORITIES: &[&str] = &["high", "medium", "low"];

// Check if `status` is valid for `item_type` per the active vocabulary.
// Requires non-empty allowedStatuses; falls back to false if type has none defined.
// Handles aliases via resolve_type.
fn vocabulary_allows_proposed_status(schema: &ProjectSchema, item_type: &str, status: &str) -> bool {
    let canonical = match resolve_type(schema, item_type) {
        Some(t) => t,
        None => return false,
    };
    schema
        .page_types
        .get(canonical)
        .map(|def| {
            !def.allowed_statuses.is_empty()
                && def.allowed_statuses.iter().any(|s| s == status)
        })
        .unwrap_or(false)
}

// Primary display name for a type in the LLM prompt: first alias if present, else canonical.
// Preferring aliases preserves the lowercase naming convention used in historical event logs
// (e.g., "task" rather than "Task"), maintaining backward compatibility.
fn display_name<'a>(canonical: &'a str, def: &'a project_schema::PageTypeDef) -> &'a str {
    def.aliases.first().map(|s| s.as_str()).unwrap_or(canonical)
}

// Select the default active-equivalent marker from a task blockType marker vocabulary.
// Priority: (1) marker mapping to "todo" status, (2) first non-terminal marker
// alphabetically, (3) first marker alphabetically. This avoids selecting terminal
// markers (done, cancelled) as the default initial state for extracted tasks.
fn default_active_marker(markers: &std::collections::HashMap<String, String>) -> Option<String> {
    let terminal = &["done", "cancelled", "closed", "resolved", "achieved", "missed", "inactive", "accepted", "mitigated"];
    let mut sorted: Vec<(&str, &str)> = markers.iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    sorted.sort_by_key(|(k, _)| *k);

    if let Some((k, _)) = sorted.iter().find(|(_, v)| *v == "todo") {
        return Some(k.to_string());
    }
    if let Some((k, _)) = sorted.iter().find(|(_, v)| !terminal.contains(v)) {
        return Some(k.to_string());
    }
    sorted.first().map(|(k, _)| k.to_string())
}

// Build the type classification list for the LLM prompt from the active vocabulary.
// Includes both page types and block types. Page types whose canonical key matches
// a block type key (case-insensitively) are omitted — the block type takes precedence,
// ensuring extracted task items use the blockType canonical name and route to nested
// block rendering rather than page rendering.
fn build_type_list(schema: &ProjectSchema) -> String {
    let block_type_lower: HashSet<String> =
        schema.block_types.keys().map(|k| k.to_lowercase()).collect();

    let mut names: Vec<String> = Vec::new();

    // Page types: skip those superseded by a same-name block type.
    let mut page_canonical: Vec<&String> = schema
        .page_types
        .keys()
        .filter(|k| !block_type_lower.contains(&k.to_lowercase()))
        .collect();
    page_canonical.sort();
    for canonical in page_canonical {
        let def = &schema.page_types[canonical];
        names.push(display_name(canonical, def).to_string());
    }

    // Block types: always included by canonical key.
    let mut block_canonical: Vec<&String> = schema.block_types.keys().collect();
    block_canonical.sort();
    for canonical in block_canonical {
        names.push(canonical.clone());
    }

    names.sort();
    names.join(", ")
}

// Build the proposed_status vocabulary section for the LLM prompt.
fn build_status_section(schema: &ProjectSchema) -> String {
    let mut types: Vec<&String> = schema.page_types.keys().collect();
    types.sort();
    let lines: Vec<String> = types
        .into_iter()
        .filter_map(|canonical| {
            let def = &schema.page_types[canonical];
            if def.allowed_statuses.is_empty() {
                None
            } else {
                let name = display_name(canonical, def);
                Some(format!("- {}: {}", name, def.allowed_statuses.join(", ")))
            }
        })
        .collect();
    if lines.is_empty() {
        "(no specific status vocabulary defined; use null)".to_string()
    } else {
        lines.join("\n")
    }
}

fn build_system_prompt(schema: &ProjectSchema, wp_items: &[WpRecord]) -> String {
    let type_list = build_type_list(schema);
    let status_section = build_status_section(schema);

    let wp_section = if wp_items.is_empty() {
        String::new()
    } else {
        let mut lines: Vec<String> = vec![
            String::new(),
            String::from("Work Package Attribution:"),
            String::from("The following work packages exist in the project record:"),
        ];
        for wp in wp_items {
            lines.push(format!("  UUID: {} — {}", wp.uuid, wp.description));
        }
        lines.push(String::new());
        lines.push(String::from(
            "For task items: if the text unambiguously places a task under one of the above work \
             packages (via a heading directly above the task or an explicit name reference in the \
             task text), set parent_item_id to that work package's UUID. \
             Otherwise set parent_item_id to null. Do not guess.",
        ));
        lines.join("\n")
    };

    format!(
        r#"You are a project management assistant. Extract project management elements from text.

For each element found, classify it as exactly one of the following types: {type_list}

Rules:
- Only extract information explicitly present in the text. Do not infer or invent.
- If wording is ambiguous, mark the item as uncertain and explain why.
- If no project management elements are present, return an empty items array.
- Use only the exact type names listed above (or their aliases shown in parentheses).

For each item, also propose an initial status and priority if the text provides a basis:

proposed_status — infer from text context. Valid values by type:
{status_section}
Use null if the text gives no clear indication.

proposed_priority — infer urgency or importance from text. Valid values: "high", "medium", "low".
Use null if the text gives no clear indication.
{wp_section}
Return ONLY a JSON object with this exact structure (no other text):
{{
  "items": [
    {{
      "item_type": "one of the types listed above",
      "description": "exact description from text",
      "uncertain": false,
      "uncertainty_reason": null,
      "proposed_status": "status_value or null",
      "proposed_priority": "high|medium|low or null",
      "parent_item_id": "work_package_uuid or null"
    }}
  ]
}}"#
    )
}

// Validate each raw item's type against the vocabulary and sanitize proposed values.
// Contract: unrecognized type → item_type="unknown", uncertain=true, proposed_status=null.
// Contract: alias-produced types are stored as-is (no normalization).
// F16: parent_item_id validated against known WP UUIDs; initial_marker derived for task block items.
fn process_raw_item(
    schema: &ProjectSchema,
    raw: RawItem,
    valid_wp_uuids: &HashSet<String>,
    task_canonical: Option<&str>,
    default_marker: Option<&str>,
) -> ExtractedItem {
    let (stored_type, uncertain, uncertainty_reason) = match resolve_type(schema, &raw.item_type) {
        Some(_) => {
            // Recognized type (canonical or alias) — store exactly as the LLM produced it.
            (raw.item_type.clone(), raw.uncertain, raw.uncertainty_reason)
        }
        None => {
            // Unrecognized type — use sentinel, override uncertain=true, null proposed_status.
            let reason = format!(
                "type not recognized by active vocabulary: {}",
                raw.item_type
            );
            ("unknown".to_string(), true, Some(reason))
        }
    };

    // Proposed status: null when type is "unknown"; vocabulary-validated otherwise.
    // Ordering rule: type resolution precedes status validation.
    let proposed_status = if stored_type == "unknown" {
        None
    } else {
        raw.proposed_status.and_then(|s| {
            if vocabulary_allows_proposed_status(schema, &stored_type, &s) {
                Some(s)
            } else {
                None
            }
        })
    };

    // Proposed priority: unaffected by type resolution failure.
    let proposed_priority = raw.proposed_priority.and_then(|p| {
        if VALID_PRIORITIES.contains(&p.as_str()) { Some(p) } else { None }
    });

    // Determine if this item is the canonical task block type.
    // Representation Ban: uses is_block_type + resolve_type via vocabulary API.
    let item_is_task = stored_type != "unknown"
        && is_block_type(schema, &stored_type)
        && task_canonical
            .map_or(false, |tc| resolve_type(schema, &stored_type) == Some(tc));

    // parent_item_id: task items only; validated against known WP UUIDs to prevent
    // the LLM from hallucinating UUIDs not present in the project record.
    let parent_item_id = if item_is_task {
        raw.parent_item_id.and_then(|id| {
            if valid_wp_uuids.contains(&id) { Some(id) } else { None }
        })
    } else {
        None
    };

    // initial_marker: schema-derived default for task items; null for all others.
    let initial_marker = if item_is_task {
        default_marker.map(str::to_string)
    } else {
        None
    };

    ExtractedItem {
        item_id: Uuid::new_v4().to_string(),
        item_type: stored_type,
        description: raw.description,
        uncertain,
        uncertainty_reason,
        proposed_status,
        proposed_priority,
        parent_item_id,
        initial_marker,
    }
}

fn gemini_api_key() -> Result<String> {
    for var in &["GEMINI_API_KEY_PMCLI", "GEMINI_API_KEY"] {
        if let Ok(key) = std::env::var(var) {
            if !key.is_empty() {
                return Ok(key);
            }
        }
    }
    anyhow::bail!("No Gemini API key found. Set GEMINI_API_KEY_PMCLI or GEMINI_API_KEY.")
}

pub async fn extract_items(
    source_text: &str,
    schema: &ProjectSchema,
    wp_items: &[WpRecord],
) -> Result<Vec<ExtractedItem>> {
    let api_key = gemini_api_key()?;

    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent?key={api_key}"
    );

    let system_prompt = build_system_prompt(schema, wp_items);

    let user_message = format!(
        "Extract structured project management elements from the following text:\n\n---\n{source_text}\n---"
    );

    let body = json!({
        "systemInstruction": {
            "parts": [{ "text": system_prompt }]
        },
        "contents": [{
            "role": "user",
            "parts": [{ "text": user_message }]
        }],
        "generationConfig": {
            "maxOutputTokens": 8192,
            "temperature": 0.1,
            "thinkingConfig": {
                "thinkingBudget": 0
            }
        }
    });

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(50))
        .build()
        .context("building HTTP client")?;
    let response = client
        .post(&url)
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .context("calling Gemini API")?;

    let status = response.status();
    let json: Value = response.json().await.context("parsing Gemini response")?;

    if !status.is_success() {
        let error = json["error"]["message"].as_str().unwrap_or("unknown error");
        anyhow::bail!("Gemini API error {}: {}", status, error);
    }

    let text = json["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("No text in Gemini response: {}", json))?;

    let text = text.trim();
    let text = text
        .strip_prefix("```json")
        .or_else(|| text.strip_prefix("```"))
        .unwrap_or(text)
        .trim_end_matches("```")
        .trim();

    let extraction: ExtractionResult = serde_json::from_str(text)
        .with_context(|| format!("Failed to parse extraction result. Response was: {}", text))?;

    // Pre-compute task type and default active marker once for all items.
    let (task_canonical, default_marker): (Option<&str>, Option<String>) =
        match canonical_task_block_type(schema) {
            Some((tc, markers)) => (Some(tc), default_active_marker(markers)),
            None => (None, None),
        };

    let valid_wp_uuids: HashSet<String> = wp_items.iter().map(|w| w.uuid.clone()).collect();

    let items = extraction
        .items
        .into_iter()
        .map(|raw| {
            process_raw_item(schema, raw, &valid_wp_uuids, task_canonical, default_marker.as_deref())
        })
        .collect();

    Ok(items)
}

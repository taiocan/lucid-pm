use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

pub struct ExtractedItem {
    pub item_id: String,
    pub item_type: String,
    pub description: String,
    pub uncertain: bool,
    pub uncertainty_reason: Option<String>,
    pub proposed_status: Option<String>,
    pub proposed_priority: Option<String>,
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
}

fn valid_statuses(item_type: &str) -> &'static [&'static str] {
    match item_type {
        "task"        => &["todo", "doing", "done", "waiting", "cancelled"],
        "milestone"   => &["pending", "achieved", "missed"],
        "risk"        => &["open", "mitigated", "accepted", "closed"],
        "issue"       => &["open", "in_progress", "resolved", "closed"],
        "stakeholder" => &["active", "inactive"],
        _             => &[],
    }
}

const VALID_PRIORITIES: &[&str] = &["high", "medium", "low"];

fn sanitize_proposed(item_type: &str, status: Option<String>, priority: Option<String>)
    -> (Option<String>, Option<String>)
{
    let status = status.and_then(|s| {
        if valid_statuses(item_type).contains(&s.as_str()) { Some(s) } else { None }
    });
    let priority = priority.and_then(|p| {
        if VALID_PRIORITIES.contains(&p.as_str()) { Some(p) } else { None }
    });
    (status, priority)
}

const SYSTEM_PROMPT: &str = r#"You are a project management assistant. Extract project management elements from text.

For each element found, classify it as exactly one of: task, milestone, risk, issue, stakeholder.
- task: a concrete action or work item to be done
- milestone: a significant checkpoint or delivery date
- risk: a potential problem that could affect the project
- issue: an existing problem currently affecting the project
- stakeholder: a person, team, or organization involved in or affected by the project

Rules:
- Only extract information explicitly present in the text. Do not infer or invent.
- If wording is ambiguous, mark the item as uncertain and explain why.
- If no project management elements are present, return an empty items array.

For each item, also propose an initial status and priority if the text provides a basis:

proposed_status — infer from text context. Valid values by type:
- task: "todo", "doing", "done", "waiting", "cancelled"
- milestone: "pending", "achieved", "missed"
- risk: "open", "mitigated", "accepted", "closed"
- issue: "open", "in_progress", "resolved", "closed"
- stakeholder: "active", "inactive"
Use null if the text gives no clear indication.

proposed_priority — infer urgency or importance from text. Valid values: "high", "medium", "low".
Use null if the text gives no clear indication.

Return ONLY a JSON object with this exact structure (no other text):
{
  "items": [
    {
      "item_type": "task|milestone|risk|issue|stakeholder",
      "description": "exact description from text",
      "uncertain": false,
      "uncertainty_reason": null,
      "proposed_status": "todo|doing|...|null",
      "proposed_priority": "high|medium|low|null"
    }
  ]
}"#;

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

pub async fn extract_items(source_text: &str) -> Result<Vec<ExtractedItem>> {
    let api_key = gemini_api_key()?;

    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent?key={api_key}"
    );

    let user_message = format!(
        "Extract structured project management elements from the following text:\n\n---\n{source_text}\n---"
    );

    let body = json!({
        "systemInstruction": {
            "parts": [{ "text": SYSTEM_PROMPT }]
        },
        "contents": [{
            "role": "user",
            "parts": [{ "text": user_message }]
        }],
        "generationConfig": {
            "maxOutputTokens": 2048,
            "temperature": 0.1
        }
    });

    let client = reqwest::Client::new();
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

    // Strip markdown code fences if model wraps output
    let text = text.trim();
    let text = text
        .strip_prefix("```json")
        .or_else(|| text.strip_prefix("```"))
        .unwrap_or(text)
        .trim_end_matches("```")
        .trim();

    let extraction: ExtractionResult = serde_json::from_str(text)
        .with_context(|| format!("Failed to parse extraction result. Response was: {}", text))?;

    let items = extraction
        .items
        .into_iter()
        .map(|raw| {
            let (proposed_status, proposed_priority) =
                sanitize_proposed(&raw.item_type, raw.proposed_status, raw.proposed_priority);
            ExtractedItem {
                item_id: Uuid::new_v4().to_string(),
                item_type: raw.item_type,
                description: raw.description,
                uncertain: raw.uncertain,
                uncertainty_reason: raw.uncertainty_reason,
                proposed_status,
                proposed_priority,
            }
        })
        .collect();

    Ok(items)
}

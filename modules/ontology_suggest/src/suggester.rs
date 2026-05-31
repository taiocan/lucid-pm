use anyhow::{bail, Context, Result};
use serde_json::Value;

const GEMINI_URL: &str =
    "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent";

const SYSTEM_PROMPT: &str = r#"You are a project ontology analyst. Given a snapshot of a project record, identify enrichment opportunities.

Return ONLY a JSON object with a "proposals" array. Each proposal must have:
- "proposal_id": unique string like "p-001", "p-002", etc.
- "type": one of "link", "status", or "priority"
- "rationale": concise explanation of why this enrichment is appropriate

For type "link", also include:
- "source_id": exact UUID of the source item
- "source_type": item type of the source item
- "link_type": one of "blocks", "affects", "assigned_to", "mitigated_by", "escalates_to", "related_to"
- "target_id": exact UUID of the target item
- "target_type": item type of the target item

For type "status", also include:
- "item_id": exact UUID of the item
- "current_status": current status string or null
- "proposed_status": the status to set

For type "priority", also include:
- "item_id": exact UUID of the item
- "current_priority": current priority string or null
- "proposed_priority": one of "high", "medium", "low"

Only propose links that follow these valid type pairs:
- blocks: (task|issue) -> (task|milestone)
- affects: (risk|issue) -> (task|milestone|stakeholder)
- assigned_to: (task|issue) -> stakeholder
- mitigated_by: risk -> task
- escalates_to: (risk|issue) -> stakeholder
- related_to: any -> any

Only propose statuses valid for each item type:
- task: todo, doing, done, waiting, cancelled
- milestone: pending, achieved, missed
- risk: open, mitigated, accepted, closed
- issue: open, in_progress, resolved, closed
- stakeholder: active, inactive

Do not propose links that already exist. Do not propose setting a status or priority to its current value.
If there are no enrichment opportunities, return {"proposals": []}.
Return only valid JSON. No markdown, no explanation outside the JSON."#;

fn gemini_api_key() -> Result<String> {
    std::env::var("GEMINI_API_KEY_PMCLI")
        .or_else(|_| std::env::var("GEMINI_API_KEY"))
        .context("GEMINI_API_KEY_PMCLI or GEMINI_API_KEY must be set")
}

pub async fn suggest_proposals(snapshot: &str) -> Result<Vec<Value>> {
    let api_key = gemini_api_key()?;
    let url = format!("{}?key={}", GEMINI_URL, api_key);

    let body = serde_json::json!({
        "systemInstruction": {
            "parts": [{ "text": SYSTEM_PROMPT }]
        },
        "contents": [{
            "role": "user",
            "parts": [{ "text": snapshot }]
        }],
        "generationConfig": {
            "temperature": 0.1,
            "maxOutputTokens": 16384
        }
    });

    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .context("sending request to Gemini API")?;

    if !response.status().is_success() {
        bail!("Gemini API returned status {}", response.status());
    }

    let response_json: Value = response.json().await.context("parsing Gemini response")?;

    let text = response_json
        .pointer("/candidates/0/content/parts/0/text")
        .and_then(|v| v.as_str())
        .context("extracting text from Gemini response")?;

    let cleaned = text
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    let parsed: Value = serde_json::from_str(cleaned).context("parsing proposals JSON")?;

    let proposals = parsed
        .get("proposals")
        .and_then(|v| v.as_array())
        .context("proposals field missing or not an array")?
        .clone();

    Ok(proposals)
}

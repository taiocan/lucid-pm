# F9: `ontology_suggest`

**Tier**: 5 — AI Assistance
**Depends on**: project_state, F1, F7
**Event spine impact**: New schema + reader refinements
**Status**: COMPLETE

---

**What this is — and why not a pm_structuring upgrade**

`pm_structuring` extracts items from raw text — its input is unstructured prose, its output is structured items. Its schema and contract are approved and stable.

`ontology_suggest` operates on the already-structured record — its input is the full set of existing items, links, statuses, and priorities. It is a distinct second phase of structuring: a human-in-the-loop AI enrichment pass that runs on demand, independently of any new text input. Merging it into pm_structuring would require re-running Stages 2–8 on a stable, complete feature and would mix two semantically different concerns (text extraction vs. record enrichment) in one module boundary. It belongs as a separate feature.

**Intent sketch**

The PM can ask an LLM to analyse the full project record and receive structured proposals for: typed links between items, status updates, and priority assignments. Each proposal includes the LLM's rationale. The PM reviews proposals individually, accepts or rejects each, and confirmed proposals are applied as proper events — indistinguishable to all downstream features from changes made manually.

**Two-phase workflow**

```
ontology_suggest propose
```
- Reads all items, current links, statuses, priorities from the event log
- Sends a structured project snapshot to the LLM
- LLM returns proposals: links, status changes, priority assignments
- Emits `OntologyReviewRequested` + `OntologyReviewProposed` (with review_id and proposals[])
- Prints a numbered proposal list with rationale for PM review

```
ontology_suggest confirm --review-id <id> --accept p-001,p-003
ontology_suggest confirm --review-id <id> --accept-all
```
- Reads the identified `OntologyReviewProposed` event from the log
- For each accepted proposal: emits the appropriate behavioral event via the owning module's event pattern
- Emits `OntologyReviewConfirmed` with `accepted_ids[]` and `rejected_ids[]`

**Proposal types**

| Type | LLM infers | Resulting behavioral event |
|---|---|---|
| `link` | A typed relationship between two items | `ItemLinked` |
| `status` | A more appropriate status given the item's description and context | `ItemStatusUpdated` |
| `priority` | A priority level given urgency/impact language | `ItemPriorityUpdated` |

**Proposal payload shape**

```json
{
  "review_id": "uuid-v4",
  "proposal_count": 4,
  "proposals": [
    {
      "proposal_id": "p-001",
      "type": "link",
      "source_id": "...", "link_type": "blocks", "target_id": "...",
      "rationale": "Bug must be resolved before the release milestone can close"
    },
    {
      "proposal_id": "p-002",
      "type": "status",
      "item_id": "...", "current_status": "open", "proposed_status": "in_progress",
      "rationale": "Description implies active work is underway"
    },
    {
      "proposal_id": "p-003",
      "type": "priority",
      "item_id": "...", "current_priority": null, "proposed_priority": "high",
      "rationale": "Customer-facing impact and urgency language in description"
    }
  ]
}
```

**Compatibility note (key design constraint)**

Confirmed `ItemLinked` events must be readable by `item_links list`, `logseq_export_links`, and `report_export`. Confirmed `ItemStatusUpdated`/`ItemPriorityUpdated` events must be readable by `item_status get`, `logseq_export`, and `report_export`. All of these readers currently filter by `source_module`.

Two valid implementation approaches for Stage 4:
1. **Emit with the owning source_module** — `ItemLinked` with `source_module: "item_links"`, `ItemStatusUpdated` with `source_module: "item_status"`. Requires no reader changes. Semantically slightly impure (ontology_suggest impersonates the owning module).
2. **Emit with `source_module: "ontology_suggest"` and add Stage 9 refinements** — Update each reader to accept "ontology_suggest" as a recognised source, following the existing `logseq_sync` → `item_status` pattern. Semantically clean. Requires reader amendments.

Stage 2 (contract derivation) will resolve this choice.

**Link validation**
Before including a link proposal in output, validate it against the F7 type matrix. Invalid combinations (e.g., task `affects` stakeholder) are silently dropped — the LLM may hallucinate invalid pairings, but the PM only sees conformant proposals.

**Failure modes**
- `OntologyReviewFailedEmptyRecord` — no items in the project record
- `OntologyReviewFailedLLMUnavailable` — LLM API unreachable or returned unusable output
- `OntologyConfirmFailedReviewNotFound` — the referenced review_id is not in the event log

**Relationship to R1**
R1 (`pm_structuring` Stage 9) proposes status and priority at extraction time for newly extracted items — based on the meeting text. F9 proposes across all existing items based on the full record context. They are complementary: R1 gives a first pass at extraction; F9 gives a holistic review pass on demand.

**Event spine (new)**

```
OntologyReviewRequested        ← OBSERVATIONAL
  ↓
  ├─ (record empty)
  │    OntologyReviewFailedEmptyRecord
  │
  ├─ (LLM unavailable)
  │    OntologyReviewFailedLLMUnavailable
  │
  └─ (proposals generated)
       OntologyReviewProposed   ← BEHAVIORAL: proposals[] with review_id

OntologyConfirmRequested       ← OBSERVATIONAL (separate invocation)
  ↓
  ├─ (review_id not found)
  │    OntologyConfirmFailedReviewNotFound
  │
  └─ (review found)
       ItemLinked × N           ← for accepted link proposals
       ItemStatusUpdated × N    ← for accepted status proposals
       ItemPriorityUpdated × N  ← for accepted priority proposals
       OntologyReviewConfirmed  ← BEHAVIORAL: accepted_ids[], rejected_ids[]
```

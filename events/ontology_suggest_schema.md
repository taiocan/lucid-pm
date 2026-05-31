# Event Schema: ontology_suggest

<!--
DERIVED FROM:
- intents/ontology_suggest.md
- contracts/ontology_suggest_contract.md
-->

## Naming Convention

See `.codeos/templates/conventions.md`.

## Required Base Fields (all events)

```json
{
  "event_id": "uuid-v4",
  "event_type": "EventName",
  "timestamp": 1710000000000,
  "correlation_id": "uuid-v4",
  "source_module": "ontology_suggest",
  "payload": {}
}
```

`correlation_id` is mandatory and must propagate through the entire execution
chain. The analyse phase and the confirm phase each have their own correlation_id
(they are separate invocations).

## Delegated Events (not owned by this schema)

Accepted proposals produce the following events. These are owned by their
respective modules and carry **their** source_module, not "ontology_suggest":

| Event | source_module | Emitted when |
|---|---|---|
| `ItemLinked` | `item_links` | An accepted link proposal passes confirm-time validation |
| `ItemStatusUpdated` | `item_status` | An accepted status proposal is applied |
| `ItemPriorityUpdated` | `item_status` | An accepted priority proposal is applied |

These events are not redefined here — they conform exactly to the schemas in
`events/item_links_schema.md` and `events/item_status_schema.md`.

---

## Event Definitions

### OntologyReviewRequested

- category: OBSERVATIONAL
- emitted when: the PM triggers an AI analysis of the project record
- payload:
  - `item_count`: `u32` — number of items in the project record at analysis time

### OntologyReviewProposed

- category: BEHAVIORAL
- emitted when: the AI analysis completes and proposals are ready for PM review
- payload:
  - `review_id`: `string` (uuid-v4) — stable identifier for this proposal set; used in the confirm phase
  - `proposal_count`: `u32` — number of proposals in this review (may be zero)
  - `proposals`: `array<Proposal>` — ordered list of proposals

  **Proposal object shape:**
  ```json
  {
    "proposal_id": "p-001",
    "type": "link | status | priority",

    // fields for type == "link":
    "source_id": "uuid",
    "source_type": "string",
    "link_type": "string",
    "target_id": "uuid",
    "target_type": "string",

    // fields for type == "status":
    "item_id": "uuid",
    "current_status": "string | null",
    "proposed_status": "string",

    // fields for type == "priority":
    "item_id": "uuid",
    "current_priority": "string | null",
    "proposed_priority": "string",

    // always present:
    "rationale": "string"
  }
  ```
  Fields not applicable to a proposal's type are omitted.

### OntologyReviewFailedEmptyRecord

- category: FAILURE
- emitted when: the project record contains no items at analysis time
- payload:
  - `failure_reason`: `string` — `"empty_project_record"`

### OntologyReviewFailedLLMUnavailable

- category: FAILURE
- emitted when: the AI service is unreachable or returns a response that cannot
  be parsed into proposals
- payload:
  - `failure_reason`: `string` — `"llm_unavailable"`
  - `error_detail`: `string` — description of the failure (connection error, parse error, etc.)

### OntologyConfirmRequested

- category: OBSERVATIONAL
- emitted when: the PM submits a confirm decision (separate invocation from analyse)
- payload:
  - `review_id`: `string` — the review being confirmed
  - `accepted_ids`: `array<string>` — proposal_ids the PM is accepting
  - `rejected_ids`: `array<string>` — proposal_ids the PM is explicitly rejecting

### OntologyReviewConfirmed

- category: BEHAVIORAL
- emitted when: the confirm phase completes (after all accepted proposals are applied)
- payload:
  - `review_id`: `string` — the review that was confirmed
  - `accepted_count`: `u32` — number of proposals applied
  - `rejected_count`: `u32` — number of proposals rejected
  - `skipped_count`: `u32` — number of accepted proposals that failed confirm-time validation
  - `accepted_ids`: `array<string>` — proposal_ids that were applied
  - `rejected_ids`: `array<string>` — proposal_ids that were rejected
  - `skipped_ids`: `array<string>` — proposal_ids accepted by PM but skipped due to validation failure at confirm time

### OntologyConfirmFailedReviewNotFound

- category: FAILURE
- emitted when: the review_id supplied to confirm does not correspond to any
  OntologyReviewProposed event in the project record
- payload:
  - `failure_reason`: `string` — `"review_not_found"`
  - `review_id`: `string` — the review_id that was not found

---

## Event Flow

```text
── ANALYSE PHASE ──────────────────────────────────────────────────────────────

OntologyReviewRequested           ← PM triggers analysis

  ↓ (record empty)
OntologyReviewFailedEmptyRecord

  ↓ (LLM unavailable or unparseable)
OntologyReviewFailedLLMUnavailable

  ↓ (success — zero or more proposals)
OntologyReviewProposed

── CONFIRM PHASE (separate invocation) ────────────────────────────────────────

OntologyConfirmRequested          ← PM submits accept/reject decisions

  ↓ (review_id not found)
OntologyConfirmFailedReviewNotFound

  ↓ (review found — for each accepted proposal, in order)
  ItemLinked            (source_module: item_links)   — if link proposal passes validation
  ItemStatusUpdated     (source_module: item_status)  — if status proposal applied
  ItemPriorityUpdated   (source_module: item_status)  — if priority proposal applied

OntologyReviewConfirmed           ← emitted after all accepted proposals processed
```

---

## Coverage Check

| Contract Scenario | Events | Status |
|---|---|---|
| HP1: Proposals generated | OntologyReviewRequested → OntologyReviewProposed | COVERED |
| HP2: Link proposal accepted | OntologyConfirmRequested → ItemLinked → OntologyReviewConfirmed | COVERED |
| HP3: Status proposal accepted | OntologyConfirmRequested → ItemStatusUpdated → OntologyReviewConfirmed | COVERED |
| HP4: Priority proposal accepted | OntologyConfirmRequested → ItemPriorityUpdated → OntologyReviewConfirmed | COVERED |
| HP5: All proposals rejected | OntologyConfirmRequested → OntologyReviewConfirmed (accepted_count=0) | COVERED |
| HP6: Partial acceptance | OntologyConfirmRequested → (delegated events) × N → OntologyReviewConfirmed | COVERED |
| HP7: Zero proposals | OntologyReviewRequested → OntologyReviewProposed (proposal_count=0) | COVERED |
| HP8: Confirm old review | OntologyConfirmRequested (with prior review_id) → OntologyReviewConfirmed | COVERED |
| FP1: EmptyProjectRecord | OntologyReviewRequested → OntologyReviewFailedEmptyRecord | COVERED |
| FP2: LLMUnavailable | OntologyReviewRequested → OntologyReviewFailedLLMUnavailable | COVERED |
| FP3: ReviewNotFound | OntologyConfirmRequested → OntologyConfirmFailedReviewNotFound | COVERED |

---

<!-- METADATA -->
status: APPROVED
feature_id: ontology_suggest
approved_by: human
approved_at: 2026-05-28
derived_from_intent: intents/ontology_suggest.md
derived_from_contract: contracts/ontology_suggest_contract.md

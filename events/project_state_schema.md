# Event Schema: project_state

<!--
PURPOSE OF THIS FILE:
Defines the complete event spine for this feature.
Once approved, implementation may ONLY emit events listed here.

DERIVED FROM:
- intents/project_state.md (actors, outcomes)
- contracts/project_state_contract.md (state transitions, failure modes)

CROSS-FEATURE NOTE:
session_id in this schema corresponds to the correlation_id produced by
pm_structuring's ExtractionConfirmed event. This is the observable link
between the two features — no additional coupling is required.
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
  "source_module": "project_state",
  "payload": {}
}
```

`correlation_id` is mandatory and must propagate through the entire execution chain.

## Event Definitions

### IncorporationRequested

- category: OBSERVATIONAL
- emitted when: PM initiates incorporation of a confirmed extraction session
- payload:
  - `session_id`: `string` — identifier of the extraction session being incorporated

### ItemsIncorporated

- category: BEHAVIORAL
- emitted when: all confirmed items from a session are successfully added to the record
- payload:
  - `session_id`: `string` — the incorporated session identifier
  - `incorporated_count`: `integer` — number of items added in this operation
  - `total_record_size`: `integer` — total items now in the project record

### IncorporationFailedDuplicate

- category: FAILURE
- emitted when: the session being incorporated is already present in the record
  (contract failure: SessionAlreadyIncorporated)
- payload:
  - `failure_reason`: `string` — always "session_already_incorporated"
  - `session_id`: `string` — the duplicate session identifier

### RecordQueried

- category: OBSERVATIONAL
- emitted when: PM requests the full project record
- payload: (none beyond base fields)

### RecordReturned

- category: BEHAVIORAL
- emitted when: the project record is successfully returned to the PM
- payload:
  - `items`: `array` — all recorded items, each containing:
    - `item_id`: `string (uuid-v4)` — unique item identifier (from originating extraction)
    - `item_type`: `string` — one of: task, milestone, risk, issue, stakeholder
    - `description`: `string` — item content
    - `uncertain`: `boolean` — whether item was flagged as uncertain at extraction time
    - `uncertainty_reason`: `string | null` — uncertainty explanation if uncertain
    - `session_id`: `string` — originating extraction session identifier
  - `total_count`: `integer` — total items in the record
  - `session_count`: `integer` — number of distinct sessions represented

### RecordQueryFailedEmpty

- category: FAILURE
- emitted when: PM queries the record but no extractions have been incorporated
  (contract failure: EmptyRecord)
- payload:
  - `failure_reason`: `string` — always "record_empty"

## Event Flow

```text
── Incorporate ──────────────────────────────────────
IncorporationRequested                  ← PM initiates incorporation
  ↓
  ├─ (session already in record)
  │    IncorporationFailedDuplicate
  │
  └─ (new session)
       ItemsIncorporated

── View ─────────────────────────────────────────────
RecordQueried                           ← PM requests the record
  ↓
  ├─ (record is empty)
  │    RecordQueryFailedEmpty
  │
  └─ (record has items)
       RecordReturned
```

## Coverage Check

| Contract Failure             | Event Here                    | Status  |
|---|---|---|
| EmptyRecord                  | RecordQueryFailedEmpty        | COVERED |
| SessionAlreadyIncorporated   | IncorporationFailedDuplicate  | COVERED |

---

<!-- METADATA -->
status: APPROVED
feature_id: project_state
approved_by:
approved_at:
derived_from_intent: intents/project_state.md
derived_from_contract: contracts/project_state_contract.md

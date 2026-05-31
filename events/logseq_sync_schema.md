# Event Schema: logseq_sync

<!--
DERIVED FROM:
- intents/logseq_sync.md
- contracts/logseq_sync_contract.md
-->

## Naming Convention

See `docs/conventions.md`.

## Required Base Fields (all events)

```json
{
  "event_id": "uuid-v4",
  "event_type": "EventName",
  "timestamp": 1710000000000,
  "correlation_id": "uuid-v4",
  "source_module": "logseq_sync",
  "payload": {}
}
```

`correlation_id` is mandatory and must propagate through the entire execution chain.

## Cross-Module Note

`ItemStatusUpdated` and `ItemPriorityUpdated` are emitted by this module
with `source_module: "logseq_sync"`. Consumers that read these event types
(item_status, logseq_export) must not filter by source_module — they must
accept these events from any source module that emits them. This is a
Stage 4 implementation constraint.

## Event Definitions

### SyncRequested

- category: OBSERVATIONAL
- emitted when: the PM triggers a sync run
- payload:
  - `graph_dir`: `string` — path to the designated Logseq graph directory

### ItemStatusUpdated

- category: BEHAVIORAL
- emitted when: a status difference is detected for a known project item
  and the updated status is valid for that item's type
- payload:
  - `item_id`: `string` — UUID of the item
  - `item_type`: `string` — type of the item (task, milestone, risk, issue, stakeholder)
  - `new_status`: `string` — status value read from Logseq
  - `previous_status`: `string | null` — effective status before this sync;
    null if no prior status was recorded

### ItemPriorityUpdated

- category: BEHAVIORAL
- emitted when: a priority difference is detected for a known project item
  and the updated priority is a valid value (high, medium, low)
- payload:
  - `item_id`: `string` — UUID of the item
  - `item_type`: `string` — type of the item
  - `new_priority`: `string` — priority value read from Logseq
  - `previous_priority`: `string | null` — effective priority before this
    sync; null if no prior priority was recorded

### ItemSyncSkippedInvalidStatus

- category: FAILURE
- emitted when: a Logseq page for a known item contains a status value
  that is not valid for that item's type; the item is skipped and the
  sync continues
- payload:
  - `failure_reason`: `string` — `"invalid_status_for_type"`
  - `item_id`: `string` — UUID of the item
  - `item_type`: `string` — type of the item
  - `rejected_status`: `string` — the invalid status value read from Logseq

### SyncCompleted

- category: BEHAVIORAL
- emitted when: sync ran to completion and at least one difference was
  detected (changes applied, items skipped, or both)
- payload:
  - `graph_dir`: `string` — path to the Logseq graph directory
  - `changes_applied`: `u32` — total count of status and priority updates recorded
  - `items_skipped`: `u32` — count of items skipped due to invalid values

### SyncCompletedNoChanges

- category: BEHAVIORAL
- emitted when: sync ran to completion and no differences were detected
  between the Logseq graph and the project record
- payload:
  - `graph_dir`: `string` — path to the Logseq graph directory
  - `items_checked`: `u32` — number of project record items compared

### SyncFailedGraphNotAccessible

- category: FAILURE
- emitted when: the Logseq graph directory is missing or cannot be read
- payload:
  - `failure_reason`: `string` — `"graph_not_accessible"`
  - `graph_dir`: `string` — path that was attempted

### SyncFailedEmptyRecord

- category: FAILURE
- emitted when: the project record contains no items at sync time
- payload:
  - `failure_reason`: `string` — `"empty_project_record"`

## Event Flow

```text
SyncRequested                           ← emitted on: PM triggers sync

  ↓ (graph directory missing or unreadable)
SyncFailedGraphNotAccessible            ← sync aborts

  ↓ (project record empty)
SyncFailedEmptyRecord                   ← sync aborts

  ↓ (per-item iteration — for each item in the project record that has a page)
  │
  ├─ (status differs and is invalid for the item's type)
  │   ItemSyncSkippedInvalidStatus
  │
  ├─ (status differs and is valid)
  │   ItemStatusUpdated
  │
  └─ (priority differs and is valid)
      ItemPriorityUpdated

  ↓ (at least one difference was detected across any item)
SyncCompleted

  ↓ (no differences detected across any item)
SyncCompletedNoChanges
```

Note: `ItemSyncSkippedInvalidStatus`, `ItemStatusUpdated`, and `ItemPriorityUpdated` are
emitted per-item as each item is processed, not grouped by event type. For a single item,
status is evaluated before priority.

## Coverage Check

| Contract Failure | Event Here | Status |
|---|---|---|
| GraphNotAccessible | SyncFailedGraphNotAccessible | COVERED |
| ProjectRecordEmpty | SyncFailedEmptyRecord | COVERED |
| InvalidStatusForType | ItemSyncSkippedInvalidStatus | COVERED |

---

<!-- METADATA -->
status: APPROVED
feature_id: logseq_sync
approved_by: human
approved_at: 2026-05-26
derived_from_intent: intents/logseq_sync.md
derived_from_contract: contracts/logseq_sync_contract.md

# Event Schema: priority_view

<!--
DERIVED FROM:
- intents/priority_view.md (actors, outcomes)
- contracts/priority_view_contract.md (state transitions, failure modes)
-->

## Naming Convention

See `docs/conventions.md` (source: `.codeos/templates/conventions.md`).

## Required Base Fields (all events)

```json
{
  "event_id": "uuid-v4",
  "event_type": "EventName",
  "timestamp": 1710000000000,
  "correlation_id": "uuid-v4",
  "source_module": "priority_view",
  "payload": {}
}
```

`correlation_id` is mandatory and must propagate through the entire execution chain.

## Status Rank Table

Defines the secondary sort order (lower rank = ranked first). Applied
when two items share the same priority level. Covers all valid status
values from the `item_status` feature.

| Rank | Statuses |
|---|---|
| 1 — Active | `doing`, `in_progress`, `active` |
| 2 — Initial | `todo`, `open`, `pending` |
| 3 — Deferred | `waiting` |
| 4 — Terminal | `done`, `achieved`, `resolved`, `mitigated`, `accepted`, `cancelled`, `missed`, `closed`, `inactive` |

Items with no status set rank after all ranked statuses, before items with
no priority set.

## Priority Rank Table

Defines the primary sort order (lower rank = ranked first).

| Rank | Priority |
|---|---|
| 1 | `high` |
| 2 | `medium` |
| 3 | `low` |
| 4 | (no priority set) |

## Event Definitions

### PriorityViewRequested

- category: OBSERVATIONAL
- emitted when: PM initiates a request for the priority-ranked item view
- payload:
  - `filter_type`: `string | null` — item type filter supplied by PM; null if not specified
  - `filter_status`: `string | null` — status filter supplied by PM; null if not specified
  - `filter_priority`: `string | null` — priority filter supplied by PM; null if not specified

### PriorityViewReturned

- category: BEHAVIORAL
- emitted when: the project record contains at least one item and all supplied
  filter values are valid (covers happy paths 1, 2, and 3 — including an
  empty filtered result)
- payload:
  - `item_count`: `integer` — number of items in the returned list (0 or more)
  - `filters_applied`: `object` — echo of the filters that were active:
    `{ "type": string|null, "status": string|null, "priority": string|null }`
  - `items`: `array` — ordered list of item summaries; each entry contains:
    - `item_id`: `string`
    - `item_type`: `string`
    - `description`: `string`
    - `priority`: `string | null` — explicit priority or null if none set
    - `status`: `string | null` — current status or null if none set
    - `session_id`: `string` — session from which the item originated

### PriorityViewFailedEmptyRecord

- category: FAILURE
- emitted when: the project record contains no items at all
  (contract failure: EmptyRecord)
- payload:
  - `failure_reason`: `string` — always `"empty_record"`

### PriorityViewFailedInvalidFilter

- category: FAILURE
- emitted when: one or more supplied filter values are not recognised as a
  valid item type, status value, or priority level
  (contract failure: InvalidFilter)
- payload:
  - `failure_reason`: `string` — always `"invalid_filter"`
  - `filter_field`: `string` — which filter was invalid (`"type"`, `"status"`, or `"priority"`)
  - `filter_value`: `string` — the unrecognised value that was supplied

## Event Flow

```text
PriorityViewRequested               ← PM requests priority view (with optional filters)
  ↓
  ├─ (record contains no items)
  │    PriorityViewFailedEmptyRecord
  │
  ├─ (a filter value is not recognised)
  │    PriorityViewFailedInvalidFilter
  │
  └─ (record has items, all filters valid)
       PriorityViewReturned             ← item_count may be 0 if filters match nothing
```

## Coverage Check

| Contract Failure | Event Here | Status |
|---|---|---|
| EmptyRecord | PriorityViewFailedEmptyRecord | COVERED |
| InvalidFilter | PriorityViewFailedInvalidFilter | COVERED |

---
status: APPROVED
feature_id: priority_view
approved_by: human
approved_at: 2026-05-26
derived_from_intent: intents/priority_view.md
derived_from_contract: contracts/priority_view_contract.md

# Event Schema: item_status

<!--
PURPOSE OF THIS FILE:
Defines the complete event spine for this feature.
Once approved, implementation may ONLY emit events listed here.

DERIVED FROM:
- intents/item_status.md (actors, outcomes)
- contracts/item_status_contract.md (state transitions, failure modes)

CROSS-FEATURE NOTE:
item_id in this schema corresponds to item_id values produced by pm_structuring
(ItemsExtracted) and incorporated via project_state (ItemsIncorporated).
No additional coupling is required beyond reading the shared event log.
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
  "source_module": "item_status",
  "payload": {}
}
```

`correlation_id` is mandatory and must propagate through the entire execution chain.

## Event Definitions

### StatusUpdateRequested

- category: OBSERVATIONAL
- emitted when: PM initiates a status change on a specific item
- payload:
  - `item_id`: `string` — identifier of the item being updated
  - `requested_status`: `string` — the status value the PM has requested

### ItemStatusUpdated

- category: BEHAVIORAL
- emitted when: the requested status is valid for the item's type and the
  item exists in the project record
- payload:
  - `item_id`: `string` — identifier of the updated item
  - `item_type`: `string` — type of the item (task, milestone, risk, issue, stakeholder)
  - `new_status`: `string` — the status value now in effect
  - `previous_status`: `string | null` — the status value before this update,
    null if this is the first status set for the item

### StatusUpdateFailedItemNotFound

- category: FAILURE
- emitted when: the item_id supplied to set-status does not exist in the
  project record (contract failure: ItemNotFound)
- payload:
  - `failure_reason`: `string` — always "item_not_found"
  - `item_id`: `string` — the item_id that was not found

### StatusUpdateFailedInvalidStatus

- category: FAILURE
- emitted when: the requested status value is not valid for the item's type
  (contract failure: InvalidStatusForType)
- payload:
  - `failure_reason`: `string` — always "invalid_status_for_type"
  - `item_id`: `string` — the item being updated
  - `item_type`: `string` — the item's type (determines valid status values)
  - `requested_status`: `string` — the invalid status value that was supplied

### PriorityUpdateRequested

- category: OBSERVATIONAL
- emitted when: PM initiates a priority change on a specific item
- payload:
  - `item_id`: `string` — identifier of the item being updated
  - `requested_priority`: `string` — the priority value the PM has requested

### ItemPriorityUpdated

- category: BEHAVIORAL
- emitted when: the requested priority is valid and the item exists in the
  project record
- payload:
  - `item_id`: `string` — identifier of the updated item
  - `new_priority`: `string` — the priority value now in effect (high, medium, low)
  - `previous_priority`: `string | null` — the priority value before this update,
    null if this is the first priority set for the item

### PriorityUpdateFailedItemNotFound

- category: FAILURE
- emitted when: the item_id supplied to set-priority does not exist in the
  project record (contract failure: ItemNotFound)
- payload:
  - `failure_reason`: `string` — always "item_not_found"
  - `item_id`: `string` — the item_id that was not found

### PriorityUpdateFailedInvalidValue

- category: FAILURE
- emitted when: the requested priority value is not one of: high, medium, low
  (contract failure: InvalidPriorityValue)
- payload:
  - `failure_reason`: `string` — always "invalid_priority_value"
  - `item_id`: `string` — the item being updated
  - `requested_priority`: `string` — the invalid priority value that was supplied

### ItemStatusQueried

- category: OBSERVATIONAL
- emitted when: PM requests the current status and priority of a specific item
- payload:
  - `item_id`: `string` — identifier of the item being queried

### ItemStatusReturned

- category: BEHAVIORAL
- emitted when: the queried item exists in the project record
- payload:
  - `item_id`: `string` — identifier of the queried item
  - `item_type`: `string` — type of the item
  - `current_status`: `string | null` — most recently recorded status, null if
    no status has ever been set
  - `current_priority`: `string | null` — most recently recorded priority, null
    if no priority has ever been set

### ItemStatusQueryFailedItemNotFound

- category: FAILURE
- emitted when: the item_id supplied to the get command does not exist in
  the project record (contract failure: ItemNotFound)
- payload:
  - `failure_reason`: `string` — always "item_not_found"
  - `item_id`: `string` — the item_id that was not found

## Event Flow

```text
── set-status ───────────────────────────────────────
StatusUpdateRequested               ← PM initiates status change
  ↓
  ├─ (item_id not in record)
  │    StatusUpdateFailedItemNotFound
  │
  ├─ (status value invalid for item_type)
  │    StatusUpdateFailedInvalidStatus
  │
  └─ (item exists, status valid)
       ItemStatusUpdated

── set-priority ─────────────────────────────────────
PriorityUpdateRequested             ← PM initiates priority change
  ↓
  ├─ (item_id not in record)
  │    PriorityUpdateFailedItemNotFound
  │
  ├─ (priority value not in {high, medium, low})
  │    PriorityUpdateFailedInvalidValue
  │
  └─ (item exists, priority valid)
       ItemPriorityUpdated

── get ──────────────────────────────────────────────
ItemStatusQueried                   ← PM requests current status/priority
  ↓
  ├─ (item_id not in record)
  │    ItemStatusQueryFailedItemNotFound
  │
  └─ (item exists)
       ItemStatusReturned
```

## Coverage Check

| Contract Failure     | Event(s) Here                                                                                         | Status  |
|---|---|---|
| ItemNotFound         | StatusUpdateFailedItemNotFound, PriorityUpdateFailedItemNotFound, ItemStatusQueryFailedItemNotFound   | COVERED |
| InvalidStatusForType | StatusUpdateFailedInvalidStatus                                                                       | COVERED |
| InvalidPriorityValue | PriorityUpdateFailedInvalidValue                                                                      | COVERED |

---

<!-- METADATA -->
status: APPROVED
feature_id: item_status
approved_by:
approved_at:
derived_from_intent: intents/item_status.md
derived_from_contract: contracts/item_status_contract.md

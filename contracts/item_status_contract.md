# Behavioral Contract: item_status

<!--
PURPOSE OF THIS FILE:
Defines observable truths derived from the approved intent.
Contracts describe OBSERVABLE behavior, not internal logic.
Every clause must be independently testable.
This file must be APPROVED before Stage 4 (implementation) begins.

DERIVED FROM: intents/item_status.md
-->

## Status Vocabularies

Valid status values by item type (any other value triggers InvalidStatusForType):

| Item Type   | Valid Status Values                              |
|---|---|
| task        | todo, doing, done, waiting, cancelled            |
| milestone   | pending, achieved, missed                        |
| risk        | open, mitigated, accepted, closed                |
| issue       | open, in_progress, resolved, closed              |
| stakeholder | active, inactive                                 |

Valid priority values (any other value triggers InvalidPriorityValue): high, medium, low

## Scenarios

### Happy Path 1: Set Item Status

```gherkin
Given an item with a known item_id exists in the project record
And the requested status value is valid for that item's type
When the PM sets a status on that item
Then the item's current status is updated to the new value
And no other item's status is affected
```

### Happy Path 2: Set Item Priority

```gherkin
Given an item with a known item_id exists in the project record
And the requested priority value is one of: high, medium, low
When the PM sets a priority on that item
Then the item's current priority is updated to the new value
And the item's status is unchanged
And no other item's priority is affected
```

### Happy Path 3: Query Item Status and Priority

```gherkin
Given an item with a known item_id exists in the project record
When the PM queries the status and priority of that item
Then the item's current status is returned (null if never set and no proposed value exists)
And the item's current priority is returned (null if never set and no proposed value exists)
And no state change occurs
```

### Happy Path 4: Query Returns Proposed Value as Fallback

```gherkin
Given an item exists in the project record
And a proposed_status or proposed_priority was recorded at extraction time
And no explicit set-status or set-priority command has been issued for that item
When the PM queries the status and priority of that item
Then the proposed_status is returned as the effective status (marked as proposed)
And the proposed_priority is returned as the effective priority (marked as proposed)
And no state change occurs
```

### Happy Path 5: Explicit Update Overrides Proposed Value

```gherkin
Given an item exists in the project record
And a proposed_status or proposed_priority was recorded at extraction time
When the PM explicitly sets a status or priority on that item
Then the explicitly set value becomes the effective status or priority
And the proposed value no longer takes effect for that field
```

### Failure Path 1: ItemNotFound

```gherkin
Given no item with the requested item_id exists in the project record
When the PM attempts to set status, set priority, or query that item_id
Then the PM is informed the item does not exist
And no status or priority is recorded
```

### Failure Path 2: InvalidStatusForType

```gherkin
Given an item exists in the project record with a known item_type
When the PM attempts to set a status value not valid for that item's type
Then the PM is informed the status is invalid for the item's type
And the item's current status is unchanged
```

### Failure Path 3: InvalidPriorityValue

```gherkin
Given an item exists in the project record
When the PM attempts to set a priority value not in {high, medium, low}
Then the PM is informed the priority value is invalid
And the item's current priority is unchanged
```

## Invariants

- Only status values valid for an item's type are ever recorded against that item
- A status or priority update on one item never alters the status or priority of
  any other item
- Priority and status are independently settable — setting one never changes the other
- The current status of an item is always the most recently recorded status for
  that item; no status update erases history
- An item must exist in the project record (incorporated via project_state) before
  any status or priority can be recorded against it
- If no explicit status update exists for an item, the effective status is the
  proposed_status from extraction (if one was given and the extraction was confirmed);
  otherwise null
- If no explicit priority update exists for an item, the effective priority is the
  proposed_priority from extraction (if one was given and the extraction was confirmed);
  otherwise null
- An explicit update always takes precedence over a proposed value

## Preconditions

- The project record contains at least one incorporated extraction session
  (item_ids are only valid once they appear in the project record)

## Postconditions

- After set status: the item's current status equals the newly set value; all
  other items are unchanged
- After set priority: the item's current priority equals the newly set value; the
  item's status and all other items are unchanged
- After query: no state change has occurred; the response reflects the item's
  most recently recorded status and priority, falling back to proposed values
  from extraction if no explicit update has been recorded

## Runtime Artifacts

| Artifact | Path | Lifecycle |
|---|---|---|
| (none beyond events/runtime_events.jsonl) | — | — |

Current status and priority are derived by replaying ItemStatusUpdated and
ItemPriorityUpdated events in the shared event log — no separate state file.

## Failure Classifications

| Failure Name         | Trigger Condition                               | Observable Signal                        |
|---|---|---|
| ItemNotFound         | item_id not present in project record           | PM informed; no status/priority recorded |
| InvalidStatusForType | status value not valid for the item's item_type | PM informed; item status unchanged       |
| InvalidPriorityValue | priority value not in {high, medium, low}       | PM informed; item priority unchanged     |

---

<!-- METADATA -->
status: APPROVED
feature_id: item_status
approved_by:
approved_at:
derived_from_intent: intents/item_status.md
derived_event_schema: events/item_status_schema.md

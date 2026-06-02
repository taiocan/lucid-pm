# Event Schema: item_status_schema_driven_vocabulary

DERIVED FROM:
- intents/item_status_schema_driven_vocabulary.md
- contracts/item_status_schema_driven_vocabulary_contract.md
AMENDS: events/item_status_schema.md

## Design note

This integration changes status vocabulary authority and adds marker-derived status
resolution at query time. Three existing events are amended; one new OBSERVATIONAL
event is introduced. Schema failures are owned by the `project_schema` module.

1. `ItemStatusUpdated` (existing) — `item_type` now accepts any vocabulary-defined
   entity type; valid status values are vocabulary-defined per type; payload structure
   unchanged.
2. `StatusUpdateFailedInvalidStatus` (existing) — trigger condition changes: the
   valid status set is now read from the active vocabulary, not the hardcoded table;
   payload structure unchanged.
3. `ItemStatusReturned` (existing) — additive payload amendment: `current_status`
   semantics broadened to "effective status" (may be explicit, marker-derived, or
   proposed); new `status_source` field added to indicate which resolution step
   produced the value.
4. `ItemStatusUnrecognized` (new OBSERVATIONAL) — emitted exactly once per `get`
   invocation when the effective status is a stale recorded value no longer present
   in the active vocabulary.

Schema failures (FP1: SchemaInvalid) are owned by the `project_schema` module and
emitted to the same event log. `item_status` emits no wrapper schema event.

## Replay note

Effective status resolution is deterministic with respect to (event log, item content,
active vocabulary). Replaying the same event log under a different vocabulary may
produce different effective statuses — different marker mappings, different recognized
status sets — while the underlying recorded status values (ItemStatusUpdated events)
remain unchanged. This is by design: vocabulary is runtime configuration, not recorded
state.

## Required Base Fields (all events)

```json
{
  "event_id":       "uuid-v4",
  "event_type":     "EventName",
  "timestamp":      1710000000000,
  "correlation_id": "uuid-v4",
  "source_module":  "item_status",
  "payload":        {}
}
```

`correlation_id` is mandatory and must propagate through the entire execution chain.

---

## Behavioral Amendments to Existing Events

### ItemStatusUpdated — behavioral amendment (no structural change)

The `item_type` field previously accepted only the hardcoded set
(`task`, `milestone`, `risk`, `issue`, `stakeholder`).
It now accepts any entity type defined in the active vocabulary.
The `new_status` field is now validated against the vocabulary's status set for the
item's type rather than the hardcoded status table.
Payload structure is unchanged.

Note: `ItemStatusUpdated` never captures or snapshots vocabulary state — it records
only the status value the PM supplied. Effective status resolution at query time is
always performed against the active vocabulary at that time, not the vocabulary that
was active when the event was written. Vocabulary versioning is not in scope.

### StatusUpdateFailedInvalidStatus — behavioral amendment (no structural change)

The trigger condition changes: `requested_status` is now validated against the
vocabulary-defined status set for the item's type, not the hardcoded table.
This failure also fires when the item's type has an empty status set in the
vocabulary (zero entries), making every possible status value invalid.
Payload structure is unchanged.

Note: `InvalidStatusForType` is the contract-level failure classification;
`StatusUpdateFailedInvalidStatus` is its event-level realization, established in the
base `events/item_status_schema.md`. This amendment changes the trigger condition
only — the event name, payload structure, and contract failure name are unchanged.

### ItemStatusReturned — payload amendment (additive)

`current_status` semantics are broadened: it now represents the **effective status**
— the value produced by the resolution chain (explicit → marker-derived → proposed →
null) — rather than strictly the most recently recorded explicit update.

One field added:

- `status_source`: `string | null` — indicates which resolution step produced the
  effective status. One of:
  - `"explicit"` — a recorded ItemStatusUpdated event is the source
  - `"marker_derived"` — a Logseq task marker mapping from the active vocabulary
  - `"proposed"` — the proposed_status value from extraction
  - `null` — no effective status (current_status is null)

Full amended payload:

- `item_id`: `string` — identifier of the queried item
- `item_type`: `string` — entity type of the item
- `current_status`: `string | null` — effective status value; null if none resolved
- `current_priority`: `string | null` — most recently recorded priority; null if none
- `status_source`: `string | null` — resolution step that produced current_status *(new)*

---

## New Event Definitions

### ItemStatusUnrecognized

- category: OBSERVATIONAL
- emitted when: a `get` invocation resolves the effective status to a recorded value
  (an explicit `ItemStatusUpdated` event exists) AND that value is not present in the
  active vocabulary's status set for the item's entity type; emitted exactly once per
  `get` invocation where this condition holds, before `ItemStatusReturned`; the
  recorded status value is still returned — this event is never a substitute for the
  result and does not abort the command
- note: this event will be re-emitted on every subsequent `get` for this item until
  the recorded status is updated or the vocabulary is changed to recognize the value;
  it is an observational signal, not a failure event
- payload:
  - `item_id`: `string` — identifier of the item whose status is stale
  - `item_type`: `string` — entity type of the item (determines vocabulary status set)
  - `recorded_status`: `string` — the stale status value as stored in the event log

---

## Cross-module events (from project_schema — emitted to same event log)

Schema failures are emitted by the `project_schema` module with
`source_module: "project_schema"`. The built-in default vocabulary ensures
SchemaNotFound cannot occur — only parse and validation failures are possible.

Event name verification: all events referenced in the flow below
(`StatusUpdateRequested`, `ItemStatusUpdated`, `StatusUpdateFailedItemNotFound`,
`StatusUpdateFailedInvalidStatus`, `PriorityUpdateRequested`, `ItemPriorityUpdated`,
`PriorityUpdateFailedItemNotFound`, `PriorityUpdateFailedInvalidValue`,
`ItemStatusQueried`, `ItemStatusReturned`, `ItemStatusQueryFailedItemNotFound`)
match the canonical names defined in `events/item_status_schema.md` exactly.

| Event | When emitted |
|---|---|
| `SchemaParseError` | Project schema file present but has a syntax error |
| `SchemaValidationFailed` | Project schema file parses but violates a structural rule |

When either fires, the item-status command does not complete and no item-status
operation event is emitted.

---

## Event Flow

```text
── set-status ───────────────────────────────────────────────────────────────
[set-status command]
  ↓
  Schema loading
  ├─ (schema parse or validation error)
  │    <SchemaParseError or SchemaValidationFailed from project_schema module>
  │    command exits — no item-status event emitted
  │
  └─ (schema loads successfully)
       ↓
       StatusUpdateRequested               ← PM initiates status change
       ↓
       ├─ (item_id not in project record)
       │    StatusUpdateFailedItemNotFound  ← existing; unchanged
       │
       ├─ (status value not in vocabulary status set for item's type,
       │    including when that set is empty)
       │    StatusUpdateFailedInvalidStatus ← existing; behavioral amendment
       │
       └─ (item exists, status in vocabulary)
            ItemStatusUpdated               ← existing; behavioral amendment

── set-priority ─────────────────────────────────────────────────────────────
[set-priority command]
  ↓
  Schema loading
  ├─ (schema parse or validation error)
  │    <SchemaParseError or SchemaValidationFailed from project_schema module>
  │    command exits — no item-status event emitted
  │
  └─ (schema loads successfully)
       ↓
       PriorityUpdateRequested             ← existing; unchanged
       ↓
       ├─ (item_id not in project record)
       │    PriorityUpdateFailedItemNotFound ← existing; unchanged
       │
       ├─ (priority value not in {high, medium, low})
       │    PriorityUpdateFailedInvalidValue ← existing; unchanged
       │
       └─ (item exists, priority valid)
            ItemPriorityUpdated            ← existing; unchanged

── get ──────────────────────────────────────────────────────────────────────
[get command]
  ↓
  Schema loading
  ├─ (schema parse or validation error)
  │    <SchemaParseError or SchemaValidationFailed from project_schema module>
  │    command exits — no item-status event emitted
  │
  └─ (schema loads successfully)
       ↓
       ItemStatusQueried                   ← existing; unchanged
       ↓
       ├─ (item_id not in project record)
       │    ItemStatusQueryFailedItemNotFound ← existing; unchanged
       │
       └─ (item exists)
            ↓
            Effective status resolution:
            1. explicit: most recent ItemStatusUpdated → status_source="explicit"
            2. marker-derived: task-type item, marker M in vocabulary mapping,
               no ItemStatusUpdated → status_source="marker_derived"
               (unmapped marker: falls through silently, no event)
            3. proposed: proposed_status from extraction → status_source="proposed"
            4. null → status_source=null
            ↓
            ├─ (resolution step 1 AND recorded value not in active vocabulary)
            │    ItemStatusUnrecognized     ← new OBSERVATIONAL; emitted once
            │    ↓
            │    ItemStatusReturned         ← existing; payload amendment
            │      current_status = stale recorded value
            │      status_source = "explicit"
            │
            └─ (all other cases)
                 ItemStatusReturned         ← existing; payload amendment
                   current_status = effective value (or null)
                   status_source  = "explicit"|"marker_derived"|"proposed"|null
```

## Validation Order (set-status operation)

Implementations must preserve this order to ensure deterministic failure classification:

1. Schema load → `<project_schema failure event>`; command exits
2. Item not found → `StatusUpdateFailedItemNotFound`
3. Status not in vocabulary status set for item's type → `StatusUpdateFailedInvalidStatus`
4. → `ItemStatusUpdated`

---

## Coverage Check

| Contract Scenario | Event(s) | Status |
|---|---|---|
| HP1: Schema-defined vocabulary governs set-status | `ItemStatusUpdated` (behavioral amendment; existing event) | COVERED — by design |
| HP2: Marker-derived effective status at query time | `ItemStatusReturned.status_source = "marker_derived"` (payload amendment) | COVERED — by design |
| HP3: Explicit status takes precedence over marker | `ItemStatusReturned.status_source = "explicit"` (payload amendment) | COVERED — by design |
| HP4: Unmapped marker falls through to proposed-value rule | `ItemStatusReturned.status_source = "proposed"` (no failure signal; payload amendment) | COVERED — by design |
| HP5: Stale recorded status produces non-failure observational signal | `ItemStatusUnrecognized` (new) + `ItemStatusReturned` (payload amendment) | COVERED |
| FP1: SchemaInvalid | `SchemaParseError` or `SchemaValidationFailed` from project_schema module | COVERED — cross-module |
| FP2: InvalidStatusForType | `StatusUpdateFailedInvalidStatus` (behavioral amendment; existing event) | COVERED — by design |

| Contract Failure | Event Here | Status |
|---|---|---|
| SchemaInvalid | `project_schema` module events (cross-module) | COVERED |
| InvalidStatusForType | `StatusUpdateFailedInvalidStatus` (existing; behavioral amendment) | COVERED |

---

status: APPROVED
feature_id: item_status_schema_driven_vocabulary
approved_by: human
approved_at: 2026-06-01
derived_from_intent: intents/item_status_schema_driven_vocabulary.md
derived_from_contract: contracts/item_status_schema_driven_vocabulary_contract.md
amends_event_schema: events/item_status_schema.md

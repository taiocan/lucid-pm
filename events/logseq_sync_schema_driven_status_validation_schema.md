# Event Schema: logseq_sync_schema_driven_status_validation

DERIVED FROM:
- intents/logseq_sync_schema_driven_status_validation.md
- contracts/logseq_sync_schema_driven_status_validation_contract.md
AMENDS: events/logseq_sync_schema.md

## Design Note

This refinement replaces the hardcoded status vocabulary with schema authority for
status validation during sync. Two changes to the event spine:

1. `ItemSyncSkippedInvalidStatus` (existing FAILURE) — behavioral amendment: trigger
   condition updated to reference the vocabulary-defined status set for the item's
   entity type concept. Payload structure and event name are unchanged.
2. `SyncFailedSchemaInvalid` (new FAILURE) — emitted when the project schema cannot
   be loaded and the sync must abort before reading any Logseq pages.

All other events from `events/logseq_sync_schema.md` are unchanged.

Schema load failures produce both a logseq_sync-level event (`SyncFailedSchemaInvalid`,
recording that the sync operation failed) and cross-module events from `project_schema`
(recording why schema loading failed). Both are emitted to the same event log.

**Architectural pattern:** Consumer modules emit business-outcome failure events
recording what operation failed (`SyncFailedSchemaInvalid`). The vocabulary owner
(`project_schema`) emits root-cause failure events recording why schema loading
failed (`SchemaNotFound`, `SchemaParseError`, `SchemaValidationFailed`). Both
coexist in the event log. Future features consuming `project_schema` should follow
this same pattern.

## Definitions

**Active vocabulary** — the vocabulary used by the sync operation to determine valid
status values for entity type concepts. When no project schema is supplied, the
embedded default vocabulary is active.

## Required Base Fields (all events)

```json
{
  "event_id":       "uuid-v4",
  "event_type":     "EventName",
  "timestamp":      1710000000000,
  "correlation_id": "uuid-v4",
  "source_module":  "logseq_sync",
  "payload":        {}
}
```

`correlation_id` is mandatory and must propagate through the entire execution chain.

---

## Behavioral Amendment to Existing Event

### ItemSyncSkippedInvalidStatus — behavioral amendment (no structural change)

The trigger condition changes: the status value from the Logseq page is now validated
against the **vocabulary-defined status set** for the item's entity type concept,
rather than a hardcoded status table. This amendment also fires when the entity type
concept has an empty vocabulary-defined status set (zero entries defined).

The entity type used for status validation is the vocabulary-resolved concept
associated with the stored representation. Validation outcome is independent of
whether the stored representation is a canonical name or alias.

Payload structure is unchanged:
- `failure_reason`: `string` — `"invalid_status_for_type"` (unchanged)
- `item_id`: `string` — UUID of the item (unchanged)
- `item_type`: `string` — entity type as stored in the project record (unchanged)
- `rejected_status`: `string` — the invalid status value read from Logseq (unchanged)

---

## New Event Definition

### SyncFailedSchemaInvalid

- category: FAILURE
- emitted when: the project schema cannot be loaded (file absent, unreadable, or
  structurally invalid); the sync aborts before reading any Logseq pages
- payload:
  - `failure_reason`: `string` — `"schema_load_failed"`

Note: the specific cause of the schema load failure (missing file, parse error,
structural violation) is carried by cross-module events from `project_schema`, not
by this event. This event records the sync business outcome only.

---

## Cross-module events relied upon (from project_schema)

These events are emitted by `project_schema` with `source_module: "project_schema"`.
They accompany `SyncFailedSchemaInvalid` but do not replace it. When no project
schema is supplied, none of these events fire and the embedded default vocabulary is
used silently.

| Event | Source module | When emitted |
|---|---|---|
| `SchemaNotFound` | project_schema | Project schema file absent or unreadable |
| `SchemaParseError` | project_schema | Project schema file present but has a syntax error |
| `SchemaValidationFailed` | project_schema | Project schema file parses but violates a structural rule |

---

## Event Flow Amendment

The amended flow adds one new branch to `events/logseq_sync_schema.md`:

```text
SyncRequested                                 ← emitted when PM triggers sync (unchanged)

  ├─ (SchemaLoadFailed)
  │   <SchemaNotFound | SchemaParseError | SchemaValidationFailed>  ← from project_schema
  │   SyncFailedSchemaInvalid                 ← new FAILURE; sync aborts
  │
  ├─ (GraphNotAccessible)                     ← unchanged
  │   SyncFailedGraphNotAccessible
  │
  ├─ (ProjectRecordEmpty)                     ← unchanged
  │   SyncFailedEmptyRecord
  │
  └─ (all preconditions satisfied)
       ↓
       per-item iteration:
       │
       ├─ (status differs; Logseq status not in vocabulary-defined set for entity type concept)
       │   ItemSyncSkippedInvalidStatus       ← behavioral amendment; payload unchanged
       │
       ├─ (status differs; status in vocabulary-defined set)
       │   ItemStatusUpdated                  ← unchanged
       │
       └─ (priority differs and is valid)
           ItemPriorityUpdated                ← unchanged
       ↓
       SyncCompleted | SyncCompletedNoChanges ← unchanged
```

The three abort branches are mutually exclusive failure conditions; no evaluation
order between them is prescribed.

---

## Coverage Check

| Contract Scenario | Event(s) | Status |
|---|---|---|
| HP1: Sync accepts custom-vocabulary status value | `ItemStatusUpdated` (existing; unchanged) | COVERED — by design |
| HP2: No project schema — behavior unchanged | All existing events; default vocabulary active silently | COVERED — by design |
| FP1: SchemaLoadFailed | `SyncFailedSchemaInvalid` (new) + cross-module project_schema events | COVERED |
| FP2: InvalidStatusForType (updated source of truth) | `ItemSyncSkippedInvalidStatus` (behavioral amendment; existing event) | COVERED — by design |
| FS1: Alias type — valid status accepted | `ItemStatusUpdated` (existing; unchanged) | COVERED — by design |
| FS2: Alias type — invalid status rejected | `ItemSyncSkippedInvalidStatus` (behavioral amendment; existing event) | COVERED — by design |

| Contract Failure | Event Here | Status |
|---|---|---|
| SchemaLoadFailed | `SyncFailedSchemaInvalid` (new FAILURE event) | COVERED |
| InvalidStatusForType | `ItemSyncSkippedInvalidStatus` (behavioral amendment; existing event) | COVERED |

---

status: APPROVED
feature_id: logseq_sync_schema_driven_status_validation
approved_by: human
approved_at: 2026-06-03
derived_from_intent: intents/logseq_sync_schema_driven_status_validation.md
derived_from_contract: contracts/logseq_sync_schema_driven_status_validation_contract.md
amends_event_schema: events/logseq_sync_schema.md

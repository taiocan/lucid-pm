# Event Schema: project_state_schema_integration

DERIVED FROM:
- intents/project_state_schema_integration.md
- contracts/project_state_schema_integration_contract.md
AMENDS: events/project_state_schema.md

## Design Note

R10 amends the view command only. Two changes to the event spine:

1. `RecordReturned` (existing BEHAVIORAL) — behavioral and semantic amendment:
   the `items` array now contains only recognized-type items; each item's
   `item_type` field shows the canonical type name rather than the stored
   representation; `total_count` now reflects the count of recognized items
   returned, not the total stored in the record. Payload structure is unchanged.
2. `RecordQueryFailedSchemaInvalid` (new FAILURE) — emitted when the schema
   cannot be loaded and the view command must abort.

Incorporation events (`IncorporationRequested`, `ItemsIncorporated`,
`IncorporationFailedDuplicate`) and the `RecordQueried` observational event are
unchanged.

**Architectural pattern (R9 precedent):** `RecordQueried` is always emitted
before schema failure is assessed — the view command was received and is
processing. `project_schema` then emits the root-cause event; `project_state`
emits `RecordQueryFailedSchemaInvalid` to record the business outcome.

**Entity type concept identity applied at read time:** The canonical type name
displayed in `RecordReturned` is resolved from the stored representation via the
vocabulary at view time. Items stored under aliases show the canonical name;
items with unrecognized types are excluded. The stored event log is never
modified.

**SchemaTypeUnknown ownership:** `project_schema` emits `SchemaTypeUnknown`
because it owns vocabulary resolution — the signal records the vocabulary fact
("this type resolves to no concept"), not the view decision. project_state
calls `emit_type_unknown` from the project_schema library at each
resolution site where the type is unrecognized; the library emits the event
with `source_module: "project_schema"`. This is consistent with R7.

## Definitions

**Active vocabulary** — the vocabulary used by the view command to resolve stored
entity types to concepts and canonical names. When no project schema is supplied,
the embedded default vocabulary is active.

## Required Base Fields (all events)

```json
{
  "event_id":       "uuid-v4",
  "event_type":     "EventName",
  "timestamp":      1710000000000,
  "correlation_id": "uuid-v4",
  "source_module":  "project_state",
  "payload":        {}
}
```

`correlation_id` is mandatory and must propagate through the entire execution chain.

---

## Behavioral Amendment to Existing Event

### RecordReturned — behavioral and semantic amendment

The `items` array and `total_count` field are amended as follows:

**`items` array:** Contains only items whose stored entity type resolves to a
recognized vocabulary concept. Items with unrecognized types are excluded.

**`item_type` field (per item):** Now displays the canonical type name as defined
in the active vocabulary for the item's entity type concept. If the item's type
was stored as an alias, the canonical name is displayed instead.

The entity type used for display is the vocabulary-resolved concept identity.
Display outcome is independent of whether the stored representation is a
canonical name or alias.

**`total_count` field:** Unchanged — still reflects the total number of items
in the project record (before exclusion). A consumer who receives
`items.length < total_count` knows exclusions occurred; the excluded count
is `total_count - items.length`, and is also derivable by counting
`SchemaTypeUnknown` events sharing the same `correlation_id`.

Keeping `total_count` at its pre-R10 semantics avoids a breaking semantic
change not required by the intent. The intent required only that unrecognized
items be absent from view output — not that the stored count be hidden.

All other payload fields are unchanged:
- `item_id`: `string` — unchanged
- `item_type`: `string` — **semantic amendment: now canonical name** (unchanged structure)
- `description`: `string` — unchanged
- `uncertain`: `boolean` — unchanged
- `uncertainty_reason`: `string | null` — unchanged
- `session_id`: `string` — unchanged
- `total_count`: `integer` — **unchanged: total items in the project record**
- `session_count`: `integer` — unchanged

---

## New Event Definition

### RecordQueryFailedSchemaInvalid

- category: FAILURE
- emitted when: the project schema cannot be loaded (file absent, unreadable, or
  structurally invalid); the view command aborts before returning any items
- payload:
  - `failure_reason`: `string` — `"schema_load_failed"`

Note: the specific cause of the schema load failure is carried by cross-module
events from `project_schema`, not by this event. This event records the view
command business outcome only.

---

## Cross-module events relied upon (from project_schema)

These events are emitted by `project_schema` with `source_module: "project_schema"`.

| Event | When emitted | Aborting? |
|---|---|---|
| `SchemaNotFound` | Schema file absent or unreadable | Yes — accompanies `RecordQueryFailedSchemaInvalid` |
| `SchemaParseError` | Schema file has a syntax error | Yes — accompanies `RecordQueryFailedSchemaInvalid` |
| `SchemaValidationFailed` | Schema file violates a structural rule | Yes — accompanies `RecordQueryFailedSchemaInvalid` |
| `SchemaTypeUnknown` | Item's entity type resolves to no vocabulary concept (one per excluded item) | No — view completes for recognized items |

`SchemaTypeUnknown` payload (defined in project_schema event schema):
- `item_id`: `string` — UUID of the excluded item
- `unknown_type`: `string` — the type value as stored in the event log

---

## Event Flow Amendment

The amended view flow adds a schema-failure branch and per-item exclusion loop:

```text
RecordQueried                              ← always emitted when PM requests the record

  ├─ (SchemaLoadFailed)
  │   <SchemaNotFound | SchemaParseError | SchemaValidationFailed> ← from project_schema
  │   RecordQueryFailedSchemaInvalid       ← new FAILURE; view aborts; no items returned
  │
  ├─ (EmptyRecord — schema loaded; no items in record before exclusion)
  │   RecordQueryFailedEmpty               ← unchanged
  │
  └─ (schema loaded; record has items)
       ↓
       <zero or more SchemaTypeUnknown>    ← from project_schema, per excluded item
       ↓
       RecordReturned                      ← behavioral/semantic amendment
         items = recognized-type items only, with canonical type names
         total_count = total items in record (unchanged; pre-exclusion)
```

The three abort branches are mutually exclusive failure conditions; no evaluation
order between SchemaLoadFailed and EmptyRecord is prescribed beyond the contract
requirement that schema is evaluated before any items are processed.

---

## Coverage Check

| Contract Scenario | Event(s) | Status |
|---|---|---|
| HP1: View includes only recognized items | `SchemaTypeUnknown` (project_schema, per excluded item) + `RecordReturned` (behavioral amendment) | COVERED |
| HP2: Alias items displayed under canonical type | `RecordReturned` — `item_type` shows canonical name (behavioral amendment) | COVERED — by design |
| HP3: No schema — no additional exclusions | `RecordReturned` with all items; no `SchemaTypeUnknown` for recognized items | COVERED — by design |
| HP4: All items excluded — empty result, not EmptyRecord | `SchemaTypeUnknown` (per item) + `RecordReturned` (`items=[]`, `total_count=0`) | COVERED |
| FP1: SchemaLoadFailed | `RecordQueryFailedSchemaInvalid` (new) + cross-module project_schema events | COVERED |

| Contract Failure | Event | Status |
|---|---|---|
| SchemaLoadFailed | `RecordQueryFailedSchemaInvalid` (new FAILURE event) | COVERED |

---

status: APPROVED
feature_id: project_state_schema_integration
approved_by: human
approved_at: 2026-06-03
derived_from_intent: intents/project_state_schema_integration.md
derived_from_contract: contracts/project_state_schema_integration_contract.md
amends_event_schema: events/project_state_schema.md

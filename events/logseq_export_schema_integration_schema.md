# Event Schema: logseq_export_schema_integration

DERIVED FROM:
- intents/logseq_export_schema_integration.md
- contracts/logseq_export_schema_integration_contract.md
AMENDS: events/logseq_export_schema.md

## Design note: zero new logseq_export events

This integration changes page rendering behavior and delegates schema-related
signals to the `project_schema` module. Two existing mechanisms cover all
contract scenarios:

1. `ExportCompleted` (existing) — carries the export outcome; gains one
   additive payload field (`items_excluded_type_unknown`)
2. `project_schema` module events (existing) — `SchemaTypeUnknown` covers
   HP4; schema FAILURE events cover FP1

No new `logseq_export` event types are introduced.

## Required Base Fields (all events)

```json
{
  "event_id":       "uuid-v4",
  "event_type":     "EventName",
  "timestamp":      1710000000000,
  "correlation_id": "uuid-v4",
  "source_module":  "logseq_export",
  "payload":        {}
}
```

## Event Amendments

### ExportCompleted — payload amendment (additive)

Existing event in `events/logseq_export_schema.md`. One field added:

- `items_excluded_type_unknown`: `u32` — count of items excluded because
  their entity type was not recognized by the active vocabulary schema.
  Present on every `ExportCompleted` event; value is 0 when all items
  were recognized.

Full amended payload:

- `output_dir`: `string` — path to the Logseq output directory written to
- `item_count`: `u32` — number of items successfully exported
- `pages_written`: `array<string>` — list of page file paths written
- `items_excluded_type_unknown`: `u32` — count of excluded unrecognized items *(new)*

All other fields and semantics of `ExportCompleted` are unchanged.

## Cross-module events (from project_schema — emitted to same event log)

These events are defined in `events/project_schema_schema.md` and emitted
with `source_module: "project_schema"`. They appear in the same
`events/runtime_events.jsonl` and form part of the observable chain for
this integration.

### SchemaTypeUnknown (project_schema module)

- emitted when: an item's entity type is not found in the active vocabulary
  schema during export item enumeration
- one event per excluded item, with the same `correlation_id` as the export run
- logseq_export calls `emit_type_unknown()` from the `project_schema` library

### Schema FAILURE events (project_schema module)

SchemaNotFound | SchemaParseError | SchemaValidationFailed |
SchemaAliasCollisionDetected — emitted when vocabulary loading fails.
When any of these fires, logseq_export does not emit ExportCompleted or
write any pages.

## Event Flow

```text
ExportRequested                  ← OBSERVATIONAL (existing)
  ↓
  Schema loading begins
  ↓
  ├─ (schema fails to load/validate)
  │    <SchemaFAILURE event from project_schema module>
  │    command exits — no pages written, no ExportCompleted
  │
  └─ (schema loads successfully)
       ↓
       for each item in project record:
       ├─ (entity type not in schema)
       │    SchemaTypeUnknown     ← project_schema module, same correlation_id
       │    item excluded, export continues
       │
       └─ (entity type recognized)
            page written with schema-driven labels + deadline
       ↓
       ExportCompleted            ← BEHAVIORAL (amended payload)
         items_excluded_type_unknown = N
```

## Coverage Check

| Contract Scenario | Event(s) | Status |
|---|---|---|
| HP1: Schema-driven labels on export | `ExportCompleted` (behavior change; no new event) | COVERED — by design |
| HP2: Deadline on every page | `ExportCompleted` (behavior change; no new event) | COVERED — by design |
| HP3: Renamed type exported under new name | `ExportCompleted` (behavior change; no new event) | COVERED — by design |
| HP4: Unrecognized type excluded with signal | `SchemaTypeUnknown` (project_schema) + `ExportCompleted.items_excluded_type_unknown` | COVERED |
| FP1: SchemaUnavailable — export aborts | Schema FAILURE event (project_schema) + no ExportCompleted | COVERED |

---

status: APPROVED
feature_id: logseq_export_schema_integration
approved_by: human
approved_at: 2026-05-31
derived_from_intent: intents/logseq_export_schema_integration.md
derived_from_contract: contracts/logseq_export_schema_integration_contract.md
amends_event_schema: events/logseq_export_schema.md

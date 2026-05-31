# Event Schema: project_schema

DERIVED FROM:
- intents/project_schema.md
- contracts/project_schema_contract.md

## Design note: zero behavioral events

This feature is a configuration layer. The schema drives the behavior of other
features' events; it does not produce behavioral events of its own. Only failure
conditions and unrecognized-type warnings are recorded in the event log.

## Required Base Fields (all events)

Every event includes:

```json
{
  "event_id":       "uuid-v4",
  "event_type":     "EventName",
  "timestamp":      1710000000000,
  "correlation_id": "uuid-v4",
  "source_module":  "project_schema",
  "payload":        {}
}
```

`correlation_id` is mandatory and must propagate through the execution chain.

## Event Definitions

### SchemaNotFound

- category: FAILURE
- emitted when: no project-level vocabulary definition is accessible AND no shared
  default vocabulary is accessible; command cannot proceed
- payload:
  - `failure_reason`: `string` — `"schema_not_found"`
  - `searched_locations`: `string[]` — abstract descriptions of locations checked
    (e.g., `["project vocabulary", "shared default"]`)

### SchemaParseError

- category: FAILURE
- emitted when: a vocabulary definition file is found but cannot be parsed due to a
  syntax error or missing required structural field
- payload:
  - `failure_reason`: `string` — `"schema_parse_error"`
  - `detail`: `string` — human-readable description of the parse failure location

### SchemaValidationFailed

- category: FAILURE
- emitted when: a vocabulary definition parses successfully but violates a structural
  rule (e.g., a `uses:` entry references an undefined property; a renderer mapping
  references an undefined relation)
- payload:
  - `failure_reason`: `string` — `"schema_validation_failed"`
  - `violated_rule`: `string` — identifies the specific rule that was violated
  - `detail`: `string` — human-readable description of the offending definition

### SchemaAliasCollisionDetected

- category: FAILURE
- emitted when: an alias value in the vocabulary definition matches another type's
  canonical name or another alias defined in the same vocabulary
- payload:
  - `failure_reason`: `string` — `"alias_collision"`
  - `alias_value`: `string` — the colliding alias value
  - `collides_with`: `string` — the canonical name or alias it collides with

### SchemaTypeUnknown

- category: OBSERVATIONAL
- emitted when: a vocabulary definition loads successfully AND the project record
  contains an item whose type does not match any defined type or alias; item is
  excluded from output; command continues
- payload:
  - `item_id`: `string` — UUID of the unrecognized item
  - `unknown_type`: `string` — the type value as recorded in the event log

## Event Flow

```text
[Any lucid command invoked]
  ↓
  Schema loading begins
  ↓
  ├─ (no vocabulary accessible)
  │    SchemaNotFound               ← FAILURE; command does not complete
  │
  ├─ (vocabulary has syntax or structural error)
  │    SchemaParseError             ← FAILURE; command does not complete
  │
  ├─ (vocabulary violates a structural rule)
  │    SchemaValidationFailed       ← FAILURE; command does not complete
  │
  ├─ (alias collides with another type)
  │    SchemaAliasCollisionDetected ← FAILURE; command does not complete
  │
  └─ (vocabulary loads successfully — no event)
       command proceeds with schema-driven behavior
         ↓
         for each item read from the project record:
         ├─ (item type not in vocabulary, no alias match)
         │    SchemaTypeUnknown     ← OBSERVATIONAL; item excluded; command continues
         │
         └─ (item type recognized — no event)
```

## Coverage Check

| Contract Scenario | Event | Status |
|---|---|---|
| Happy Path 1: Valid vocabulary loaded | No event (drives behavior of other features) | COVERED — by design |
| Happy Path 2: Project vocabulary takes precedence | No event (merge behavior; no new outcome) | COVERED — by design |
| Happy Path 3: No project vocab — default used | No event (fallback behavior; no new outcome) | COVERED — by design |
| Happy Path 4: Renamed type — data accessible | No event (alias resolution at read time) | COVERED — by design |
| Happy Path 5: Task marker mapping applied | No event (applied at query time by other features) | COVERED — by design |
| Failure Path 1: SchemaNotFound | `SchemaNotFound` | COVERED |
| Failure Path 2: SchemaParseError | `SchemaParseError` | COVERED |
| Failure Path 3: SchemaValidationError | `SchemaValidationFailed` | COVERED |
| Failure Path 4: AliasCollision | `SchemaAliasCollisionDetected` | COVERED |
| Non-aborting: SchemaTypeUnknownWarning | `SchemaTypeUnknown` | COVERED |

---

status: APPROVED
feature_id: project_schema
approved_by: human
approved_at: 2026-05-31
derived_from_intent: intents/project_schema.md
derived_from_contract: contracts/project_schema_contract.md

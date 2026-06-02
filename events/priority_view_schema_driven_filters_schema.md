# Event Schema: priority_view_schema_driven_filters

DERIVED FROM:
- intents/priority_view_schema_driven_filters.md
- contracts/priority_view_schema_driven_filters_contract.md
AMENDS: events/priority_view_schema.md

## Design note

R7 introduces no new priority_view events and no structural payload
changes. All changes are behavioral amendments to two existing events,
plus a cross-module schema loading gate (consistent with R4, R5, R6)
and reliance on SchemaTypeUnknown (already defined in the project_schema
event schema).

1. `PriorityViewRequested` (existing) — behavioral amendment: filter type
   and status values are now validated against the active vocabulary;
   payload structure unchanged.
2. `PriorityViewFailedInvalidFilter` (existing) — behavioral amendment:
   trigger condition changes from hardcoded validation to vocabulary-based
   validation for type and status filters; priority filter validation
   remains hardcoded; payload structure unchanged.
3. `PriorityViewReturned` (existing) — no structural change. The count of
   excluded items is observable through SchemaTypeUnknown events sharing
   the same correlation_id, which the project_schema contract guarantees.

Schema loading gate: Consistent with R4, R5, and R6, vocabulary loading
occurs before the module's first observational event (PriorityViewRequested).
If vocabulary loading fails, PriorityViewRequested is not emitted. This
does not change the semantics of PriorityViewRequested itself — it
remains the observational event signalling that the priority view command
has been accepted and is proceeding.

Item exclusion loop: For each project record item whose entity type is
not recognized by the active vocabulary, project_schema emits one
SchemaTypeUnknown event and the item is excluded from the result. This
per-item exclusion loop is an implementation detail; the observable
commitment is the SchemaTypeUnknown event per excluded item.

## Required Base Fields (all events)

```json
{
  "event_id":       "uuid-v4",
  "event_type":     "EventName",
  "timestamp":      1710000000000,
  "correlation_id": "uuid-v4",
  "source_module":  "priority_view",
  "payload":        {}
}
```

`correlation_id` is mandatory on every priority_view event and must
propagate through the execution chain.

---

## Behavioral Amendments to Existing Events

### PriorityViewRequested — behavioral amendment (no structural change)

The `filter_type` and `filter_status` filter values are now validated
against the active vocabulary rather than hardcoded sets:

- `filter_type`: accepted if and only if the value is recognized by the
  active vocabulary (matches a canonical type name or alias)
- `filter_status`: accepted if and only if the value appears in the
  union of all per-type status sets defined in the active vocabulary
- `filter_priority`: validation unchanged (hardcoded: high, medium, low)

Payload structure is unchanged.

### PriorityViewFailedInvalidFilter — behavioral amendment (no structural change)

The trigger condition changes for `filter_type` and `filter_status`:

- `filter_type` invalid: the supplied type value is not recognized by
  the active vocabulary (matches no canonical name or alias)
- `filter_status` invalid: the supplied status value does not appear in
  the union of all per-type status sets defined in the active vocabulary
- `filter_priority` invalid: unchanged — value is not in {high, medium, low}

Payload structure is unchanged.

### PriorityViewReturned — no structural change

Payload is unchanged. The count of items excluded due to unrecognized
entity type is derivable by counting SchemaTypeUnknown events in the
event log that share the same correlation_id as this invocation.

---

## Cross-module events (from project_schema — emitted to same event log)

Schema failures and per-item exclusions are owned by the project_schema
module. They appear in the shared event log under
`source_module: "project_schema"`.

| Event | When emitted | Aborting? |
|---|---|---|
| `SchemaParseError` | Vocabulary file has a syntax error | Yes — command does not proceed |
| `SchemaValidationFailed` | Vocabulary file violates a structural rule | Yes — command does not proceed |
| `SchemaTypeUnknown` | A project record item's entity type is not recognized (one event per excluded item) | No — command continues |

`SchemaTypeUnknown` payload (defined in project_schema event schema):
- `item_id`: `string` — UUID of the excluded item
- `unknown_type`: `string` — the type value as stored in the event log

---

## Event Flow

```text
[priority_view command]
  ↓
  Vocabulary loading
  ├─ (parse or structural validation error)
  │    <SchemaParseError or SchemaValidationFailed — project_schema>
  │    command exits — PriorityViewRequested not emitted
  │
  └─ (vocabulary loads successfully)
       ↓
       PriorityViewRequested            ← filter values accepted (vocabulary loaded)
       ↓
       ├─ (project record is empty — before any exclusion)
       │    PriorityViewFailedEmptyRecord  ← existing; unchanged
       │
       ├─ (a filter value is invalid per active vocabulary or hardcoded rules)
       │    PriorityViewFailedInvalidFilter ← existing; behavioral amendment
       │
       └─ (record has items, all filters valid)
            ↓
            <zero or more SchemaTypeUnknown — project_schema, per excluded item>
            ↓
            PriorityViewReturned         ← existing; no structural change
              item_count = count of items in result (excludes excluded items)
```

---

## Coverage Check

| Contract Scenario | Event(s) | Status |
|---|---|---|
| HP1: Custom vocabulary type filter succeeds | `PriorityViewRequested` (amend) + `PriorityViewReturned` | COVERED — by design |
| HP2: Alias filter matching is bidirectional | `PriorityViewRequested` (amend) + `PriorityViewReturned` | COVERED — by design |
| HP3: Unrecognized items excluded; command completes | `SchemaTypeUnknown` (project_schema, per excluded item) + `PriorityViewReturned` | COVERED |
| HP4: All items excluded; empty result | `SchemaTypeUnknown` (per item) + `PriorityViewReturned` (item_count=0) | COVERED |
| HP5: Status globally valid but inapplicable to filtered type | `PriorityViewRequested` (amend) + `PriorityViewReturned` (item_count=0) | COVERED — by design |
| FP1: SchemaInvalid | `SchemaParseError` or `SchemaValidationFailed` — project_schema; no priority_view event | COVERED — cross-module |
| FP2: InvalidFilter (type) | `PriorityViewFailedInvalidFilter` (behavioral amendment) | COVERED |
| FP3: InvalidFilter (status) | `PriorityViewFailedInvalidFilter` (behavioral amendment) | COVERED |

| Contract Failure | Event | Status |
|---|---|---|
| SchemaInvalid | project_schema module events (cross-module) | COVERED |
| InvalidFilter (type) | `PriorityViewFailedInvalidFilter` (existing; behavioral amendment) | COVERED |
| InvalidFilter (status) | `PriorityViewFailedInvalidFilter` (existing; behavioral amendment) | COVERED |

---

status: DRAFT
feature_id: priority_view_schema_driven_filters
approved_by:
approved_at:
derived_from_intent: intents/priority_view_schema_driven_filters.md
derived_from_contract: contracts/priority_view_schema_driven_filters_contract.md
amends_event_schema: events/priority_view_schema.md

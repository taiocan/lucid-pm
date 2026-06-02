# Event Schema: item_links_schema_integration

DERIVED FROM:
- intents/item_links_schema_integration.md
- contracts/item_links_schema_integration_contract.md
AMENDS: events/item_links_schema.md

## Design note

This integration changes validation and label-derivation behavior. Three existing
mechanisms cover the happy-path scenarios; one new OBSERVATIONAL event and two new
FAILURE events are introduced.

1. `ItemLinked` (existing) — records a successful link; set of valid relation types
   is now vocabulary-defined, structure unchanged.
2. `LinkListReturned` (existing) — `display_label` field now comes from the
   vocabulary; `links_excluded_relation_unknown` count field added (see amendment
   below).
3. `LinkRelationTypeUnknown` (new OBSERVATIONAL) — emitted per excluded link during
   list operations when a link's relation type is not in the active vocabulary.

Schema failures (SchemaInvalid / FP1) are owned by the `project_schema` module and
emitted to the same event log. `item_links` emits no wrapper schema event.

## Replay note

Query outputs are deterministic with respect to the combination of (event log,
active vocabulary). Replaying the same event log under a different vocabulary may
produce different rendered outputs — different labels, different included/excluded
links — while the underlying project record (the sequence of ItemLinked and
ItemUnlinked events) remains unchanged. This is by design: vocabulary is runtime
configuration, not recorded state.

## Required Base Fields (all events)

```json
{
  "event_id":       "uuid-v4",
  "event_type":     "EventName",
  "timestamp":      1710000000000,
  "correlation_id": "uuid-v4",
  "source_module":  "item_links",
  "payload":        {}
}
```

`correlation_id` is mandatory and must propagate through the entire execution chain.

## Behavioral Amendments to Existing Events

### ItemLinked — behavioral amendment (no structural change)

The `link_type` field was previously constrained to the hardcoded set
(`blocks`, `affects`, `assigned_to`, `mitigated_by`, `escalates_to`, `related_to`).
It now accepts any relation type defined in the active vocabulary. Payload
structure is unchanged.

### LinkListReturned — payload amendment (additive)

The `display_label` field in each link entry now reflects the label defined for
that relation type in the active vocabulary at the time of the command, rather
than a hardcoded string. Links whose relation type is not in the active vocabulary
are excluded from the `links` array.

One field added:

- `links_excluded_relation_unknown`: `u32` — count of links excluded because their
  relation type was not recognized by the active vocabulary. Present on every
  `LinkListReturned` event; value is 0 when all links had recognized types.

Full amended payload:

- `item_id`: `string | null` — the item filter applied; null for all-links listing
- `link_count`: `integer` — total number of link entries returned (recognized only)
- `links`: `array` — the recognized link entries (structure unchanged)
- `links_excluded_relation_unknown`: `u32` — count of excluded unrecognized links *(new)*

## New Event Definitions

### LinkRelationTypeUnknown

- category: OBSERVATIONAL
- emitted when: a link list operation encounters a link whose relation type is not
  defined in the active vocabulary; one event per excluded link, emitted before
  `LinkListReturned`; the link remains in the project record
- note: this is an observational runtime event emitted during query execution and
  is not interpreted as a state transition; it will be re-emitted on every
  subsequent list command until the vocabulary is updated or the link is removed;
  one event per excluded link is emitted by design to preserve item-level
  traceability — the aggregate count in `LinkListReturned.links_excluded_relation_unknown`
  communicates scope while per-link events enable targeted remediation
- payload:
  - `source_id`: `string` — item_id of the source item
  - `link_type`: `string` — the unrecognized relation type value as stored in the
    event log
  - `target_id`: `string` — item_id of the target item

### LinkFailedRelationTypeUnrecognized

- category: FAILURE
- emitted when: the vocabulary loads successfully AND the PM supplies a relation
  type that is not defined in the active vocabulary during a link add operation
  (contract failure: InvalidRelationType)
- payload:
  - `failure_reason`: `string` — always `"relation_type_unrecognized"`
  - `relation_type`: `string` — the relation type value that was supplied

### LinkFailedItemTypeUnrecognized

- category: FAILURE
- emitted when: the vocabulary loads successfully AND the source or target item
  has an entity type that is not recognized by the active vocabulary; applies
  to link add operations only — link removal is not subject to entity type
  recognition
  (contract failure: ItemTypeUnrecognized)
- payload:
  - `failure_reason`: `string` — always `"item_type_unrecognized"`
  - `item_id`: `string` — the item whose entity type is not recognized
  - `item_type`: `string` — the unrecognized entity type value as stored in the
    event log
  - `role`: `string` — `"source"` or `"target"`

## Deprecation Note

`LinkFailedRelationTypeUnrecognized` supersedes `LinkFailedInvalidLinkType` for
the add operation. `LinkFailedInvalidLinkType` is **deprecated for new writes** —
it will not be emitted by implementations conforming to this schema. It is
**retained for historical replay** — logs predating this integration contain it
and replay tooling must continue to recognize it. The type-matrix-violation case
it previously covered is removed (relation source/target metadata is informational
only); the "unknown relation type" case is now covered by
`LinkFailedRelationTypeUnrecognized`.

## Cross-module events (from project_schema — emitted to same event log)

Schema failures are emitted by the `project_schema` module with
`source_module: "project_schema"`. The built-in default vocabulary ensures
SchemaNotFound cannot occur — only parse and validation failures are possible.

| Event | When emitted |
|---|---|
| `SchemaParseError` | Project schema file present but has a syntax error |
| `SchemaValidationFailed` | Project schema file parses but violates a structural rule |

When either fires, the item-link command does not complete and no link operation
event is emitted.

## Event Flow

```text
── add ──────────────────────────────────────────────────────────────────────
[any link add command]
  ↓
  Schema loading
  ├─ (schema parse or validation error)
  │    <SchemaParseError or SchemaValidationFailed from project_schema module>
  │    command exits — no link operation event emitted
  │
  └─ (schema loads successfully)
       ↓
       ├─ (source_id not in project record)
       │    LinkFailedItemNotFound          ← existing; operation="add"
       │
       ├─ (target_id not in project record)
       │    LinkFailedItemNotFound          ← existing; operation="add"
       │
       ├─ (source item's entity type not in vocabulary)
       │    LinkFailedItemTypeUnrecognized    [role="source"]
       │
       ├─ (target item's entity type not in vocabulary)
       │    LinkFailedItemTypeUnrecognized    [role="target"]
       │
       ├─ (relation type not defined in vocabulary)
       │    LinkFailedRelationTypeUnrecognized
       │
       ├─ (identical link already exists)
       │    LinkFailedDuplicateLink          ← existing
       │
       └─ (all checks pass)
            ItemLinked                       ← existing; behavioral amendment

── remove ───────────────────────────────────────────────────────────────────
[any link remove command]
  ↓
  Schema loading
  ├─ (schema parse or validation error)
  │    <SchemaParseError or SchemaValidationFailed from project_schema module>
  │    command exits — no link operation event emitted
  │
  └─ (schema loads successfully)
       ↓
       ├─ (source_id not in project record)
       │    LinkFailedItemNotFound          ← existing; operation="remove"
       │
       ├─ (target_id not in project record)
       │    LinkFailedItemNotFound          ← existing; operation="remove"
       │
       ├─ (link does not exist)
       │    LinkFailedLinkNotFound          ← existing
       │
       └─ (link exists — no entity type or relation type check on remove)
            ItemUnlinked                     ← existing; no amendment

── list ─────────────────────────────────────────────────────────────────────
[any link list command]
  ↓
  Schema loading
  ├─ (schema parse or validation error)
  │    <SchemaParseError or SchemaValidationFailed from project_schema module>
  │    command exits
  │
  └─ (schema loads successfully)
       ↓
       for each link in project record:
       ├─ (relation type not in active vocabulary)
       │    LinkRelationTypeUnknown          ← OBSERVATIONAL; one per excluded link
       │
       └─ (relation type recognized)
            link included with vocabulary labels in LinkListReturned.links
       ↓
       LinkListReturned                       ← existing; payload amendment
         links = recognized only; display_label from vocabulary
         links_excluded_relation_unknown = N
```

## Validation Order (add operation)

The following is the canonical precedence rule for add-operation validation.
Implementations must preserve this order to ensure deterministic failure classification.

1. Schema load → `<project_schema failure event>`; command exits
2. Source item not found → `LinkFailedItemNotFound`
3. Target item not found → `LinkFailedItemNotFound`
4. Source entity type unrecognized → `LinkFailedItemTypeUnrecognized` [role="source"]
5. Target entity type unrecognized → `LinkFailedItemTypeUnrecognized` [role="target"]
6. Relation type not in vocabulary → `LinkFailedRelationTypeUnrecognized`
7. Duplicate link → `LinkFailedDuplicateLink`
8. → `ItemLinked`

When both an item's entity type and the relation type are unrecognized (steps 4–6),
`LinkFailedItemTypeUnrecognized` is emitted for the first unrecognized item
encountered and the command exits; `LinkFailedRelationTypeUnrecognized` is never
reached in the same invocation. This is the accepted canonical behavior.

## Coverage Check

| Contract Scenario | Event(s) | Status |
|---|---|---|
| HP1: Vocabulary-defined relation type used to link items | `ItemLinked` (behavioral amendment; existing event) | COVERED — by design |
| HP2: Updated vocabulary label reflected in output | `LinkListReturned.display_label` (behavioral amendment; existing field) | COVERED — by design |
| HP3: Links with unrecognized relation type produce observable signal | `LinkRelationTypeUnknown` per excluded link + `LinkListReturned.links_excluded_relation_unknown` | COVERED |
| FP1: SchemaInvalid | `SchemaParseError` or `SchemaValidationFailed` from project_schema module | COVERED — cross-module |
| FP2: InvalidRelationType | `LinkFailedRelationTypeUnrecognized` | COVERED |
| FP3: ItemTypeUnrecognized | `LinkFailedItemTypeUnrecognized` | COVERED |

| Contract Failure | Event Here | Status |
|---|---|---|
| SchemaInvalid | `project_schema` module events (cross-module) | COVERED |
| InvalidRelationType | `LinkFailedRelationTypeUnrecognized` | COVERED |
| ItemTypeUnrecognized | `LinkFailedItemTypeUnrecognized` | COVERED |

---

status: APPROVED
feature_id: item_links_schema_integration
approved_by: human
approved_at: 2026-05-31
derived_from_intent: intents/item_links_schema_integration.md
derived_from_contract: contracts/item_links_schema_integration_contract.md
amends_event_schema: events/item_links_schema.md

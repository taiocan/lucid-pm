# Behavioral Contract: logseq_sync_schema_driven_status_validation

DERIVED FROM: intents/logseq_sync_schema_driven_status_validation.md
AMENDS: contracts/logseq_sync_contract.md — replaces the hardcoded status vocabulary
with schema authority for InvalidStatusForType validation; adds a SchemaLoadFailed
failure path. All other scenarios from the base contract remain in force unchanged.

## Definitions

**Active vocabulary** — the vocabulary used by the sync operation to determine valid
status values for entity type concepts. When no project schema is supplied, the
embedded default vocabulary is active.

**Vocabulary-defined status set** — the set of valid status values defined in the
active vocabulary for a given entity type concept. An entity type concept with no
status entries defined has an empty vocabulary-defined status set.

## Scenarios

### Happy Path 1: Sync accepts a custom-vocabulary status value

```gherkin
Given the active vocabulary is loaded successfully
And the vocabulary defines status value S for entity type concept T
And the project record contains an item whose entity type resolves to concept T
And the item has a corresponding Logseq page showing status S
When the PM triggers a sync
Then the item's status is updated to S
And no skip event is emitted for this item
And the update is recorded in the project event log
```

### Happy Path 2: No project schema — behavior unchanged

```gherkin
Given no project schema file is supplied
And the project record and Logseq graph are in a configuration that produced a
  successful sync before R9
When the PM triggers a sync
Then the sync produces an outcome indistinguishable from the pre-R9 result
```

### Failure Path 1: SchemaLoadFailed

```gherkin
Given the project schema file cannot be loaded (absent, unreadable, or
  structurally invalid)
When the PM triggers a sync
Then a SyncRequested event is emitted
And a SyncFailedSchemaInvalid event is emitted
And no Logseq pages are read
And no changes are made to the project record
```

### Failure Path 2: InvalidStatusForType (updated source of truth)

```gherkin
Given the active vocabulary is loaded successfully
And the vocabulary's status set for the item's entity type concept does not
  include the status value shown on the item's Logseq page
When the PM triggers a sync
Then an ItemSyncSkippedInvalidStatus event is emitted for that item
And that item's status is not updated in the project record
And the sync continues processing all other items
```

Note: this applies equally when the entity type concept's vocabulary-defined status
set is empty — any Logseq status for an item of that type triggers the skip.

### Falsification Scenario 1: Entity type stored as alias — valid status accepted

```gherkin
Given the active vocabulary is loaded successfully
And a project item's entity type is stored as alias A, where A resolves to concept T
And the vocabulary defines status value S for concept T
And the item's Logseq page shows status S
When the PM triggers a sync
Then the item's status is updated to S
And no skip event is emitted for this item
```

Falsifies: an implementation that compares the stored alias A directly against a
status table keyed on canonical T would fail to find the entry, reject S, and emit
a spurious skip event.

### Falsification Scenario 2: Entity type stored as alias — invalid status rejected

```gherkin
Given the active vocabulary is loaded successfully
And a project item's entity type is stored as alias A, where A resolves to concept T
And the vocabulary defines only status value S for concept T
And the item's Logseq page shows status X where X ≠ S
When the PM triggers a sync
Then an ItemSyncSkippedInvalidStatus event is emitted for that item
```

Falsifies: an implementation that only resolves aliases on the acceptance path but
uses the stored representation directly on the rejection path — producing either
incorrect acceptance of X or rejection for the wrong reason.

## Invariants

- **Concept Dependency Invariant:** Status validation depends only on
  vocabulary-defined concepts and their associated status sets. Entity type
  representations (canonical names, aliases, casing conventions, or specific type
  names) are not used as decision criteria.
- Status validation during sync uses only the vocabulary-defined status set for the
  item's entity type concept — no hardcoded status table is ever consulted
- The valid status set for any entity type is determined entirely by the vocabulary
  associated with that type's concept — no entity type name is treated as a special
  case for status set selection
- An item whose entity type is stored as an alias is validated against the same
  vocabulary-defined status set as an item whose entity type is stored as the
  corresponding canonical form
- When no project schema is supplied, sync behavior is identical to pre-R9
  behavior — no existing project is affected
- A schema load failure always produces SyncRequested followed by
  SyncFailedSchemaInvalid; no Logseq pages are read and no project record changes
  are made
- The condition under which an item is skipped via ItemSyncSkippedInvalidStatus is
  unchanged — only the source of truth for the valid status set changes; the event
  name and payload structure are unchanged
- All invariants from the base logseq_sync contract remain in force

## Vocabulary Dependency

**Vocabulary owner:** project_schema module
**Concepts operated on:** entity type concept identity (for status-set lookup);
vocabulary-defined valid status values per entity type concept
**Concept Dependency Invariant:** Business logic depends on vocabulary-defined
concepts and their associated status sets — not on entity type representations.
**Representation Ban invariant** *(derived from Concept Dependency):* Because
business logic depends only on concepts, entity type representations — aliases,
canonical strings, casing conventions, specific type names — must not appear in
domain decision logic.

## Invariant Falsification Scenarios

| Invariant | Falsifying fixture | Observable when correct | Wrong implementation assumption | Test ID |
|---|---|---|---|---|
| Vocabulary-defined set only; no hardcoded table | Vocabulary defines custom status "reviewing" for entity type concept T; item of type T; Logseq page shows "reviewing" | No skip event; status updated to "reviewing" | Hardcoded status table consulted; "reviewing" absent → incorrectly rejected | `test_vocabulary_defined_set_falsifies_hardcoded_table` |
| No entity type name is a hardcode special case | Vocabulary defines type "Inspector" with statuses ["scheduled", "done"]; item of type "Inspector"; Logseq page shows "scheduled" | Status updated; no skip event | Status lookup branches on known type names; "Inspector" hits no branch → empty/default set → "scheduled" rejected | `test_unknown_type_name_falsifies_hardcoded_type_branching` |
| Alias resolves to same status set as canonical (acceptance) | Vocabulary: canonical "Risk", alias "risk"; status "identified" for "Risk" concept; item stored with type "risk"; Logseq page shows "identified" | Status updated; no skip event | Stored type "risk" compared as string against "Risk"-keyed status table → mismatch → "identified" incorrectly rejected | `test_alias_acceptance_falsifies_string_comparison` |
| Alias resolves consistently on rejection path | Vocabulary: canonical "Risk", alias "risk"; status "identified" only; item stored with type "risk"; Logseq page shows "closed" (absent from set) | ItemSyncSkippedInvalidStatus emitted | Alias resolution applied on acceptance path only; stored "risk" used directly on rejection path → incorrect outcome | `test_alias_rejection_falsifies_acceptance_only_resolution` |
| Representation Ban (Concept Dependency Invariant) | Vocabulary: canonical "Risk", alias "risk"; item type stored as "risk"; Logseq page shows valid "Risk" status | Status updated correctly | Domain layer compares "risk" (stored) as string against "Risk" (canonical) → not equal → wrong status set → incorrect skip | `test_representation_ban_falsifies_direct_string_comparison` |
| Default vocabulary preserves pre-R9 behavior | No project schema supplied; item of default entity type with a status value valid under the previous implementation; Logseq page shows that status | Sync outcome identical to pre-R9; no additional skip events | Default vocabulary differs from previous hardcoded table → previously valid status rejected | `test_default_vocabulary_preserves_pre_r9_behavior` |

## Preconditions

- All preconditions from the base logseq_sync contract apply
- If a project schema file is present, it is evaluated before any Logseq pages are
  read or any project record changes are made

## Postconditions

- After successful sync: every item whose status was updated carries a status value
  present in the vocabulary-defined status set for its entity type concept
- Items skipped via InvalidStatusForType: their recorded status is unchanged; they
  appear in the SyncCompleted payload as before
- On schema load failure: SyncRequested and SyncFailedSchemaInvalid have been
  emitted; no Logseq pages were read; project record is unchanged

## Runtime Artifacts

| Artifact | Path | Lifecycle |
|---|---|---|
| None beyond events/runtime_events.jsonl | — | — |

### Cross-module signals relied upon

| Event | Source module | When relied upon |
|---|---|---|
| SchemaNotFound / SchemaInvalid | project_schema | Emitted by project_schema when the schema file is absent or structurally invalid; these are distinct from the sync-level SyncFailedSchemaInvalid event |

## Failure Classifications

| Failure Name | Trigger Condition | Observable Signal |
|---|---|---|
| SchemaLoadFailed | Project schema cannot be loaded (absent, unreadable, or structurally invalid) | SyncRequested emitted, then SyncFailedSchemaInvalid emitted; sync aborts; no Logseq pages read; project record unchanged |
| InvalidStatusForType | Logseq page carries a status not in the vocabulary-defined status set for the item's entity type concept (includes empty set) | ItemSyncSkippedInvalidStatus emitted per affected item; sync continues for remaining items; unchanged from base contract |

---

Note: InvalidStatusForType has identical semantics to the base logseq_sync contract —
event name, payload structure, and skip-and-continue behavior are all unchanged.
This amendment updates only the source of truth (vocabulary-defined set replaces
hardcoded table).

Note: project_schema emits SchemaNotFound or SchemaInvalid when it fails to load the
schema. logseq_sync additionally emits SyncFailedSchemaInvalid to record the sync
business outcome. Both facts are recorded: schema load failed (project_schema event)
and sync aborted because of it (logseq_sync event).

---
status: APPROVED
feature_id: logseq_sync_schema_driven_status_validation
approved_by: human
approved_at: 2026-06-03
derived_from_intent: intents/logseq_sync_schema_driven_status_validation.md
amends_contract: contracts/logseq_sync_contract.md
derived_event_schema: events/logseq_sync_schema_driven_status_validation_schema.md

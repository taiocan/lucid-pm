# Behavioral Contract: pm_structuring_schema_driven_entity_types

DERIVED FROM: intents/pm_structuring_schema_driven_entity_types.md
AMENDS: contracts/pm_structuring_contract.md — replaces the hardcoded entity type list
with schema authority; updates proposed status constraint to be vocabulary-driven;
adds SchemaInvalid failure path; specifies handling for unrecognized predicted types.
All existing pm_structuring scenarios (EmptyInput, NoExtractableContent,
PMRejectedExtraction, ApiRequestFailed, R2 folder scenarios) remain in force unchanged.

## Definitions

**Recognized type** — an entity type whose name matches a canonical type or alias
defined in the active vocabulary.

**Unrecognized type** — a type name predicted by the LLM that does not match any
canonical type or alias in the active vocabulary.

**Stale item_type** — an `item_type` value stored in a historical `ItemsExtracted`
event that is no longer recognized by the currently active vocabulary.

## Scenarios

### Happy Path 1: Schema vocabulary governs extraction type classification

```gherkin
Given the active vocabulary is loaded successfully
And the vocabulary defines one or more entity types (including any custom types)
When the PM submits text for extraction
Then extraction type classification is constrained to the vocabulary's recognized
  entity types
And each extracted item's item_type is a type recognized by the active vocabulary
And no hardcoded entity type list is consulted
```

### Happy Path 2: Unrecognized predicted type — item marked uncertain

```gherkin
Given the active vocabulary is loaded successfully
And the LLM predicts an item_type that is not recognized by the active vocabulary
When the PM reviews the extracted items
Then the item appears in the extraction result with uncertain=true
And the item's uncertainty_reason identifies the type as unrecognized by the vocabulary
And the item's item_type is not the raw unrecognized value predicted by the LLM
And the item is presented to the PM for review like any other uncertain item
And an unrecognized predicted type does not prevent confirmation or incorporation —
  the item is treated as an uncertain extraction candidate and may be confirmed by the PM
```

Note: the exact item_type value recorded for items whose predicted type is unrecognized
is resolved in Stage 3 event schema.

### Happy Path 3: Proposed status drawn from vocabulary status set

```gherkin
Given the active vocabulary is loaded successfully
And the LLM predicts a proposed_status for an extracted item of recognized type T
And the predicted proposed_status is present in the vocabulary's status set for T
When the PM reviews the extracted items
Then the item carries the proposed_status value
```

### Happy Path 4: Proposed status null when type is unrecognized

```gherkin
Given the active vocabulary is loaded successfully
And an extracted item's predicted type is not recognized by the active vocabulary
When the PM reviews the extracted items
Then the item's proposed_status is null regardless of any LLM status inference
And the item's proposed_priority is unaffected by the type resolution failure
```

### Happy Path 5: Proposed status null when outside vocabulary status set

```gherkin
Given the active vocabulary is loaded successfully
And the LLM infers a proposed_status value for an extracted item of recognized type T
And that proposed_status is not present in the vocabulary's status set for T
When the PM reviews the extracted items
Then the item's proposed_status is null
And the item is not marked uncertain solely because of this status constraint
```

### Happy Path 6: Historical stale item_type is preserved unchanged

```gherkin
Given an item was extracted and incorporated when type T was in the active vocabulary
And the vocabulary has since changed so that T is no longer a recognized type
When the historical event log is consulted
Then the item remains accessible with item_type T preserved exactly as it was written
And the item_type value in the historical ItemsExtracted event is never silently changed
  or removed
```

Note: observability of stale item_types when encountered by reading operations (e.g.,
item_status, item_links) is the responsibility of those modules' contracts. pm_structuring's
responsibility is faithful preservation of what was written.

### Failure Path 1: SchemaInvalid

```gherkin
Given the project schema file is present but contains a parse error or violates a
  structural validation rule
When the PM invokes any pm_structuring command (stdin or --folder mode)
Then the command fails with a schema error before any LLM call is made
And no extraction events are written
And no items are presented for review
```

---

## Invariants

- Entity type classification uses only the canonical entity types and aliases defined
  in the active vocabulary — no hardcoded type list is ever consulted
- An item's item_type stored in ItemsExtracted is never the raw unrecognized value
  predicted by the LLM; the exact representation for unrecognized items is resolved
  in Stage 3
- Proposed status validation occurs only after type resolution; if an item's type is
  unrecognized, proposed_status is null regardless of LLM inference
- Proposed status values for items of recognized types are always drawn from the
  vocabulary's status set for that type; out-of-vocabulary proposed statuses are
  silently set to null (not an error, not an uncertainty signal)
- A schema load failure always prevents any LLM invocation; the schema failure event
  is the terminal event for that invocation chain
- Vocabulary evolution never makes a historical item unreadable; recorded item_type
  values remain retrievable from the event log regardless of whether they are
  recognized by the active vocabulary — historical ItemsExtracted events are never
  modified and item_type values are preserved exactly as written
- When no project schema is supplied, the embedded default vocabulary preserves the
  existing five entity types and their status vocabularies unchanged
- Whether alias-produced item_type values are stored as the alias or normalized to the
  canonical name is resolved in Stage 3 event schema; this contract does not specify
  storage format
- All invariants from the existing pm_structuring contract remain in force

## Preconditions

- All preconditions from the existing pm_structuring contract apply
- Vocabulary availability is evaluated at command startup; the default vocabulary is
  embedded in the application binary — SchemaNotFound cannot occur

## Postconditions

- After successful extraction and confirmation: every incorporated item has an
  item_type that was either vocabulary-recognized at extraction time or the
  Stage-3-defined representation for unrecognized items
- After SchemaInvalid: no extraction events have been written; no LLM call was made;
  the project record is unchanged
- All postconditions from the existing pm_structuring contract remain in force

## Runtime Artifacts

No new payload fields are introduced to `ItemsExtracted`. The existing `uncertain`
and `uncertainty_reason` fields (already present in the ItemsExtracted payload) are
used to communicate unrecognized type detection. The vocabulary is read-only at
extraction time and is not written.

## Failure Classifications

| Failure Name | Trigger Condition | Observable Signal |
|---|---|---|
| SchemaInvalid | Project schema file present but contains parse error or structural validation error | Cross-module schema error from project_schema module; command fails before any LLM call; no extraction events written |

Note: SchemaInvalid maps to events in the project_schema event schema. The observable
signal for pm_structuring is the schema error plus the absence of any extraction event.

Note: Unrecognized predicted type is not a failure classification — it does not abort
the extraction. Items with unrecognized predicted types are included in the extraction
result as uncertain items.

Note: Out-of-vocabulary proposed_status is not a failure classification — it is
silently set to null.

---

status: APPROVED
feature_id: pm_structuring_schema_driven_entity_types
approved_by: human
approved_at: 2026-06-01
derived_from_intent: intents/pm_structuring_schema_driven_entity_types.md
amends_contract: contracts/pm_structuring_contract.md
derived_event_schema: events/pm_structuring_schema_driven_entity_types_schema.md

# Behavioral Contract: item_links_schema_integration

DERIVED FROM: intents/item_links_schema_integration.md
AMENDS: contracts/item_links_contract.md (replaces validation mechanism; adds
schema-specific failure paths; does not replace link lifecycle scenarios)

## Scenarios

### Happy Path 1: Vocabulary-defined relation type used to link items

```gherkin
Given the active vocabulary is loaded successfully
And the vocabulary defines relation type R
And a source item exists with a recognized entity type
And a target item exists with a recognized entity type
And no identical link already exists
When the PM adds a link of type R from the source item to the target item
Then the link is recorded
And querying the source item's links shows the link under R's forward label from the vocabulary
And querying the target item's links shows the link under R's inverse label from the vocabulary
```

### Happy Path 2: Updated vocabulary label reflected in link list and item link query output

```gherkin
Given one or more links exist in the project record with recognized relation types
And the active vocabulary defines forward label F and inverse label I for each link's relation type
When the PM lists all links
Then all links with recognized relation types are shown using forward label F on the source side
And all links with recognized relation types are shown using inverse label I on the target side
And no previously hardcoded label value appears in the output
```

### Happy Path 3: Links with unrecognized relation type produce observable signal and are excluded

```gherkin
Given the project record contains links of relation type R
And R is not present in the active vocabulary
When the PM lists links
Then a LinkRelationTypeUnknown event is emitted for each link of type R
And links of type R are excluded from the listing output
And the excluded links remain in the project record unchanged
And all links with recognized relation types are shown normally
```

### Failure Path 1: SchemaInvalid

```gherkin
Given the project schema file is present but contains a parse error or violates
  a structural validation rule
When the PM invokes any item-link command
Then the command fails with a schema error
And no link is recorded or removed
And the project record is unchanged
```

### Failure Path 2: InvalidRelationType

```gherkin
Given the active vocabulary is loaded successfully
And the PM specifies a relation type R that is not defined in the active vocabulary
When the PM attempts to add a link of type R
Then a failure result is returned identifying R as not recognized by the active vocabulary
And no link is recorded
And the project record is unchanged
```

### Failure Path 3: ItemTypeUnrecognized

```gherkin
Given the active vocabulary is loaded successfully
And the source item or target item has an entity type not recognized by the active vocabulary
When the PM attempts to add a link involving that item
Then a failure result is returned identifying which item's entity type is unrecognized
And no link is recorded
And the project record is unchanged
```

---

## Invariants

- Relation type validation always uses the active vocabulary at command startup —
  no hardcoded relation type set is ever consulted
- Relation source and target metadata in the vocabulary is informational only —
  any vocabulary-recognized relation type may be used between any two items
  regardless of their entity types; source/target fields exist as documentation
  and forward-compatibility hooks for future enforcement
- Forward and inverse labels in link command output — covering both link list output
  and item link query output — always match the labels defined in the active vocabulary
  at command startup; no previously hardcoded label value is ever used
- For read operations (link list, item link query): links with unrecognized relation
  types produce a LinkRelationTypeUnknown event per excluded link and are absent
  from the output; the command completes successfully
- For mutating operations (link add): items with unrecognized entity types cause
  the operation to fail before any state is recorded; no partial state is written
- Vocabulary evolution never prevents removal of an existing link — a link may be
  removed regardless of whether the current vocabulary recognizes the items' entity
  types or the link's relation type (rationale: schema changes must not leave the
  project record in an uncleanable state)
- A vocabulary error (parse or validation failure) always prevents any modification
  to the project record
- Query outputs are deterministic with respect to the combination of (event log,
  active vocabulary); the same event log replayed under a different vocabulary may
  produce different outputs — this is by design, as vocabulary is runtime
  configuration, not recorded state
- All invariants from the existing item_links contract remain in force for link
  lifecycle (recording, removal, querying, directionality)

## Preconditions

- All preconditions from the existing item_links contract apply
- Vocabulary availability is evaluated during command execution; the default
  vocabulary is embedded in the application binary — SchemaNotFound cannot occur

## Postconditions

- On successful link add: the link is visible under the vocabulary's forward label
  when querying the source item's links, and under the vocabulary's inverse label
  when querying the target item's links
- On successful link list: all displayed labels match the active vocabulary at the
  time of the command; links with unrecognized relation types are absent from the
  result and a LinkRelationTypeUnknown event has been emitted for each one
- On any failure: no link has been added or removed; the project record is unchanged

## Runtime Artifacts

No new artifacts beyond those declared in the existing item_links contract.
The vocabulary is read-only at command time and is not written.

## Failure Classifications

| Failure Name | Trigger Condition | Observable Signal |
|---|---|---|
| SchemaInvalid | Project schema file present but contains parse error or structural validation error | Cross-module schema error from `project_schema` module; command fails; project record unchanged |
| InvalidRelationType | Relation type not defined in active vocabulary | Failure result identifying unrecognized relation type; no link recorded |
| ItemTypeUnrecognized | Source or target item's entity type not recognized by active vocabulary (link add only) | Failure result identifying which item's entity type is unrecognized; no link recorded |

---

Note: SchemaInvalid maps to events in the `project_schema` event schema, emitted by
that module. The observable signal for item_links is the schema error + absence of
any link operation outcome. This is a cross-module observable consistent with the
pattern established by logseq_export_schema_integration.

Note: InvalidRelationType supersedes the validation mechanism described in the
existing item_links InvalidLinkType failure path. Whether it reuses the existing
LinkFailedInvalidLinkType event or introduces a new event is resolved in Stage 3.

---

status: APPROVED
feature_id: item_links_schema_integration
approved_by: human
approved_at: 2026-05-31
derived_from_intent: intents/item_links_schema_integration.md
amends_contract: contracts/item_links_contract.md
derived_event_schema: events/item_links_schema_integration_schema.md

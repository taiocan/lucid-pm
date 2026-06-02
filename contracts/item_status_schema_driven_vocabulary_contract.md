# Behavioral Contract: item_status_schema_driven_vocabulary

DERIVED FROM: intents/item_status_schema_driven_vocabulary.md
AMENDS: contracts/item_status_contract.md — replaces the hardcoded status vocabulary
table with schema authority; adds marker-derived status resolution at query time;
adds SchemaInvalid failure path covering writes and queries; adds ItemStatusUnrecognized
observational signal for stale recorded statuses. Priority scenarios, ItemNotFound,
InvalidPriorityValue, and proposed-value fallback scenarios from the existing contract
remain in force unchanged.

## Definitions

**Effective status** — the status value returned by the get command, resolved in
priority order: (1) explicit: the value from the most recent ItemStatusUpdated event;
(2) marker-derived: for task-type items only, the vocabulary mapping for the item's
Logseq task marker when no explicit event exists; (3) proposed: the proposed_status
value from extraction when no explicit event and no mapped marker exists; (4) null.

**Recorded status** — a status value stored in the event log via an explicit
ItemStatusUpdated event. A recorded status is always the source for resolution step 1.
Marker-derived and proposed values are not recorded statuses.

**Stale status** — a recorded status value that is no longer present in the active
vocabulary's status set for the item's entity type. A stale status is still the
effective status (resolution step 1 applies); it is never silently suppressed.

## Scenarios

### Happy Path 1: Schema-defined vocabulary governs set-status

```gherkin
Given the active vocabulary is loaded successfully
And the vocabulary defines one or more valid status values for the item's entity type
And the item exists in the project record
When the PM sets a status value that appears in the vocabulary's status set for that entity type
Then the item's status is updated to the new value
And no other item's status is affected
And status validation used the active vocabulary — no hardcoded status table was consulted
```

### Happy Path 2: Marker-derived effective status at query time

```gherkin
Given the active vocabulary is loaded successfully
And a task-type item exists in the project record
And the item's content carries a Logseq task marker M
And M is present in the vocabulary's task-marker mapping
And no ItemStatusUpdated event has been recorded for this item
When the PM queries the item's status via the get command
Then the effective status returned is the vocabulary's mapped value for M
And the result is marked as marker-derived, not as an explicit or proposed value
And no state change occurs
```

### Happy Path 3: Explicit status takes precedence over marker at query time

```gherkin
Given the active vocabulary is loaded successfully
And a task-type item exists in the project record
And the item's content carries a Logseq task marker M
And an ItemStatusUpdated event with status value S has been recorded for this item
When the PM queries the item's status via the get command
Then the effective status returned is S
And the marker M is not used to compute the effective status
And no state change occurs
```

### Happy Path 4: Unmapped marker falls through to proposed-value rule

```gherkin
Given the active vocabulary is loaded successfully
And a task-type item exists in the project record
And the item's content carries a Logseq task marker M
And M is not present in the vocabulary's task-marker mapping
And no ItemStatusUpdated event has been recorded for this item
And a proposed_status value P was recorded for this item at extraction time
When the PM queries the item's status via the get command
Then the effective status returned is P
And the marker M does not affect the result
And no failure signal is emitted for the unmapped marker
And no state change occurs
```

### Happy Path 5: Stale recorded status produces non-failure observational signal

```gherkin
Given the active vocabulary is loaded successfully
And an item exists in the project record with a recorded status value S
  (i.e. an ItemStatusUpdated event with value S is the most recent such event)
And S is not present in the active vocabulary's status set for the item's entity type
When the PM queries the item's status via the get command
Then S is the effective status (resolution step 1 applies — explicit update present)
And an ItemStatusUnrecognized event is emitted exactly once for this invocation
And S is returned by the get command
And the get command completes successfully — it is not treated as a failure
And no state change occurs
```

### Failure Path 1: SchemaInvalid

```gherkin
Given the project schema file is present but contains a parse error or violates a
  structural validation rule
When the PM invokes any set-status, set-priority, or get command
Then the command fails with a schema error before any state is modified or any
  result is returned
And no status or priority is recorded
And the project record is unchanged
```

### Failure Path 2: InvalidStatusForType

```gherkin
Given the active vocabulary is loaded successfully
And the vocabulary's status set for the item's entity type does not include the
  requested status value
When the PM attempts to set that status value on the item
Then a failure result is returned identifying the status as invalid for that entity type
And the item's current status is unchanged
And the project record is unchanged
```

Note: this failure applies equally when the entity type's status vocabulary is empty
(zero entries defined) — in that case every possible status value is invalid for that
type and set-status is always rejected with InvalidStatusForType.

---

## Invariants

- Status validation for every entity type uses only the active vocabulary at command
  startup — no hardcoded status table is ever consulted
- A schema load failure always prevents any command from completing — including
  read-only get; all commands are equally gated because accurate effective status
  resolution, including marker mapping, requires a successfully loaded vocabulary;
  priority commands are gated for the same reason even though priority values are not
  schema-defined in this release
- When no project schema is supplied, the embedded default vocabulary is used; it
  provides the same status vocabulary as the legacy hardcoded table, so no existing
  project's status commands are affected
- Vocabulary evolution never makes a recorded status value unreadable — a status
  value written via ItemStatusUpdated remains returnable by the get command regardless
  of whether it is still present in the active vocabulary
- An entity type defined in the vocabulary with an empty status set has no valid
  statuses — set-status always produces InvalidStatusForType for items of that type
- Effective status resolution priority (highest to lowest):
    1. Recorded status: most recent ItemStatusUpdated event
    2. Marker-derived: task-type item carries a mapped marker and no recorded status exists
    3. Proposed: no recorded status and no mapped marker (or item is not task-type)
    4. null
- An unmapped task marker produces no failure signal — resolution falls through to the
  proposed-value rule silently
- A stale recorded status (not in the active vocabulary) is always returned by the get
  command; it is never silently suppressed; an ItemStatusUnrecognized event is emitted
  exactly once per get invocation when the effective status is stale — this event is
  non-failure and does not abort the command
- Effective status resolution is deterministic with respect to (event log, item content,
  active vocabulary) — the same inputs always produce the same effective status
- All invariants from the existing item_status contract remain in force for priority
  lifecycle, item existence checks, and proposed-value fallback

## Preconditions

- All preconditions from the existing item_status contract apply
- Vocabulary availability is evaluated at command startup; the default vocabulary is
  embedded in the application binary — SchemaNotFound cannot occur

## Postconditions

- After set-status (success): the item's current status equals the newly set value,
  validated against the active vocabulary at command startup; all other items unchanged
- After set-priority (success): unchanged from the existing contract
- After get (success): the effective status reflects the resolution chain in order
  (recorded → marker-derived → proposed → null); if the effective status is a stale
  recorded status, exactly one ItemStatusUnrecognized event has been emitted and the
  stale value has been returned; no state change has occurred
- On SchemaInvalid: no status or priority has been recorded or returned; the project
  record is unchanged
- On InvalidStatusForType: the item's recorded status is unchanged; the project record
  is unchanged

## Runtime Artifacts

No new artifacts beyond those declared in the existing item_status contract.
The vocabulary is read-only at command time and is not written.

## Failure Classifications

| Failure Name | Trigger Condition | Observable Signal |
|---|---|---|
| SchemaInvalid | Project schema file present but contains parse error or structural validation error | Cross-module schema error from project_schema module; command fails before any state modification or result is returned; project record unchanged |
| InvalidStatusForType | Status value not present in the active vocabulary's status set for the item's entity type (including when that set is empty) | Failure result identifying the invalid status and entity type; item recorded status unchanged |

---

Note: SchemaInvalid maps to events in the project_schema event schema, emitted by
that module. The observable signal for item_status is the schema error plus the
absence of any status or priority operation outcome. This is the same cross-module
observable pattern used in item_links_schema_integration.

Note: ItemStatusUnrecognized is a non-failure observational event emitted by the get
command when the effective status is a stale recorded value. It is not listed in
Failure Classifications. Its payload is resolved in Stage 3; its cardinality is fixed
here: exactly one event per get invocation where the condition is present.

---

status: APPROVED
feature_id: item_status_schema_driven_vocabulary
approved_by: human
approved_at: 2026-06-01
derived_from_intent: intents/item_status_schema_driven_vocabulary.md
amends_contract: contracts/item_status_contract.md
derived_event_schema: events/item_status_schema_driven_vocabulary_schema.md

# Event Schema: task_model

DERIVED FROM:
- intents/task_model.md (actors, outcomes)
- contracts/task_model_contract.md (state transitions, failure modes)

---

## Required Base Fields (all events)

Every event must include these fields:

```json
{
  "event_id":       "uuid-v4",
  "event_type":     "EventName",
  "timestamp":      1710000000000,
  "correlation_id": "uuid-v4",
  "source_module":  "task_model",
  "payload":        {}
}
```

`correlation_id` is mandatory and must propagate through the entire
execution chain.

---

## Design Notes

**task_id is the unified identity:**
In this schema, `task_id` serves as both the project record item
identifier and the stable sync identity. The contract abstracted the
stable task identifier as an implementation choice; this schema
makes the coupling explicit: the `task_id` assigned at creation is
the same value embedded in the task block line at export time and
read back during sync. Implementations that need project item UUID
and sync correlation token to be distinct would require a schema
amendment.

**TaskAdded is authoritative; TaskAddRequested is optional:**
Every task instance creation — whether via direct user action or via
sync discovery — is recorded by a `TaskAdded` event. `TaskAddRequested`
is emitted only for direct user-initiated creation; it is absent from
the sync discovery path. `TaskAdded` is the authoritative record of
task existence; `TaskAddRequested` is an observational precursor for
user-initiated operations only.

**Sync discovery requires a stable identifier:**
Only task block lines that carry a stable identifier (`task_id`) are
eligible for discovery during sync. Task block lines with no
resolvable stable identifier are silently skipped — they produce
no `TaskAdded` and are not registered as task instances. This is the
architectural consequence of the contract's discovery definition:
Logseq-authored tasks must already carry a stable identifier to be
eligible for discovery. The system does not generate or assign
identifiers to identifier-less block lines during sync.

**Marker update during sync:**
When logseq_sync encounters a task block line whose `task_id` resolves
to a known task and whose marker differs from the stored current
marker, it emits `TaskMarkerUpdated`. Downstream effective-status
resolution reads the latest `TaskMarkerUpdated.new_marker` for the
task, falling back to `TaskAdded.initial_marker` if no update exists.
Reconstructing which parent item contained the task requires reading
the historical `TaskAdded` event; the parent association is not
repeated in `TaskMarkerUpdated` because it is immutable.

**TaskMarkerSyncSkipped:**
When a task block line's marker is not vocabulary-recognized during
sync, the task's state is left unchanged. No task_model event is
emitted for this case — the observable signal is purely behavioral
(unchanged state, sync continues). The logseq_sync module handles any
sync-level skip signalling through its own event spine.

**Validation order (task add):**
The contract does not prescribe an ordering between the three failure
conditions. The flow diagram represents them as a mutually exclusive
set; no sequence is implied.

---

## Event Definitions

### TaskAddRequested

- category: OBSERVATIONAL
- emitted when: task instance creation is requested by a user action
- payload:
  - `description`: `string` — the task description provided by the PM
  - `parent_item_id`: `string` — the parent item ID provided by the PM
  - `requested_marker`: `string | null` — the initial marker requested
    by the PM; null if no marker was specified (default will be used)

### TaskAdded

- category: BEHAVIORAL
- emitted when: a task instance is successfully created in the project
  record — either via user-initiated creation or via sync discovery
- payload:
  - `task_id`: `string (uuid-v4)` — the unified identity of this task
    instance; serves as both the project record item identifier and the
    stable sync identifier embedded in the Logseq task block line
  - `item_type`: `string` — the canonical task block type name as
    defined in the active vocabulary
  - `description`: `string` — the task description
  - `parent_item_id`: `string` — the identifier of the parent project
    record item
  - `initial_marker`: `string` — the task marker assigned at creation

### TaskMarkerUpdated

- category: BEHAVIORAL
- emitted when: a task block line's marker is found to differ from the
  task's current stored marker during sync, and the new marker is
  present in the vocabulary's block type marker mapping
- payload:
  - `task_id`: `string` — the unified identity of the task whose
    marker changed
  - `previous_marker`: `string` — the marker before the change
  - `new_marker`: `string` — the marker as read from the Logseq page

Note: the parent item association is not repeated here because it is
immutable and already recorded in the originating `TaskAdded` event.
Reconstructing the full task context requires reading `TaskAdded` for
this `task_id`.

### TaskAddFailedParentNotFound

- category: FAILURE
- emitted when: the parent item ID supplied to task add does not
  resolve to any item in the project record
- payload:
  - `failure_reason`: `string` — `"parent_not_found"`
  - `parent_item_id`: `string` — the ID that was not found

### TaskAddFailedSchemaInvalid

- category: FAILURE
- emitted when: a project schema file is present but fails to parse,
  fails structural validation, or contains an alias collision; task add
  aborts
- payload:
  - `failure_reason`: `string` — `"schema_invalid"`

Note: the specific cause of the schema failure is carried by
cross-module events from project_schema (`SchemaParseError`,
`SchemaValidationFailed`, or `SchemaAliasCollisionDetected`), not by
this event. This event records the task add business outcome only.

### TaskAddFailedTaskTypeNotDefined

- category: FAILURE
- emitted when: the active vocabulary loads successfully but defines no
  canonical task block type concept; task add cannot proceed
- payload:
  - `failure_reason`: `string` — `"task_type_not_defined"`

---

## Event Flow

```text
── task add ─────────────────────────────────────────────────────────────────

TaskAddRequested                      ← task creation requested by user action

Then exactly one of:

  TaskAddFailedSchemaInvalid          ← FAILURE (SchemaInvalid)
    accompanied by cross-module events from project_schema:
    SchemaParseError | SchemaValidationFailed | SchemaAliasCollisionDetected

  TaskAddFailedTaskTypeNotDefined     ← FAILURE (TaskTypeNotDefined)

  TaskAddFailedParentNotFound         ← FAILURE (ParentNotFound)

  TaskAdded                           ← BEHAVIORAL; task instance created

The three failure conditions are mutually exclusive; no evaluation
order between them is prescribed.

── logseq_sync (task-related portion) ───────────────────────────────────────

[within a sync run, per task block line encountered]

For each task block line, exactly one of:

  TaskMarkerUpdated                   ← BEHAVIORAL (known task_id; marker changed;
                                        new marker vocabulary-recognized)

  TaskAdded                           ← BEHAVIORAL (unknown task_id;
                                        marker vocabulary-recognized;
                                        task discovered)

  [no task_model event]               ← (TaskMarkerSyncSkipped: marker not
                                        vocabulary-recognized; task state unchanged)

  [no task_model event]               ← (no resolvable stable identifier;
                                        silently skipped)
```

---

## Cross-module events relied upon

| Event | Source module | Contract clause |
|---|---|---|
| SchemaParseError | project_schema | SchemaInvalid failure: schema file present but fails to parse — accompanies TaskAddFailedSchemaInvalid |
| SchemaValidationFailed | project_schema | SchemaInvalid failure: schema file present but fails structural validation — accompanies TaskAddFailedSchemaInvalid |
| SchemaAliasCollisionDetected | project_schema | SchemaInvalid failure: schema file present but contains alias collision — accompanies TaskAddFailedSchemaInvalid |

Note: SchemaNotFound is not listed because the embedded default
vocabulary guarantees a vocabulary is always present. SchemaNotFound
cannot occur during task operations (contract: Embedded default
vocabulary definition; Boundary Scenarios 1 and 2).

---

## Coverage Check

### Contract scenarios → events

| Contract Scenario | Event(s) | Status |
|---|---|---|
| HP1: Task created via direct command | `TaskAddRequested`, `TaskAdded` | COVERED |
| HP2: Task visible in priority-filtered view | (no new events — task visibility is via project record reads) | COVERED — by design |
| HP3: Task effective status from current marker | (no new events — marker-derived resolution reads `TaskAdded.initial_marker` and `TaskMarkerUpdated.new_marker`) | COVERED — by design |
| HP4: Task exported as nested block line | (no new events — logseq_export amendment; existing export events unchanged) | COVERED — by design |
| HP5: Task marker change synced from Logseq | `TaskMarkerUpdated` | COVERED |
| HP6: Task discovered in Logseq during sync | `TaskAdded` (same event as direct creation) | COVERED |
| HP7: Typed link between task and another item | (no new events — item_links handles link creation) | COVERED — by design |
| BS1: No schema — behavior unchanged when no tasks | (no new events — embedded default vocabulary handles silently) | COVERED — by design |
| BS2: No schema — default vocabulary used when tasks present | (no new events — default vocabulary is active) | COVERED — by design |
| BS3: Repeated sync — no duplicate task instances | (deduplication: same stable identifier → `TaskAdded` emitted only on first discovery) | COVERED — by design |
| BS4: Parent has no tasks — export page unaffected | (no new events — logseq_export behavior unchanged for task-less parents) | COVERED — by design |
| FP1: ParentNotFound | `TaskAddRequested`, `TaskAddFailedParentNotFound` | COVERED |
| FP2: SchemaInvalid | cross-module events, `TaskAddRequested`, `TaskAddFailedSchemaInvalid` | COVERED |
| FP3: TaskTypeNotDefined | `TaskAddRequested`, `TaskAddFailedTaskTypeNotDefined` | COVERED |
| FP4: TaskMarkerSyncSkipped | (no task_model event — behavioral outcome only) | COVERED — by design |

### Contract failures → FAILURE events

| Contract Failure | FAILURE Event | Status |
|---|---|---|
| ParentNotFound | `TaskAddFailedParentNotFound` | COVERED |
| SchemaInvalid | `TaskAddFailedSchemaInvalid` | COVERED |
| TaskTypeNotDefined | `TaskAddFailedTaskTypeNotDefined` | COVERED |
| TaskMarkerSyncSkipped | (no FAILURE event — observable signal is unchanged state) | COVERED — by design |

---

status: APPROVED
feature_id: task_model
approved_by: human
approved_at: 2026-06-04
derived_from_intent: intents/task_model.md
derived_from_contract: contracts/task_model_contract.md

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
`task_id` serves as both the project record item identifier and the
stable sync identity. The `task_id` assigned at creation is the same
value embedded in the task block line at export time and read back
during sync. Implementations that need the project item UUID and sync
correlation token to be distinct would require a schema amendment.

**TaskAdded is authoritative; TaskAddRequested is optional:**
Every task instance creation — via direct user action or via sync
discovery — is recorded by a `TaskAdded` event. `TaskAddRequested` is
emitted only for direct user-initiated creation; it is absent from the
sync discovery path.

**owner_id in TaskAdded:**
`owner_id` carries the item ID of the assigned owner stakeholder. When
no owner was specified at creation, `owner_id` is the item ID of the
TBD placeholder stakeholder. The TBD placeholder is a stakeholder
entity whose existence is an invariant of any project running
task_model; its item ID is an implementation detail resolved at
runtime. `owner_id` is never null.

**Additive payload fields in TaskAdded:**
`owner_id`, `scheduled_date`, and `deadline` are new fields in
`TaskAdded`. They do not alter the semantics of existing fields.
`TaskAdded` events emitted before this refinement are treated as
having `owner_id = TBD placeholder ID`, `scheduled_date = null`,
`deadline = null` for backward compatibility.

**Sync discovery — owner defaults to TBD:**
Task instances discovered via sync (unknown `task_id`) are registered
with `owner_id = TBD placeholder ID`. Their dates are populated from
any SCHEDULED and DEADLINE lines found in the block if present.

**Multiple sync events per task block line:**
A single task block line can produce zero, one, or more of
`TaskMarkerUpdated`, `TaskOwnerUpdated`, and `TaskDatesUpdated` in the
same sync run if multiple attributes have changed simultaneously. These
events are independent; no ordering between them is prescribed.

**Sync discovery requires a stable identifier:**
Only task block lines that carry a stable identifier (`task_id`) are
eligible for sync discovery. Task block lines with no resolvable stable
identifier are silently skipped — they produce no events and are not
registered as task instances.

**Validation order (task add):**
The contract does not prescribe ordering between the four failure
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
  - `requested_marker`: `string | null` — the initial marker requested;
    null if not specified
  - `requested_owner_id`: `string | null` — the owner item ID provided
    by the PM; null if not specified (TBD placeholder will be used)
  - `requested_scheduled_date`: `string | null` — ISO date (YYYY-MM-DD)
    or null if not specified
  - `requested_deadline`: `string | null` — ISO date (YYYY-MM-DD) or
    null if not specified

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
  - `owner_id`: `string` — the item ID of the owner stakeholder;
    equals the TBD placeholder item ID when no named owner was specified
  - `scheduled_date`: `string | null` — ISO date (YYYY-MM-DD) of the
    scheduled start; null if not set
  - `deadline`: `string | null` — ISO date (YYYY-MM-DD) of the
    deadline; null if not set

### TaskMarkerUpdated

- category: BEHAVIORAL
- emitted when: a task block line's marker is found to differ from the
  task's current stored marker during sync, and the new marker is
  present in the vocabulary's block type marker mapping
- payload:
  - `task_id`: `string` — the identity of the task whose marker changed
  - `previous_marker`: `string` — the marker before the change
  - `new_marker`: `string` — the marker as read from the Logseq page

Note: the parent item association is not repeated here because it is
immutable and already recorded in the originating `TaskAdded` event.

### TaskOwnerUpdated

- category: BEHAVIORAL
- emitted when: a task block line's owner reference changes during sync
  and the referenced name resolves to a known stakeholder in the
  project record
- payload:
  - `task_id`: `string` — the identity of the task whose owner changed
  - `previous_owner_id`: `string` — the item ID of the owner before
    the change
  - `new_owner_id`: `string` — the item ID of the newly resolved owner

Note: when the owner reference in Logseq does not resolve to any known
stakeholder, no event is emitted and the owner is unchanged (contract:
Boundary Scenario 7).

### TaskDatesUpdated

- category: BEHAVIORAL
- emitted when: a task block line's SCHEDULED or DEADLINE date changes
  during sync (one or both may change in a single sync run)
- payload:
  - `task_id`: `string` — the identity of the task whose dates changed
  - `previous_scheduled_date`: `string | null` — ISO date before the
    change; null if no scheduled date was previously stored
  - `new_scheduled_date`: `string | null` — ISO date after the change;
    null if the date was cleared
  - `previous_deadline`: `string | null` — ISO date before the change;
    null if no deadline was previously stored
  - `new_deadline`: `string | null` — ISO date after the change; null
    if the date was cleared

### TaskAddFailedParentNotFound

- category: FAILURE
- emitted when: the parent item ID supplied to task add does not
  resolve to any item in the project record
- payload:
  - `failure_reason`: `string` — `"parent_not_found"`
  - `parent_item_id`: `string` — the ID that was not found

### TaskAddFailedOwnerNotFound

- category: FAILURE
- emitted when: the owner item ID supplied to task add does not resolve
  to any stakeholder item in the project record
- payload:
  - `failure_reason`: `string` — `"owner_not_found"`
  - `owner_id`: `string` — the ID that was not found

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

TaskAddRequested                       ← task creation requested by user action

Then exactly one of:

  TaskAddFailedSchemaInvalid           ← FAILURE (SchemaInvalid)
    accompanied by cross-module events from project_schema:
    SchemaParseError | SchemaValidationFailed | SchemaAliasCollisionDetected

  TaskAddFailedTaskTypeNotDefined      ← FAILURE (TaskTypeNotDefined)

  TaskAddFailedParentNotFound          ← FAILURE (ParentNotFound)

  TaskAddFailedOwnerNotFound           ← FAILURE (OwnerNotFound)

  TaskAdded                            ← BEHAVIORAL; task instance created
                                         with owner_id, scheduled_date, deadline

The four failure conditions are mutually exclusive; no evaluation
order between them is prescribed.

── logseq_sync (task-related portion) ───────────────────────────────────────

[within a sync run, per task block line encountered]

Case 1 — known task_id (task exists in project record):
  Zero or more of the following, independently, one per changed
  attribute:

    TaskMarkerUpdated     ← marker changed; new marker vocabulary-recognized
    TaskOwnerUpdated      ← owner reference changed; resolves to known stakeholder
    TaskDatesUpdated      ← scheduled date or deadline changed

  If marker changed but not vocabulary-recognized: no event; state unchanged
  If owner reference changed but name not resolvable: no event; owner unchanged

Case 2 — unknown task_id; marker vocabulary-recognized:
  TaskAdded               ← BEHAVIORAL; discovered task; owner_id = TBD placeholder;
                            dates populated if SCHEDULED/DEADLINE lines present

Case 3 — no resolvable stable identifier:
  [no task_model event]   ← silently skipped
```

---

## Cross-module events relied upon

| Event | Source module | Contract clause |
|---|---|---|
| SchemaParseError | project_schema | SchemaInvalid failure: schema file present but fails to parse — accompanies TaskAddFailedSchemaInvalid |
| SchemaValidationFailed | project_schema | SchemaInvalid failure: schema file present but fails structural validation — accompanies TaskAddFailedSchemaInvalid |
| SchemaAliasCollisionDetected | project_schema | SchemaInvalid failure: schema file present but contains alias collision — accompanies TaskAddFailedSchemaInvalid |

Note: SchemaNotFound is not listed because the embedded default
vocabulary guarantees a vocabulary is always present.

---

## Coverage Check

### Contract scenarios → events

| Contract Scenario | Event(s) | Status |
|---|---|---|
| HP1: Task created via direct command | `TaskAddRequested`, `TaskAdded` | COVERED |
| HP2: Task visible in priority-filtered view | (no new events — task visibility via project record reads) | COVERED — by design |
| HP3: Task effective status from current marker | (no new events — reads `TaskAdded.initial_marker` + `TaskMarkerUpdated.new_marker`) | COVERED — by design |
| HP4: Task exported as nested block line with owner + dates | (no new events — logseq_export amendment; `TaskAdded` payload carries owner + dates) | COVERED — by design |
| HP5: Task marker change synced from Logseq | `TaskMarkerUpdated` | COVERED |
| HP6: Task discovered in Logseq during sync | `TaskAdded` (owner_id = TBD) | COVERED |
| HP7: Typed link between task and another item | (no new events — item_links handles) | COVERED — by design |
| HP8: Task created with named owner | `TaskAddRequested`, `TaskAdded` (owner_id = S) | COVERED |
| HP9: Task created without owner — TBD assigned | `TaskAddRequested`, `TaskAdded` (owner_id = TBD) | COVERED |
| HP10: Task created with scheduled date and deadline | `TaskAddRequested`, `TaskAdded` (scheduled_date, deadline set) | COVERED |
| HP11: Ownership change synced from Logseq | `TaskOwnerUpdated` | COVERED |
| HP12: Date changes synced from Logseq | `TaskDatesUpdated` | COVERED |
| BS1: No schema — behavior unchanged when no tasks | (no new events) | COVERED — by design |
| BS2: No schema — default vocabulary used when tasks present | (no new events) | COVERED — by design |
| BS3: Repeated sync — no duplicate task instances | (deduplication: same task_id → no new `TaskAdded`) | COVERED — by design |
| BS4: Parent has no tasks — export page unaffected | (no new events) | COVERED — by design |
| BS5: Task without dates is valid in all operations | (no new events — null dates in `TaskAdded` is valid) | COVERED — by design |
| BS6: TBD-owned task participates in all queries | (no new events — TBD owner_id in `TaskAdded` is valid) | COVERED — by design |
| BS7: Sync with unresolvable owner — owner unchanged | (no event — contract: silent skip) | COVERED — by design |
| FP1: ParentNotFound | `TaskAddRequested`, `TaskAddFailedParentNotFound` | COVERED |
| FP2: SchemaInvalid | cross-module events, `TaskAddRequested`, `TaskAddFailedSchemaInvalid` | COVERED |
| FP3: TaskTypeNotDefined | `TaskAddRequested`, `TaskAddFailedTaskTypeNotDefined` | COVERED |
| FP4: TaskMarkerSyncSkipped | (no task_model event — behavioral outcome only) | COVERED — by design |
| FP5: OwnerNotFound | `TaskAddRequested`, `TaskAddFailedOwnerNotFound` | COVERED |

### Contract failures → FAILURE events

| Contract Failure | FAILURE Event | Status |
|---|---|---|
| ParentNotFound | `TaskAddFailedParentNotFound` | COVERED |
| OwnerNotFound | `TaskAddFailedOwnerNotFound` | COVERED |
| SchemaInvalid | `TaskAddFailedSchemaInvalid` | COVERED |
| TaskTypeNotDefined | `TaskAddFailedTaskTypeNotDefined` | COVERED |
| TaskMarkerSyncSkipped | (no FAILURE event — observable signal is unchanged state) | COVERED — by design |

---

status: APPROVED
feature_id: task_model
approved_by: human
approved_at: 2026-06-08
derived_from_intent: intents/task_model.md
derived_from_contract: contracts/task_model_contract.md

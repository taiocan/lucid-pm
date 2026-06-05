# Behavioral Contract: task_model

DERIVED FROM: intents/task_model.md

---

## Definitions

**Task instance** — a project record item whose entity type resolves to
the canonical task block type concept in the active vocabulary. A task
instance has a unique identifier, a description, exactly one parent
item association, and a current marker.

**Canonical task block type concept** — the entity type concept in the
active vocabulary under which task markers are defined. Its
representation in the vocabulary determines which items are recognized
as task instances for all downstream operations. If the active
vocabulary defines no such concept, task operations that require it
fail with TaskTypeNotDefined.

**Embedded default vocabulary** — a vocabulary built into the
application binary that is always available when no project schema is
supplied. The embedded default vocabulary always defines a canonical
task block type concept and a non-empty block type marker mapping.
SchemaNotFound cannot occur during task operations — the embedded
default vocabulary guarantees that a vocabulary is always present.

**Current marker** — the Logseq task marker associated with a task
instance's current state (e.g., `TODO`, `DONE`, `DOING`). Initialized
at creation; updated when a changed vocabulary-mapped marker is
observed during sync.

**Stable task identifier** — an identifier associated with a task
instance that allows sync operations to correlate a task block line in
a Logseq page to the same logical task instance across export and sync
runs. The stable identifier is embedded in the task block line at
export time and read back during sync. Its concrete representation is
an implementation choice.

**Task block line** — a line in a Logseq item page representing a task
instance, nested within the parent item's page rather than rendered as
a separate page. A task block line carries the task's current marker,
its stable identifier, and its description. The exact serialization
format is an implementation choice.

**Discovered task** — a task instance registered in the project record
from a task block line found in a Logseq page during sync, where the
task block line carries a stable identifier that does not match any
existing project record item. Task block lines carrying no stable
identifier are not eligible for discovery.

---

## Scenarios

### Happy Path 1: Task created via direct command

```gherkin
Given the active vocabulary is loaded successfully
And the vocabulary defines a canonical task block type concept
And a project record item with ID P exists
When the PM runs task add with a description and parent item ID P
Then a task instance is created in the project record
And the task instance is associated with parent item P
And the task instance appears in the project record view alongside
  other items
And the task instance is immediately eligible for status queries,
  priority assignment, link creation, and AI analysis
```

### Happy Path 2: Task visible in priority-filtered view

```gherkin
Given a task instance T exists in the project record with priority set
  to high
And the active vocabulary is loaded successfully
When the PM runs a priority-filtered view requesting high-priority items
Then task instance T appears in the output
And T's output shows its description and its entity type in canonical
  vocabulary representation
```

### Happy Path 3: Task effective status derived from current marker

```gherkin
Given a task instance T exists in the project record with current
  marker M
And M is present in the vocabulary's block type marker mapping
And no explicit status has been set for T via the status command
When the PM queries T's status
Then the effective status is the vocabulary's mapped value for M
And the status is identified as marker-derived
And no state change occurs
```

### Happy Path 4: Task exported as nested block line under parent's page

```gherkin
Given the project record contains parent item P and task instance T
  with parent P
When the PM triggers an export
Then P's Logseq page contains T rendered as a task block line
And the task block line carries T's current marker, T's stable
  identifier, and T's description
And NO separate Logseq page is created for T
And all non-task items are exported as standalone Logseq pages per
  existing behavior
```

### Happy Path 5: Task marker change synced from Logseq

```gherkin
Given task instance T exists in the project record with current marker M
And the parent item's Logseq page shows T's task block line with
  marker M' where M' != M
And M' is present in the vocabulary's block type marker mapping
When the PM triggers a sync
Then T's effective status in the project record reflects the
  vocabulary's mapped value for M'
And T's current marker becomes M'
And no new task instance is created
And all other sync operations for other items complete as before
```

### Happy Path 6: Task discovered in Logseq during sync

```gherkin
Given the Logseq page for project record item P contains a task block
  line whose stable identifier does not match any existing project
  record item
And the task block line's marker is present in the vocabulary's block
  type marker mapping
When the PM triggers a sync
Then a task instance is created in the project record, identified by
  the stable identifier from the block line
And the task instance is associated with parent item P
And the task instance's effective status reflects the block line's
  marker via the vocabulary mapping
And this task instance is subsequently indistinguishable in the project
  record from a task created via direct command
```

### Happy Path 7: Typed link between task and another item

```gherkin
Given task instance T exists in the project record
And project record item Q exists
And the active vocabulary defines a relation type R valid between T's
  entity type and Q's entity type
When the PM creates a link of type R from T to Q
Then the link is recorded and queryable in the project record
And downstream queries over item links include this link
```

### Boundary Scenario 1: No project schema — behavior unchanged when no tasks present

```gherkin
Given no project schema is supplied
And the project record contains no task instances
When any existing command is invoked
Then the command's output and behavior are identical to pre-task_model
  behavior for the same project state
And no error or warning related to task vocabulary is signalled
```

### Boundary Scenario 2: No project schema — default vocabulary used when tasks present

```gherkin
Given no project schema is supplied
And the project record contains a task instance T with current marker M
And M is present in the embedded default vocabulary's block type marker
  mapping
When the PM queries T's status
Then the effective status is the default vocabulary's mapped value for M
And no schema error is signalled
```

### Boundary Scenario 3: Repeated sync — no duplicate task instances

```gherkin
Given task instance T exists in the project record with current marker M
And the parent item's Logseq page shows T's task block line with the
  same marker M (unchanged)
When the PM triggers sync N times (N >= 2) with no marker change between
  runs
Then exactly one task instance T exists in the project record after all
  N runs
And T's effective status is unchanged after the first run
```

### Boundary Scenario 4: Parent item has no tasks — export page unaffected

```gherkin
Given project record item P has no task instances associated with it
When the PM triggers an export
Then P's Logseq page contains no task block lines
And P's page content is identical to what it would be without task_model
```

### Falsification Scenario: Alias-stored task type included in project queries

```gherkin
Given the vocabulary defines canonical task block type "Task" with alias
  "task"
And task instance T1 is stored in the project record with type "Task"
  (canonical)
And task instance T2 is stored with type "task" (alias)
When the PM runs the project record view
Then both T1 and T2 appear in the output
Falsifies: query eligibility is determined by direct string comparison
           against the canonical type name "Task" — T2 stored as "task"
           would be excluded rather than resolved to the same concept.
```

### Failure Path 1: ParentNotFound

```gherkin
Given no project record item with ID P exists
When the PM runs task add specifying parent item ID P
Then a failure result is returned indicating the parent item was not
  found
And no task instance is created
And the project record is unchanged
```

### Failure Path 2: SchemaInvalid (task add)

```gherkin
Given a project schema file is present but cannot be parsed or fails
  structural validation
When the PM runs task add
Then a failure result is returned
And no task instance is created
And the project record is unchanged
```

### Failure Path 3: TaskTypeNotDefined

```gherkin
Given the active vocabulary is loaded successfully
And the vocabulary defines no canonical task block type concept
When the PM runs task add
Then a failure result is returned indicating no task type is defined in
  the active vocabulary
And no task instance is created
And the project record is unchanged
```

### Failure Path 4: TaskMarkerSyncSkipped — unrecognized marker on task block

```gherkin
Given task instance T exists in the project record with current marker M
And the parent item's Logseq page shows T's task block line with
  marker M'
And M' is NOT present in the vocabulary's block type marker mapping
When the PM triggers a sync
Then T's effective status is not updated for this marker
And T's current marker remains M
And sync completes for all other tasks and items
```

---

## Invariants

- **Identity invariant:** two task instances with different stable task
  identifiers are distinct logical tasks, regardless of description,
  marker, or parent association; two task block lines with the same
  stable identifier represent the same logical task in the project record
- A task instance is a first-class participant in the project record —
  it is reachable by any query or operation whose contract states it
  operates on project record items generically, unless that feature
  explicitly excludes task instances in its own contract
- Task state is always evaluated through the vocabulary-defined block
  type marker mapping — no raw marker representation enters domain
  status comparisons
- The absence of a project schema does not leave task state
  unevaluable — the embedded default vocabulary's block type marker
  mapping is always available
- A task instance created via direct command and a task instance
  discovered via Logseq sync are indistinguishable in the project
  record — downstream features cannot determine which creation path was
  used
- A task instance corresponds to exactly one logical task — repeated
  synchronization or export operations never produce additional task
  instances representing the same task
- Every task instance is associated with exactly one parent item; this
  association is preserved and queryable for the lifetime of the task
  instance
- Task instances are rendered in Logseq as task block lines nested under
  their parent item's page — no task instance produces a standalone
  Logseq page in any export operation
- A task block line carrying no resolvable stable identifier is neither
  assigned to an existing task instance nor registered as a discovered
  task during sync — it is silently skipped
- The absence of task instances from the project record leaves the
  behavior of all existing commands unchanged

## Vocabulary Dependency

**Vocabulary owner:** project_schema module  
**Concepts operated on:** canonical task block type concept (for task
entity type identity, query inclusion, and type display); block type
marker-to-status mapping (maps current marker to domain status concept);
vocabulary-defined valid status values per entity type concept (consulted
when explicit status operations are performed on a task instance via
item_status)  
**Concept Dependency Invariant (governing):** Task identity resolution,
marker-derived status outcomes, and eligibility for project queries are
invariant under substitution of equivalent vocabulary representations. A
task stored with the canonical task type representation and a task stored
with an alias of that type must produce identical outcomes in all
operations.  
**Representation Ban invariant (derived):** Stored type representations
— canonical names, aliases, casing variations — must not appear as
inputs to domain decision logic for task identity resolution, status
derivation, or query eligibility. Operations receive the resolved
concept, not the stored string.  
**Display invariant:** When a task instance's entity type is displayed
in any output, the canonical representation associated with the resolved
concept is used, regardless of the stored representation.

---

## Invariant Falsification Scenarios

| Invariant | Falsifying fixture | Observable when correct | Wrong implementation assumption | Test ID |
|---|---|---|---|---|
| Task is first-class in queries with generic scope | 1 task instance, 1 non-task item in project record; run project_state view (generic scope) | Both items appear in the output | View skips items whose type is not found in pageTypes; block types excluded | `test_task_first_class_falsifies_page_types_only_check` |
| No raw marker in domain comparisons | Vocabulary: marker "DONE"→"done"; task has marker "DONE"; filter by status "done" | Task appears in filter results | Marker string "DONE" compared directly against status filter "done" → string mismatch; task excluded | `test_no_raw_marker_in_comparisons_falsifies_direct_string_match` |
| Direct command ≡ Logseq-discovered | Create T1 via task add; create T2 via sync discovery; run project_state view and status get on both | T1 and T2 appear with identical structure; no field distinguishes creation origin | task add stores a creation_source field absent from synced tasks; queries expose the difference | `test_direct_and_discovered_tasks_indistinguishable_falsifies_origin_field` |
| One instance per logical task | Task T in project record; run sync 3 times, task block line unchanged | Exactly 1 task instance T after all 3 runs | Sync creates a new task instance on each run that encounters a task block | `test_one_instance_per_task_falsifies_duplicate_on_each_sync` |
| Parent association preserved | Create task T with parent P; run project_state view | T's record includes parent item ID P | Parent association not stored in the creation event; queries cannot return it | `test_parent_association_preserved_falsifies_missing_parent_field` |
| No standalone page for task | Parent P and task T with parent P; run export | No page slug for T exists in pages/; T's block line appears in P's page only | Export treats all items uniformly and creates pages/ entries for all item types | `test_no_standalone_page_falsifies_uniform_page_creation` |
| Absent tasks leave existing behavior unchanged | Project record with 0 task instances; run project_state view | Output identical to pre-task_model for the same record | task_model changes the item-loading path unconditionally; empty task list alters output | `test_absent_tasks_leave_behavior_unchanged_falsifies_unconditional_code_path` |
| Concept Dependency — alias equals canonical for type resolution | Vocabulary: canonical "Task", alias "task"; T1 stored as "Task", T2 stored as "task"; run project_state view | Both T1 and T2 in output | String comparison against "Task"; "task"-stored item excluded | `test_alias_type_resolves_same_as_canonical_falsifies_string_match` |
| Concept Dependency — marker mapping uses concept not representation | Vocabulary: canonical "Task", alias "task"; both items have marker "TODO"→"todo"; run status filter on "todo" | Both items appear regardless of stored type representation | Status filter compares stored type representation against canonical before resolving; alias-stored item missed | `test_marker_mapping_uses_concept_not_raw_marker_falsifies_no_mapping` |
| Identity invariant — different stable identifiers = distinct tasks | Two task block lines under same parent P, identical description "Review", different stable identifiers; run sync | Two distinct task instances exist in project record | Implementation uses description+parent as identity key rather than stable identifier; two tasks with same description collapse into one | `test_identity_invariant_different_ids_are_distinct_tasks_falsifies_desc_parent_identity` |

---

## Preconditions

- For task add: the project record is accessible; the active vocabulary
  is loaded successfully and defines a canonical task block type concept;
  the specified parent item exists in the project record
- For export with tasks: all preconditions from the logseq_export
  contract apply; task instances have parent items present in the
  project record at export time
- For sync with tasks: all preconditions from the logseq_sync contract
  apply; item pages may or may not contain task block lines

## Postconditions

- After successful task add: a task instance exists in the project
  record associated with the specified parent item, carrying the
  specified initial marker; the creation is recorded in the event log
- After export with tasks: each task instance appears as a task block
  line nested in its parent item's Logseq page; no task instance has a
  standalone page; all other items are exported per existing behavior
- After sync with task changes: each recognized task block with a
  changed vocabulary-mapped marker has its effective status updated and
  current marker updated; newly discovered tasks are registered in the
  project record; no duplicate instances are created
- On ParentNotFound: no task instance created; project record unchanged
- On SchemaInvalid: no task instance created; project record unchanged
- On TaskTypeNotDefined: no task instance created; project record
  unchanged

## Runtime Artifacts

| Artifact | Path | Lifecycle |
|---|---|---|
| None beyond events/runtime_events.jsonl | — | — |

### Cross-module signals relied upon

| Event | Source module | When relied upon |
|---|---|---|
| SchemaParseError | project_schema | When a project schema file is present but fails to parse; task add aborts alongside this event |
| SchemaValidationFailed | project_schema | When a project schema file present but fails structural validation; task add aborts alongside this event |
| SchemaAliasCollisionDetected | project_schema | When alias collision is detected in a project schema; task add aborts alongside this event |

Note: SchemaNotFound cannot occur during task operations — the
embedded default vocabulary is always present. Only parse, validation,
and alias-collision failures from a present-but-invalid project schema
file are relevant here.

Note: task add also requires that the project record contains the
specified parent item. This is a dependency on project record state
(not a specific event signal): the operation reads whatever items are
currently recorded and fails if the parent is absent.

---

## Amendments to existing features

### logseq_export — behavioral amendment

Task instances in the project record are rendered as task block lines
nested under their parent item's Logseq page, not as standalone Logseq
pages. Each task block line carries the task's current marker, the
task's stable identifier, and the task's description. No other change
to logseq_export behavior.

Clarification to existing invariant: "Every item in the project record
appears in the exported output on a successful export" — for task
instances, appearing in the output means appearing as a task block line
nested in the parent item's page, not as a standalone page.

All existing logseq_export events and their schemas are unchanged.

### logseq_sync — behavioral amendment

In addition to reading page-level status and priority properties, the
sync operation also scans task block lines nested in item pages.

For each task block line whose stable identifier resolves to a known
project record task instance: if the block line's marker differs from
the task's current stored marker and the new marker is present in the
vocabulary's block type marker mapping, the task's effective status and
current marker are updated.

For each task block line whose stable identifier does not resolve to
any existing project record item and whose marker is
vocabulary-recognized: the task is registered as a new task instance
(discovered task) with the owning page's item as parent.

Task block lines carrying no resolvable stable identifier are silently
skipped.

All existing logseq_sync events and their schemas are unchanged.

---

## Failure Classifications

| Failure Name | Trigger Condition | Observable Signal |
|---|---|---|
| ParentNotFound | Parent item ID specified in task add does not exist in the project record | Failure result returned; no task created; project record unchanged |
| SchemaInvalid | A project schema file is present but fails to parse, fails structural validation, or contains an alias collision | Schema error signals (SchemaParseError, SchemaValidationFailed, or SchemaAliasCollisionDetected) from project_schema module; task_model business-outcome failure signal; no task created; project record unchanged |
| TaskTypeNotDefined | Active vocabulary loaded successfully but defines no canonical task block type concept | Failure result returned indicating no task type is defined; no task created; project record unchanged |
| TaskMarkerSyncSkipped | A task block line's marker is not present in the vocabulary's block type marker mapping during sync | Task's effective status and current marker are unchanged; sync completes for remaining tasks and items |

---

status: APPROVED
feature_id: task_model
approved_by: human
approved_at: 2026-06-04
derived_from_intent: intents/task_model.md
derived_event_schema: events/task_model_schema.md

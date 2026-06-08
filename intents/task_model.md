# Intent: task_model

`task_model` exists to let the PM record and track individual task instances
within the project record — with an identified owner and optional temporal
coordinates — so that the task vocabulary defined by the project schema
becomes operational, task-level accountability is visible, and task work
participates in all existing project queries and analyses.

The **owner** of a task is the stakeholder to whom the task is assigned.
The **TBD placeholder** is a reserved stakeholder identity used when no
owner is specified — it satisfies the ownership requirement without naming
a person.

Specifically:
- PM can create a task instance associated with a parent project item and
  have it persist in the project record
- PM can view task instances through existing project queries — priority
  view, status queries, and AI enrichment analysis — with task state
  evaluated through the vocabulary-defined marker mapping
- PM can have task state changes made in Logseq reflected in the project
  record without re-running extraction
- PM can create typed links between task instances and other project record
  items using the established link vocabulary
- PM can export task instances to Logseq as native task block lines nested
  under their parent item's page
- PM can assign a task to a named owner at creation time; when no owner is
  specified, the task is assigned to the TBD placeholder
- PM can have ownership assignments reflected in the project record when
  they change in Logseq
- PM can record a scheduled start date and a deadline for a task at
  creation time
- PM can have scheduled date and deadline changes reflected in the project
  record when they change in Logseq

## Stable Guarantees

- A task instance is a first-class participant in the project record —
  reachable by any query or operation that applies to project record items,
  unless that feature explicitly excludes it in its own contract
- A task instance is visible to all existing project-record queries and
  analyses — the absence of a task from any query is never the result of
  a type mismatch between task instances and the query layer
- Task state is always evaluated through the vocabulary-defined marker
  mapping — no raw marker representation enters domain comparisons
- Task state evaluation always has an available marker mapping — the
  absence of a project schema does not leave task state unevaluable
- A task instance created via direct command and a task instance discovered
  via Logseq are indistinguishable in the project record — downstream
  features cannot tell them apart
- A task instance corresponds to exactly one logical task — repeated
  synchronization or export operations do not create additional task
  instances representing the same task
- A task instance's association with its parent item is preserved and
  queryable for the lifetime of the task instance
- Every task instance in the project record is associated with exactly one
  owner — either a named stakeholder or the TBD placeholder
- Task scheduling dates are optional — their absence does not affect task
  validity, visibility in queries, or eligibility for any project operation
- When no project schema is supplied, task behavior is unchanged for any
  project that has no task instances

## Vocabulary Dependency

- **Vocabulary owner:** `project_schema` module
- **Vocabulary consumer:** `task_model` module
- **Vocabulary facts relied upon:** marker-to-status mapping defined in
  `blockTypes`; canonical task type identity; the `assignedTo` link concept
  defining the owner relationship between a task and a stakeholder

## Scope Boundary

This feature does NOT:
- Evaluate whether a deadline has passed or enforce scheduling constraints
- Validate that the specified owner has any particular status, role, or
  availability — ownership is recorded, not enforced
- Cascade ownership changes to other tasks when an owner is changed or
  removed
- Sync task subtasks (nested sub-bullets under a task block in Logseq) —
  subtask support is a future refinement
- Cascade-delete child tasks when a parent item is removed from the record
- Change the schemas of any existing events emitted by logseq_export,
  logseq_sync, priority_view, item_status, item_links, or ontology_suggest

---

status: APPROVED
feature_id: task_model
approved_by: human
approved_at: 2026-06-07
derived_contracts: contracts/task_model_contract.md

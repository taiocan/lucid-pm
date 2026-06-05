# Intent: task_model

`task_model` exists to let the PM record and track individual task instances
within the project record, so that the task vocabulary defined by the project
schema becomes operational and task-level work is visible to all existing
project queries and analyses.

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

## Stable Guarantees

- A task instance is a first-class participant in the project record —
  it is reachable by any query or operation that applies to project record
  items, unless that feature explicitly excludes it in its own contract
- A task instance in the project record is visible to all existing project-
  record queries and analyses — the absence of a task from any query is
  never the result of a type mismatch between task instances and the
  query layer
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
- When no project schema is supplied, task behavior is unchanged for any
  project that has no task instances

## Vocabulary Dependency

- **Vocabulary owner:** `project_schema` module
- **Vocabulary consumer:** `task_model` module
- **Vocabulary facts relied upon:** marker-to-status mapping defined in
  `blockTypes` (maps task marker representations to domain status concepts);
  canonical task type identity; any property definitions associated with
  the task block type

## Scope Boundary

This feature does NOT:
- Provide recurrence, scheduling, due-date management, calendar integration,
  or deadline evaluation
- Sync task subtasks (nested sub-bullets under a task block in Logseq) —
  subtask support is a future refinement
- Cascade-delete child tasks when a parent item is removed from the record
- Enforce assignment validity or ownership workflows
- Change the schemas of any existing events emitted by logseq_export,
  logseq_sync, priority_view, item_status, item_links, or ontology_suggest

---

status: APPROVED
feature_id: task_model
approved_by: human
approved_at: 2026-06-04
derived_contracts: contracts/task_model_contract.md

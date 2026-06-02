# F12: `task_model` — Task Persistence and Lifecycle Sync

**Tier**: 7 — Task Layer
**Depends on**: F11 (project_schema), F3 (logseq_sync), F7 (item_links)
**Event spine impact**: New schema (new events for task lifecycle)
**Status**: BACKLOG

---

**Why F11 deferred this**

F11 introduced `blockTypes` in `project-schema.yaml` as a vocabulary definition:

```yaml
blockTypes:
  Task:
    markers:
      TODO: active
      DOING: active
      NOW: active
      WAITING: waiting
      LATER: waiting
      DONE: done
    uses: [scheduled, deadline]
```

F11 drew an explicit scope boundary: it defines what Task blocks look like and how their markers map to domain statuses, but it does **not** persist, create, synchronize, or manage task instances. The marker mapping is a dead configuration key in F11 — no task records exist in the event log for it to apply to. Every feature that queries status (priority_view, item_status, ontology_suggest) has no task instances to normalize. The Logseq export format shows Task blocks nested under WorkPackage pages (in F11's alignment example), but no export code renders them — there is nothing to render.

`logseq_export_schema_integration` also calls this out explicitly: *"Render Task block items within pages — task instances are not persisted until F12 (task_model)."*

F12 is the persistence and sync layer that makes the F11 vocabulary operational.

---

**Intent sketch**

Today there is no way to create, track, or synchronize individual task instances in LucidPM. The `blockTypes.Task` schema vocabulary exists but has no backing records. Task blocks that a PM writes in Logseq under a WorkPackage page are invisible to all LucidPM queries.

`task_model` closes this gap:
- Task instances are persisted in the event log as first-class items with their own `item_id`
- Tasks can be created via CLI and via Logseq sync (reading task blocks from exported pages)
- Task marker changes in Logseq (TODO → DONE) are detected during sync and recorded as status events
- All status-based queries (priority_view, item_status, ontology_suggest) apply the `blockTypes.Task.markers` mapping to normalize task markers to domain statuses
- Exported WorkPackage pages include their Task blocks, rendered as native Logseq task lines

**Architectural split inherited from F11:**
- F11 = vocabulary (what a Task is, what its markers mean, what properties it has)
- F12 = persistence + sync (recording task instances, reading marker state from Logseq, normalizing at query time)

---

**Key behaviors**

*Task persistence*

- A task instance is a record in the event log with a `task_id` (UUID), a parent `item_id` (the WorkPackage or Issue it belongs to), a marker state (e.g., `TODO`), optional `scheduled` and `deadline` dates, an optional description, and an optional assignee reference
- Tasks are `blockTypes` entities; they are never exported as Logseq pages — they render as task block lines under their parent's page

*Task creation — CLI*

```
lucid task add --parent <item_id> --marker TODO --description "Prepare design docs" [--assignee <item_id>] [--deadline <date>]
```

*Task sync — Logseq → event log*

- Logseq sync reads Task block lines from exported WorkPackage/Issue pages
- Each task block carries its `task-id:` inline ref (injected by the export) for traceability
- Marker changes (TODO → DOING, DOING → DONE, etc.) are recorded as `TaskMarkerUpdated` events
- New task blocks added in Logseq (no `task-id:`) are created as new task instances on sync
- Deleted task blocks trigger a `TaskDeleted` event

*Marker normalization at query time*

- When any command (priority_view, item_status, ontology_suggest) evaluates a task's effective status, it applies `blockTypes.Task.markers` from the active schema: marker → domain status
- Example: task with marker `DOING` → effective status `active`; marker `DONE` → effective status `done`
- This is the marker mapping F11 defined but could not use — F12 is the first consumer

*Logseq export — Task blocks under pages*

- WorkPackage and Issue pages include a `- Tasks` section listing their child task blocks
- Each task renders as: `- <MARKER> <description> [[<assignee-slug>]]` with `SCHEDULED:` / `DEADLINE:` drawers if set
- A `task-id: <uuid>` inline comment is injected for sync back-reference (invisible in Logseq reading mode)

---

**New event schema (indicative — resolved in Stage 3)**

| Event | Category | Emitted when |
|---|---|---|
| `TaskCreated` | BEHAVIORAL | A task instance is created (CLI or sync) |
| `TaskMarkerUpdated` | BEHAVIORAL | A task's marker changes (sync or CLI) |
| `TaskPropertyUpdated` | BEHAVIORAL | A task's scheduled/deadline/description changes |
| `TaskDeleted` | BEHAVIORAL | A task block is removed from Logseq and sync records its removal |
| `TaskSyncCompleted` | BEHAVIORAL | A sync run processing task blocks completes |
| `TaskCreationFailed` | FAILURE | Task creation fails (invalid parent, schema error, etc.) |

---

**Integration with existing modules**

| Module | Impact |
|---|---|
| `logseq_export` | Add Task block rendering under parent pages (new `- Tasks` section) |
| `logseq_sync` | Add task block detection, marker-change sync, new-task creation from Logseq |
| `priority_view` | Apply marker normalization when evaluating task effective status |
| `item_status` | `task get-status` applies marker normalization; `task set-marker` records TaskMarkerUpdated |
| `item_links` | Tasks can appear as link sources/targets per schema `relations` |
| `ontology_suggest` | Task instances included in AI analysis; marker status used for enrichment proposals |

---

**Scope boundary**

F12 does NOT:
- Introduce recurrence, scheduling rules, or automated task generation
- Sync task subtasks (Logseq nested sub-bullets under a task) — subtask support is a future refinement
- Validate assignee references against Stakeholder records (informational in v1)
- Provide a task query CLI beyond what priority_view and item_status already offer

---

**Open design questions for Stage 1**

1. Does task `item_id` share the same UUID namespace as page-type items, or does it use a separate `task_id` field? Sharing the namespace means task items are first-class participants in `item_links` without any extra wiring; a separate namespace keeps page and block types clearly separated.
2. For new task blocks added directly in Logseq (no `task-id:` present), what is the canonical creation trigger — does sync create them, or does a separate `import` step exist?
3. When a parent WorkPackage is deleted from the project record (out of scope for now, but possible in future), are its child tasks orphaned or cascade-deleted?
4. Does the `task-id:` inline ref injected into Logseq pages need to survive round-trip editing without corruption (a common Logseq block-property fragility)?

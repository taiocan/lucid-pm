# Intent: item_status_schema_driven_vocabulary

`item_status_schema_driven_vocabulary` exists to let the PM define the valid
lifecycle states for each entity type — including custom types — through the
project vocabulary schema, so that status validation and task-marker resolution
always reflect the active schema rather than a frozen code-level table.

Specifically:
- PM can assign a status to a custom entity type defined in the project schema
  and have that assignment accepted as valid
- PM can query the effective status of a task-type item and receive the status
  that corresponds to the item's current Logseq task marker when no explicit
  status update has been recorded
- PM can change the status vocabulary for an entity type in the schema and have
  `set-status` validate against the updated vocabulary on the next command,
  without any code change
- PM can rely on any status or priority command — including read-only queries —
  being aborted when the active schema cannot be loaded, because accurate effective
  status resolution requires a successfully loaded vocabulary

## Stable Guarantees

- Status validation for every entity type uses only the vocabulary defined in
  the active schema — no hardcoded status table is ever consulted
- A schema load failure prevents any status or priority command — including
  read-only queries — from completing; all commands are equally gated because
  accurate effective status resolution, including marker mapping, requires a
  successfully loaded vocabulary; priority commands are gated for the same reason
  even though priority values themselves are not schema-defined in this release
- When no project schema is supplied, the embedded default vocabulary provides
  the same status vocabulary as the legacy hardcoded implementation, so no
  existing project is affected
- An entity type defined in the schema without any status vocabulary entries has
  an empty valid status set — any `set-status` attempt on an item of that type
  is rejected as InvalidStatusForType
- When a task-type item carries a Logseq task marker and no explicit
  `ItemStatusUpdated` event exists, the effective status is the value produced
  by the task-marker mapping in the active vocabulary
- A task marker not present in the active vocabulary's task-marker mapping is
  treated as if no marker-derived status exists; the fallback proceeds to the
  proposed-value rule as normal
- An explicit `ItemStatusUpdated` event always takes precedence over a
  marker-derived status
- Historical status values stored in the event log are readable even when no
  longer present in the active vocabulary; when an item's recorded status is no
  longer recognized by the active vocabulary, the condition is surfaced as a
  non-failure observational signal — the query completes and the condition is
  never silently suppressed
- The proposed-value fallback rule is unchanged: proposed status is used as the
  effective status when neither an explicit update nor a marker-derived value is
  available
- Effective status resolution is deterministic with respect to (event log, item
  content, active vocabulary) — given the same inputs, the same effective status
  is always produced

## Scope Boundary

This feature does NOT:
- Change the event spine — `ItemStatusUpdated`, `ItemPriorityUpdated`, and all
  failure events are unchanged
- Make priority vocabulary (high/medium/low) schema-driven — that is deferred
- Enforce status transition ordering
- Migrate existing event log entries when a status vocabulary changes
- Affect how statuses are displayed or validated in Logseq sync — that is R9

---

status: APPROVED
feature_id: item_status_schema_driven_vocabulary
approved_by: human
approved_at: 2026-06-01
derived_contracts: contracts/item_status_schema_driven_vocabulary_contract.md

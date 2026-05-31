# F11: `project_schema`

**Tier**: 1 — Foundation
**Depends on**: project_state, item_status, logseq_export, logseq_sync, item_links
**Event spine impact**: None — schema is a configuration artifact loaded at runtime, not appended to the event log. Additive rendering/validation changes to 5 existing modules.
**Status**: COMPLETE

**Architectural boundary**
- F11 = vocabulary + rendering + validation (configuration layer, zero new events)
- F12 = task persistence + task synchronization (future feature)

---

**Intent sketch**

Today every entity type, valid status value, relation type, and Logseq rendering label is hardcoded in Rust across multiple modules. A PM who wants `Workstream` instead of `WorkPackage`, or a `Budget` property, or a different label, must change source code.

`project_schema` introduces a per-project `project-schema.yaml` at the project root that defines the entity vocabulary (`pageTypes` for Logseq pages, `blockTypes` for native Logseq task blocks), shared property definitions, relation types with source/target metadata, marker normalization, and renderer-specific mappings for labels and property names. All downstream modules derive their runtime behavior from this schema at startup — no new events are emitted.

A global default at `~/.lucidpm/default-schema.yaml` provides immediate usability; per-project schemas can extend or override it.

Architecture: Schema → Domain Graph → Renderer → Logseq pages + native Logseq tasks. LucidPM understands tasks; Logseq executes them.

---

**Default schema structure**

```yaml
schemaVersion: 1
extends: ~/.lucidpm/default-schema.yaml   # optional

# Shared property definitions — referenced by name in pageTypes/blockTypes
properties:
  status:
    type: enum
  priority:
    type: enum
    values: [low, medium, high]
  deadline:
    type: date        # stored as Option<NaiveDate> internally; None → "TBD" in Logseq
  scheduled:
    type: date

# Global status vocabulary as a map — extensible for metadata (color, display name later)
statuses:
  active:
  waiting:
  done:
  cancelled:

# Page-level entities → each becomes a Logseq page
pageTypes:
  Stakeholder:
    uses: [status]
  Milestone:
    uses: [status, deadline]
  WorkPackage:
    uses: [status, priority, deadline]
    allowedStatuses: [active, waiting, done, cancelled]   # optional per-type restriction
    aliases: []   # e.g. aliases: [Feature] — items stored as "Feature" match here
  Issue:
    uses: [status, deadline]
  Risk:
    uses: [status, deadline]
  # Rename example — uncomment to rename WorkPackage → Workstream for this project:
  # Workstream:
  #   uses: [status, priority, deadline]
  #   aliases:
  #     - WorkPackage

# Block-level entities → render as native Logseq task markers (not pages)
# markers map to normalized domain statuses for cross-type queries
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

# Relation types — domain identifiers with source/target metadata (informational in v1)
relations:
  assignedTo:
    source: [WorkPackage, Task, Issue]
    target: [Stakeholder]
  blocks:
    source: [WorkPackage, Task, Issue]
    target: [WorkPackage, Task, Milestone]
  affects:
    source: [Risk, Issue]
    target: [WorkPackage, Task, Milestone, Stakeholder]
  relatedTo:
    source: [any]
    target: [any]

# Renderer config — labels and property name mappings separated from domain
renderers:
  logseq:
    relations:
      assignedTo:
        forwardLabel: "Assigned To"
        inverseLabel: "Owns"
      blocks:
        forwardLabel: "Blocks"
        inverseLabel: "Blocked By"
      affects:
        forwardLabel: "Affects"
        inverseLabel: "Affected By"
      relatedTo:
        forwardLabel: "Related To"
        inverseLabel: "Related To"
    properties:
      assignedTo: assigned-to
      deadline: deadline
      scheduled: scheduled
      status: status
      priority: priority
```

---

**Key behaviors**

*Schema location — aligned with multi_project isolation*

- Active schema: `<project-dir>/project-schema.yaml`
- Global default: `~/.lucidpm/default-schema.yaml` (installed by LucidPM installer)
- `extends:` merge semantics: **maps merge recursively; lists replace completely; scalars replace completely**
  - Example: project `statuses: { active:, cancelled: }` replaces the default `statuses` entirely — result is `active` + `cancelled` only
- If no project schema and no global default: abort with clear error

*Schema reload behavior*

- Each `lucid` command reads `project-schema.yaml` from disk at startup
- No long-lived schema cache in v1
- Editing the file takes effect on the next command execution

*Inline versioning — no events*

- `schemaVersion: int` inside the YAML is the version record
- No events emitted on schema load or schema change — schema is configuration, not data
- Migration engine deferred to a future refinement

*Alias resolution — one-way only*

- `aliases:` maps old type names → canonical type name, never the reverse
- Items stored as `WorkPackage` are interpreted as `Workstream` at runtime
- Items stored as `Workstream` are never interpreted as `WorkPackage`
- Alias resolution is a runtime interpretation, not recorded in the event log

*Status transitions — fully open*

- Any → any transition within the global `statuses` map is valid
- `allowedStatuses:` restricts valid values per type, not transition paths
- No forbidden transition enforcement anywhere

*Deadline as `Option<NaiveDate>`*

- Internal Rust model: `deadline: Option<NaiveDate>` — no "TBD" string in memory
- `None` renders as `deadline:: TBD` on Logseq export
- Populated from three tagged sources via existing event infrastructure: `extraction`, `manual`, `logseq_sync`

*Marker normalization*

- `markers:` maps each Logseq keyword to a domain status
- Enables cross-type queries: "active work" = WorkPackage(status=active) + Task(marker=TODO|DOING|NOW)
- Logseq renders the literal marker unchanged; normalization is read-only

*Task instances — not persisted in F11*

F11 defines how Task blocks are rendered when task data is available from upstream modules. F11 does **not** introduce task persistence, task creation, task synchronization, or task lifecycle management. Those are F12 (`task_model`). F11 is the vocabulary and rendering definition; F12 is the persistence and sync layer.

*Renderer property and label mappings*

- `renderers.logseq.properties` maps domain property names to Logseq property keys
- `renderers.logseq.relations` provides forward and inverse labels per relation
- Changing a label or key takes effect on the next export — no Rust code change required

---

**Logseq alignment**

WorkPackage page:
```
type:: work-package
status:: active
priority:: high
deadline:: 2026-06-30
tags:: work-package

- item-id: wp-001

- Blocks
    - [[stakeholder-sign-off]]

- Blocked By
    - [[vendor-delivery]]

- Tasks
    - TODO Prepare design documents [[Maria]]
        DEADLINE: <2026-06-06 Sat>
        - DONE Review existing specs
        - TODO Draft new architecture
    - DOING Implement auth module [[Ivan]]
        SCHEDULED: <2026-05-30 Fri>
```

Risk page (absent deadline):
```
type:: risk
status:: active
deadline:: TBD
tags:: risk

- item-id: risk-001

- Affects
    - [[tower-design]]
```

Task blocks use Logseq native `SCHEDULED:` / `DEADLINE:` drawer format so agenda, journal, and recurring task integrations work natively. Assignee uses inline `[[PageRef]]` by default; future renderer option `assigneeRenderMode: property` can switch to `assigned-to::` style.

---

**Integration with existing modules**

| Module | Change | What changes |
|---|---|---|
| `pm_structuring` | Additive | Entity type vocabulary from schema `pageTypes`; LLM prompt includes schema entity names; `ItemsExtracted` gains nullable `deadline` per item |
| `project_state` | Additive | Entity type validation at incorporation reads schema `pageTypes` |
| `item_status` | Additive | Valid status values from schema `statuses` + per-type `allowedStatuses`; any transition accepted |
| `logseq_export` | Additive | `deadline::` (or TBD) on all pages; inverse labels from schema renderer; property keys from renderer mappings; Task blocks rendered under WorkPackage pages |
| `logseq_sync` | Additive | `deadline::` parsed and synced back; status transitions unrestricted; Task block changes not synced (F12 scope) |
| `item_links` | Additive | Valid source/target types from schema `relations` metadata; no hardcoded type matrix |

---

**Event spine**

None. F11 adds zero new events. Schema is a configuration artifact read at startup. Existing events (`ItemStatusUpdated`, `ItemsExtracted`, `ItemsIncorporated`, etc.) remain unchanged. Deadline tracking uses existing event infrastructure; additive fields on existing events resolved during F11 implementation.

---

**Schema validation rules** (enforced at load time)

- `pageTypes` names must be unique after alias expansion
- `blockTypes` names must be unique
- Aliases may not collide with another type's canonical name or alias
- Relation names must be unique
- Renderer mappings may only reference relation and property names defined in the schema
- `uses:` entries must reference property names in the `properties:` block
- Unknown entity type names in `relations.source/target` are schema errors, not warnings

**Failure modes**

- `SchemaFileNotFound` — no project schema and no global default; abort with message
- `SchemaParseError` — YAML parse error or validation rule violated
- `SchemaEntityTypeUnknown` — item_type from event log not in schema (no alias match); warn and skip
- `DeadlineParseError` — deadline string from Logseq not ISO date; treat as absent

---

**Deferred to future features/refinements**

- F12: `task_model` — Task creation, update, deletion events; full task lifecycle sync
- R-future: `schema_migration` — schema version events, migration engine
- R-future: `schema_views` — `views:` block generating Logseq queries automatically
- Status metadata (color, display name) in the `statuses:` map
- Relation cardinality enforcement (source/target metadata informational in v1)
- Task `assigneeRenderMode:` renderer option

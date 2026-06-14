# Behavioral Contract: logseq_export

<!--
DERIVED FROM: intents/logseq_export.md
-->

## Scenarios

### Happy Path: Successful Export

```gherkin
Given the project record contains one or more items with statuses and priorities
And a valid Logseq output directory is accessible
When the PM triggers an export
Then an ExportCompleted event is emitted
And every item in the project record is present as a page in the Logseq output
And each page is named by a human-readable slug derived from the item's description
And each exported page contains the item's current status and priority as queryable properties
And each exported page carries a tags property set to the item's type
And each exported page contains the item's UUID as a plain-text bullet for traceability
And any pages previously present in the output directory that do not correspond
  to a current item are deleted
And the project event log is unchanged
```

### Happy Path: Task Block with Owner and Dates

```gherkin
Given a task T exists in the project record with:
  - current marker: DOING
  - owner: stakeholder S whose Logseq page slug is "maria"
  - scheduled_date: "2026-06-15"
  - deadline: "2026-06-30"
And T's parent item P has been exported as a page
When the PM triggers an export
Then T appears as an indented child block within P's page
And T's block line is: `- DOING T-description [[maria]]`
And T's block contains a :PROPERTIES: drawer with `:task-id: T-uuid`
And T's block contains `SCHEDULED: <2026-06-15 Sun>`
And T's block contains `DEADLINE: <2026-06-30 Tue>`
```

### Happy Path: Task Block with TBD Placeholder Owner

```gherkin
Given a task T exists in the project record whose owner is the TBD placeholder
And T's parent item P has been exported as a page
When the PM triggers an export
Then T appears as an indented child block within P's page
And T's block line is: `- TODO T-description`
And T's block line contains no owner wiki-link
And T's block contains a :PROPERTIES: drawer with `:task-id: T-uuid`
```

### Happy Path: Task Block with No Dates

```gherkin
Given a task T exists in the project record with a named stakeholder owner (slug: "maria")
And T has no scheduled_date and no deadline
When the PM triggers an export
Then T's block line is: `- TODO T-description [[maria]]`
And T's block contains no SCHEDULED line
And T's block contains no DEADLINE line
```

### Boundary: TBD Placeholder Owner Does Not Suppress Other Task Block Content

```gherkin
Given a task T exists in the project record whose owner is the TBD placeholder
And T's parent item P has been exported as a page
And T has scheduled_date "2026-06-15" and deadline "2026-06-30"
When the PM triggers an export
Then T appears as an indented child block within P's page
And T's block line is: `- TODO T-description`
And T's block contains a :PROPERTIES: drawer with `:task-id: T-uuid`
And T's block contains `SCHEDULED: <2026-06-15 Sun>`
And T's block contains `DEADLINE: <2026-06-30 Tue>`
```

### Falsification: TBD Placeholder Owner Produces No Wiki-Link of Any Kind

```gherkin
Given a task T exists in the project record whose owner is the TBD placeholder
And T's parent item P has been exported as a page
When the PM triggers an export
Then T's block line contains no `[[...]]` pattern of any kind

Falsifies: an implementation that always emits some owner wiki-link for every task
           (e.g. [[TBD]], [[unassigned]], [[none]]) regardless of owner concept —
           such an implementation passes the named-owner scenario but fails here
```

### Happy Path: Work Package Relations as Page Properties

```gherkin
Given a work package item W exists with:
  - an assigned_to link to stakeholder S (slug: "maria")
  - a blocks link (outgoing) to work package X (slug: "infrastructure-setup")
  - a blocks link (incoming) from work package Y (slug: "mobile-backend")
When the PM triggers an export
Then W's page header contains `assigned-to:: [[maria]]`
And W's page header contains `blocking:: [[infrastructure-setup]]`
And W's page header contains `blocked-by:: [[mobile-backend]]`
And these relations do NOT appear as content section bullets under W's page
```

### Happy Path: Work Package with Multiple Targets on One Relation

```gherkin
Given a work package W has blocks (incoming) links from items Y and Z
When the PM triggers an export
Then W's page header contains `blocked-by:: [[Y-slug]] [[Z-slug]]`
And all targets appear on the same property line, space-separated
```

### Happy Path: Dashboard Created on Fresh Export

```gherkin
Given the project record contains one or more items with types recognized by the loaded schema
And the output directory does not contain a Dashboard.md
When the PM triggers a successful export
Then Dashboard.md is written to the output directory
And Dashboard.md contains a query section for each recognized operational type present
  in the loaded schema (Milestone, WorkPackage, Risk, Stakeholder, Task)
And each query section uses the same type identifier that appears in the `type::` page
  property of items of that type — not a hardcoded string
```

### Happy Path: Dashboard Preserves Existing Customization

```gherkin
Given a Dashboard.md already exists in the output directory with custom content
When the PM triggers an export
Then the existing Dashboard.md is not modified
And the custom content is unchanged after export
```

### Happy Path: Dashboard Section Omitted for Missing Schema Type

```gherkin
Given the loaded schema does not declare a Milestone-equivalent type
And the output directory does not contain a Dashboard.md
When the PM triggers a successful export
Then Dashboard.md is written to the output directory
And Dashboard.md does not contain a Pending Milestones query section or any
  placeholder for a missing type
```

### Boundary: No Dashboard Written When No Recognized Operational Types

```gherkin
Given the loaded schema contains no types matching any of the five operational types
  (Milestone, WorkPackage, Risk, Stakeholder, Task)
And the output directory does not contain a Dashboard.md
When the PM triggers a successful export
Then no Dashboard.md is written to the output directory
```

### Falsification: Dashboard Query Type String Derived from Schema, Not Hardcoded

```gherkin
Given the loaded schema declares a pageType with canonical key "Workstream"
  (not "WorkPackage") for the work-package equivalent
And the output directory does not contain a Dashboard.md
When the PM triggers a successful export
Then Dashboard.md contains a query section referencing "workstream"
And Dashboard.md does not contain a query section referencing "work-package"
  or any other hardcoded work-package type string

Falsifies: implementation that hardcodes "work-package" in the Active Work Packages
           query regardless of the schema's canonical type key
```

### Happy Path: Idempotent Re-export

```gherkin
Given a successful export has already been performed
And the project record has not changed since that export
When the PM triggers a second export
Then an ExportCompleted event is emitted
And the output pages are identical in content to the previous export
And the project event log is unchanged
```

### Failure Path 1: EmptyProjectRecord

```gherkin
Given the project record contains no items
When the PM triggers an export
Then an ExportFailedEmptyRecord event is emitted
And no Logseq pages are written
And the project event log is unchanged
```

### Failure Path 2: OutputDirectoryNotAccessible

```gherkin
Given the project record contains one or more items
And the Logseq output directory is not accessible (missing, no write permission)
When the PM triggers an export
Then an ExportFailedOutputUnavailable event is emitted
And no partial Logseq pages are written
And the project event log is unchanged
```

### Failure Path 3: ProjectRecordUnreadable

```gherkin
Given the project record source is corrupted or unreadable
When the PM triggers an export
Then an ExportFailedRecordUnreadable event is emitted
And no Logseq pages are written
And the project event log is unchanged
```

## Invariants

- Existing events in the project event log are never modified or deleted by an export operation
- Exported pages contain only information present in the project record — nothing is invented or inferred
- Every item in the project record appears in the exported output on a successful export
- An export with the same project state always produces the same page content (idempotent)
- Each page filename is a URL-safe slug derived from the item's description: lowercase,
  non-alphanumeric characters replaced with hyphens, consecutive hyphens collapsed,
  leading/trailing hyphens stripped, max 120 characters truncated at a word boundary;
  slug collisions resolved by appending -2, -3, etc. in item order
- The item UUID is preserved in every page as a plain-text bullet (`- item-id: <uuid>`) —
  it is not a Logseq page property and does not create a Logseq index page
- Relationship links in exported pages reference target items by their slug name, not by UUID,
  making backlinks human-readable in Logseq's graph view
- Relationship sections use Logseq outline indentation (indented child bullets) rather than
  markdown headers, enabling collapsing and block embedding in Logseq
- Pages in the output directory not corresponding to any current item are deleted on each export
- Every exported task block carries a `:PROPERTIES:` drawer containing `:task-id: <uuid>` —
  this identity is never omitted, enabling logseq_sync to match blocks back to task records
- Work package relations (assigned_to, outgoing blocks, incoming blocks) render as page
  properties in the page header; non-work-package items render relations as content sections
- The SCHEDULED and DEADLINE lines in task blocks use the format `<YYYY-MM-DD DDD>` where
  DDD is the 3-letter English day abbreviation (e.g. `<2026-06-15 Mon>`); these lines are
  omitted entirely when the task has no scheduled_date or deadline in the project record
- A task block line includes an owner wiki-link (`[[owner-page-slug]]`) if and only if
  the task's owner is a named stakeholder; a task whose owner is the TBD placeholder
  carries no owner reference of any kind on its block line
- Dashboard.md in the output directory is never overwritten by export — if the file
  existed before export it is left unchanged regardless of project state or schema changes
- Dashboard.md is generated only when at least one recognized operational type is present
  in the loaded schema; if no operational types are recognized, no Dashboard.md is written
- Each query section in the generated Dashboard uses the same type identifier that appears
  in the `type::` page property of items of that type — the string is derived from the
  schema canonical key, never hardcoded

## Vocabulary Dependency (R16)

- **Vocabulary owner:** task_model (F12) defines the owner concept: every task is associated
  with exactly one owner — either a named stakeholder or the TBD placeholder.
- **Vocabulary consumer:** this contract reasons about that distinction to determine task block
  rendering.
- **Concepts relied upon:** named stakeholder owner; TBD placeholder.

**Concept Dependency Invariant (governing):**
The rendering outcome for owner display — wiki-link present or absent — depends only on
whether the task's owner resolves to a named stakeholder concept. The stored representation
of the owner in the project record does not affect the rendering outcome.

**Representation Ban (derived):**
No stored owner representation may appear directly as the rendered output. The wiki-link
slug is derived from the named stakeholder's identity in the project record. Owner
representations are inputs to concept resolution, not to rendering logic.

**Display invariant:**
When a named stakeholder owner is displayed, the reference shown is the owner's canonical
Logseq page slug.

## Invariant Falsification Scenarios

| Invariant | Fixture | Wrong Assumption Named | Test ID |
|---|---|---|---|
| Owner wiki-link present iff named stakeholder | Task with named owner → assert `[[owner-slug]]` present on block line | Implementation omits wiki-link for all tasks, treating all owners as unassigned | `test_task_block_line_has_marker_description_owner` |
| No owner reference for TBD placeholder | Task with TBD owner → assert no `[[...]]` pattern on block line | Implementation always emits some owner wiki-link (e.g. `[[TBD]]`) for every task | `test_tbd_owner_no_wiki_link_pattern_of_any_kind` |
| Concept governs rendering, not representation | Task stored with TBD → no wiki-link; named owner stored as slug → wiki-link; if owner sentinel changes representation, observable output is unchanged | Implementation checks a specific string representation rather than resolving the owner concept through task_model's definitions | `test_task_block_tbd_owner_omits_wiki_link` + `test_task_block_line_has_marker_description_owner` |
| Removing owner reference does not suppress other task block content | TBD-owner task with dates → PROPERTIES drawer, SCHEDULED, and DEADLINE lines all present | Implementation simplifies TBD-owner block by removing PROPERTIES drawer alongside the owner reference | `test_tbd_owner_does_not_suppress_properties_drawer_and_dates` |
| Dashboard query type string derived from schema (R15) | Schema canonical key "Workstream" → Dashboard contains "workstream", not "work-package" | Implementation hardcodes type strings in Dashboard queries | |
| Dashboard not overwritten (R15) | Dashboard.md pre-exists with custom content → after export, content unchanged | Implementation always regenerates Dashboard.md | |

## Preconditions

- A project record exists and is readable
- A target Logseq output directory has been designated
- The project record contains at least one item (otherwise EmptyProjectRecord failure applies)

## Postconditions

- Every item from the project record exists as a slug-named page in the Logseq output directory
- Each page reflects the item's current status and priority
- Each page contains navigable links to related items expressed as human-readable page references
- Pages for items no longer in the current export set have been removed from the output directory
- The project event log is in the same state as before the export
- If the output directory did not contain Dashboard.md before export, and the loaded schema
  contains at least one recognized operational type, Dashboard.md exists in the output directory
  after a successful export

## Runtime Artifacts

| Artifact | Path | Lifecycle |
|---|---|---|
| Logseq item pages | `<logseq_output_dir>/pages/<description-slug>.md` | Created or overwritten on each export; pages not in current export set are deleted |
| Dashboard.md | `<logseq_output_dir>/pages/Dashboard.md` | Created on first export if absent and schema has recognized operational types; never overwritten |
| Export event record | `events/runtime_events.jsonl` | Appended on each export attempt |

### Page format

**Non-work-package items** use the following structure:

```
type:: <item_type>
status:: <status or "not-set">
priority:: <priority or "not-set">
tags:: <item_type>

- item-id: <uuid>

- <Relationship Label>
    - [[<target-description-slug>]]
```

**Work package items** use the following structure (relations as page properties):

```
type:: work package
status:: <status or "not-set">
priority:: <priority or "not-set">
assigned-to:: [[<stakeholder-slug>]]
blocking:: [[<slug1>]] [[<slug2>]]
blocked-by:: [[<slug>]]
tags:: work package

- item-id: <uuid>
```

Work package relation properties are omitted when empty. The property names for blocks
relations are always `blocking::` (outgoing) and `blocked-by::` (incoming).

**Task blocks** are indented child blocks within their parent item's page.

Named stakeholder owner:
```
    - <MARKER> <description> [[<owner-slug>]]
      :PROPERTIES:
      :task-id: <task-uuid>
      :END:
      SCHEDULED: <YYYY-MM-DD DDD>
      DEADLINE: <YYYY-MM-DD DDD>
```

TBD placeholder owner:
```
    - <MARKER> <description>
      :PROPERTIES:
      :task-id: <task-uuid>
      :END:
      SCHEDULED: <YYYY-MM-DD DDD>
      DEADLINE: <YYYY-MM-DD DDD>
```

`type::`, `status::`, `priority::`, `tags::`, `assigned-to::`, `blocking::`, `blocked-by::` are
Logseq page properties (double-colon syntax).
`item-id:` is plain text (single-colon bullet) and does not create a Logseq index page.
`:PROPERTIES:` ... `:END:` is an Org-mode drawer rendered by Logseq as collapsed metadata.
Relationship sections on non-work-package items are omitted entirely when an item has no
active links of that type.
SCHEDULED/DEADLINE lines on task blocks are omitted when the task has no stored dates.

## Failure Classifications

| Failure Name | Trigger Condition | Observable Signal |
|---|---|---|
| EmptyProjectRecord | Project record contains no items at export time | `ExportFailedEmptyRecord` event emitted |
| OutputDirectoryNotAccessible | Target Logseq directory is missing or write-protected | `ExportFailedOutputUnavailable` event emitted |
| ProjectRecordUnreadable | Project record source is corrupted or cannot be parsed | `ExportFailedRecordUnreadable` event emitted |

---

<!-- METADATA -->
status: APPROVED
feature_id: logseq_export
approved_by: human
approved_at: 2026-05-25
refined_at: 2026-06-14 (R15: schema-driven Dashboard.md generation)
refinement_log: intents/logseq_export_refinements.md
derived_from_intent: intents/logseq_export.md
derived_event_schema: events/logseq_export_schema.md

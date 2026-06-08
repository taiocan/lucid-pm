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

### Happy Path: Task Block with TBD Owner

```gherkin
Given a task T exists in the project record with owner_id "TBD"
When the PM triggers an export
Then T's block line is: `- MARKER T-description [[TBD]]`
And no stakeholder page for TBD is created by the export
```

### Happy Path: Task Block with No Dates

```gherkin
Given a task T exists in the project record with no scheduled_date and no deadline
When the PM triggers an export
Then T's block line is: `- MARKER T-description [[owner-slug]]`
And T's block contains no SCHEDULED line
And T's block contains no DEADLINE line
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
- The owner reference in a task block line is `[[owner-page-slug]]`; for TBD-owned tasks
  the literal `[[TBD]]` is used; no TBD page is created

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

## Runtime Artifacts

| Artifact | Path | Lifecycle |
|---|---|---|
| Logseq item pages | `<logseq_output_dir>/pages/<description-slug>.md` | Created or overwritten on each export; pages not in current export set are deleted |
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

**Task blocks** are indented child blocks within their parent item's page:

```
    - <MARKER> <description> [[<owner-slug>]]
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
refined_at: 2026-06-08
refinement_log: intents/logseq_export_refinements.md
derived_from_intent: intents/logseq_export.md
derived_event_schema: events/logseq_export_schema.md

# Behavioral Contract: F16 — task_extraction

<!--
DERIVED FROM: intents/F16_task_extraction.md
AMENDS: contracts/pm_structuring_contract.md — extends extraction to produce
task records with parent_item_id when WP attribution is unambiguous.
All existing pm_structuring scenarios remain in force unchanged.
-->

## Definitions

**WP-attributed task** — an extracted item whose entity type resolves to the
canonical task block type concept in the active vocabulary AND whose extraction
context unambiguously identifies an existing WP in the project record as its parent.

**Unassigned task** — an extracted item whose entity type resolves to the canonical
task block type concept but whose WP attribution is absent or unresolvable.

**Unambiguous WP attribution** — the extraction source text exhibits one of the
following signals:
- *Structural hierarchy*: a WP heading (bold or heading-level line) followed
  immediately by indented task bullets — the heading identifies the WP; the
  indented bullets are its tasks.
- *Explicit name reference*: a task bullet directly names an existing WP from
  the project record by slug or description.

Attribution is NOT unambiguous when:
- The text is flat with no structural hierarchy and no explicit WP name reference.
- Multiple WPs match the reference and disambiguation is not possible.
- The referenced WP name does not match any item in the project record.

---

## Scenarios

### Happy Path 1: Structural hierarchy → tasks extracted with parent_item_id

```gherkin
Given the active schema declares a canonical task blockType
And the active schema declares a WP-equivalent pageType (resolved via alias "workpackage")
And the project record contains a WP item W with description "Platform Integration"
And the extraction text contains:
  **Platform Integration**
    - Set up CI pipeline
    - Write API documentation
When the PM runs extraction and confirms the extracted items
Then each task item in the extraction carries parent_item_id equal to W's item UUID
And each task item's item_type resolves to the canonical task block type
And each task item has an initial_marker equal to the schema's default active marker
And each task item has owner_id equal to the TBD placeholder
```

### Happy Path 2: Explicit WP name reference → task extracted with parent_item_id

```gherkin
Given the project record contains a WP item W whose description slug is "platform-integration"
And the extraction text contains a task bullet:
  "Set up CI pipeline — Platform Integration"
When the PM runs extraction and confirms the extracted items
Then the extracted task item carries parent_item_id equal to W's item UUID
```

### Happy Path 3: No WP context → task extracted as unassigned

```gherkin
Given the extraction text contains task descriptions with no structural WP hierarchy
  and no explicit WP name reference
When the PM runs extraction and confirms the extracted items
Then each task item's parent_item_id is absent (null)
And each task item appears in the project record as a standalone task record
And the export shows these tasks as orphan task blocks (visible in Dashboard
  "Open Tasks" query but not nested under any WP page)
```

### Happy Path 4: Named WP not in project record → task extracted as unassigned

```gherkin
Given the extraction text contains:
  **Future Feature Backlog**
    - Implement notifications
And no WP with description "Future Feature Backlog" exists in the project record
When the PM runs extraction and confirms the extracted items
Then the task "Implement notifications" is extracted with parent_item_id absent
And no new WP item is created for "Future Feature Backlog"
And the task appears as an unassigned task record
```

### Falsification: No automatic WP creation

```gherkin
Given the extraction text names a WP that does not exist in the project record
When the PM runs extraction
Then no new WP item is created in the project record
And the project record's WP count is unchanged after extraction

Falsifies: an implementation that creates WP items automatically when the
           extraction AI identifies a WP-like heading not already in the record
```

### Happy Path 5: WP-attributed task appears as nested block after export

```gherkin
Given a WP item W has been exported as a Logseq page
And a task T was extracted with parent_item_id equal to W's UUID
And T's item_type resolves to the canonical task blockType
When the PM runs export
Then T appears as an indented task block within W's Logseq page
And T is NOT written as a separate Logseq page
And T's block line includes T's initial_marker and description
And T's block contains a :PROPERTIES: drawer with :task-id: equal to T's item UUID
```

### Happy Path 6: Default marker and owner on extracted tasks

```gherkin
Given the active schema's canonical task blockType defines markers with at least
  one marker mapping to an active-equivalent status
When a task is extracted (regardless of WP attribution)
Then the task's initial_marker is the first active-equivalent marker from the schema
  (typically "TODO" when that is defined in the schema's blockType marker vocabulary)
And the task renders with the TBD placeholder as owner in the Logseq export
  (owner_id defaults to TBD via the export rendering fallback;
   owner is not set at extraction time and is absent from the ItemsExtracted payload)
```

### Happy Path 7: WP type and task type derived from schema

```gherkin
Given the active schema declares a pageType with canonical key "Workstream"
  aliased as "workpackage" and a blockType with canonical key "action_item"
And the extraction text attributes an action_item to an existing Workstream W
When extraction runs
Then the extracted task's item_type matches the canonical key for the action_item type
And the parent_item_id is set to W's UUID
And no hardcoded type names ("workpackage", "task", etc.) are used in the matching logic

Falsifies: an implementation that hardcodes "task" or "workpackage" in the WP
           resolution or task type detection logic
```

### Boundary: Ambiguous WP attribution → unassigned

```gherkin
Given the extraction text contains task bullets that could plausibly belong to
  multiple WPs (e.g., flat list with no hierarchy and task descriptions that
  partially match two WPs)
When extraction runs
Then the task is extracted as unassigned (parent_item_id absent)
And no parent assignment is made without unambiguous evidence

Boundary: ensures non-deterministic inference defaults to safe (unassigned), not guess
```

### Boundary: parent_item_id survives extraction confirmation

```gherkin
Given a task T is extracted with parent_item_id set to WP W's UUID
And the PM confirms T during the extraction review
When find_confirmed_items is called for the extraction session
Then the returned RecordedItem for T carries parent_item_id equal to W's UUID
And parent_item_id is NOT lost or reset to null during the confirmation flow
```

---

## Invariants

- `parent_item_id` on an extracted task always references an existing WP item UUID
  in the project record at extraction time — it is never invented or inferred from
  WP items that do not exist
- No WP items are created by extraction — the project record's WP count is
  unchanged by extraction regardless of what WP names appear in the source text
- All extracted task records receive an initial_marker derived from the schema's
  canonical task blockType marker vocabulary — the marker is never hardcoded
- All extracted tasks render with the TBD placeholder as owner in Logseq export —
  owner_id is not set during extraction (absent from ItemsExtracted payload);
  the export rendering fallback supplies TBD when owner_id is unset
- The WP type match uses the schema alias "workpackage" via `resolve_type` —
  a user who renames WorkPackage to Workstream (with alias "workpackage") sees
  correct WP attribution behavior without any code change
- `parent_item_id` is present in the `ItemsExtracted` event payload per extracted
  task item and is readable by `find_confirmed_items()` — it is not lost between
  event emission and record reading
- Unassigned task records (parent_item_id absent) are valid project record items —
  they require no further action and do not cause failures in export or state display

## Vocabulary Dependency

- **Vocabulary owner:** `project_schema` (F11) defines the canonical task blockType
  (via `canonical_task_block_type`) and WP-equivalent pageType (via alias "workpackage").
- **Vocabulary consumer:** this feature reads canonical task blockType key and
  default active marker from `blockTypes`; reads WP pageType canonical key from
  `pageTypes` via alias resolution.
- **Vocabulary owner:** `task_model` (F12) defines TBD placeholder, initial_marker
  semantics, and owner_id field on task records.
- **Concept dependency invariant:** WP resolution is governed by the schema alias
  "workpackage" — the canonical key of the WP-equivalent type flows from schema to
  extraction prompt to parent_item_id assignment without hardcoding.

## Preconditions

- The active schema declares at least one task blockType with a non-empty marker
  mapping (otherwise no task items can be produced)
- The active schema declares at least one WP-equivalent pageType with alias
  "workpackage" (otherwise no WP attribution can occur; tasks are extracted as
  unassigned)
- The project record is accessible at extraction time (for WP UUID resolution)

## Postconditions

- Every extracted task with unambiguous WP attribution has `parent_item_id` set
  to an existing WP UUID in the project record
- Every extracted task without unambiguous WP attribution has `parent_item_id` absent
- `parent_item_id` survives through `ExtractionConfirmed` and is returned by
  `find_confirmed_items()` with the full `RecordedItem`
- After export, tasks with `parent_item_id` appear as nested blocks under their
  parent WP's Logseq page

## Failure Classification

F16 introduces no new failure events. Existing pm_structuring failure paths
(EmptyInput, NoExtractableContent, PMRejectedExtraction, ApiRequestFailed,
SchemaInvalid, SchemaNotFound) apply unchanged.

---

<!-- METADATA -->
status: APPROVED
feature_id: F16
approved_by: human
approved_at:
derived_from_intent: intents/F16_task_extraction.md
amends: contracts/pm_structuring_contract.md

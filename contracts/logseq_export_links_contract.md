# Behavioral Contract: logseq_export_links
# F8 — logseq_export Stage 9 Refinement: Item Link Rendering

<!--
DERIVED FROM: intents/logseq_export_links.md
This contract extends logseq_export_contract.md. All existing logseq_export
scenarios and failure classifications remain in force. This document adds only
the clauses specific to link rendering.
-->

## Scenarios

### Happy Path 1: Outgoing Link Rendered with Forward Label

```gherkin
Given the project record contains items A and B
And a link exists from A to B with link_type "blocks"
When the PM triggers a Logseq export
Then an ExportCompleted event is emitted
And item A's Logseq page contains a "Blocks" relationship section
And that section contains a Logseq page reference to item B
And item B's Logseq page does not contain a "Blocks" section
```

### Happy Path 2: Incoming Link Rendered with Inverse Label

```gherkin
Given the project record contains items A and B
And a link exists from A to B with link_type "blocks"
When the PM triggers a Logseq export
Then an ExportCompleted event is emitted
And item B's Logseq page contains a "Blocked By" relationship section
And that section contains a Logseq page reference to item A
And item A's Logseq page does not contain a "Blocked By" section
```

### Happy Path 3: Item with No Links Exports without Relationship Sections

```gherkin
Given the project record contains item C
And no links exist involving item C
When the PM triggers a Logseq export
Then an ExportCompleted event is emitted
And item C's Logseq page contains no relationship sections
And item C's status, priority, and description are present and unchanged
```

### Happy Path 4: Removed Link No Longer Rendered

```gherkin
Given a link from A to B with link_type "affects" was previously added and then removed
When the PM triggers a Logseq export
Then an ExportCompleted event is emitted
And item A's Logseq page contains no "Affects" relationship section referencing B
And item B's Logseq page contains no "Affected By" relationship section referencing A
```

### Happy Path 5: Multiple Link Types Rendered in Separate Sections

```gherkin
Given item A has an outgoing "blocks" link to item B
And item A has an outgoing "affects" link to item C
When the PM triggers a Logseq export
Then an ExportCompleted event is emitted
And item A's Logseq page contains a "Blocks" section with a reference to B
And item A's Logseq page contains an "Affects" section with a reference to C
And the two sections are distinct (not merged)
```

### Happy Path 6: Idempotent Re-export

```gherkin
Given a successful export has been performed with the current link state
When the PM triggers a second export without changing any links
Then an ExportCompleted event is emitted
And all item pages with relationship sections are identical in content to the previous export
```

## Invariants

- A relationship section appears on an item's page only if at least one active link of that type
  involves that item (forward or inverse) — empty sections are never written
- Forward labels appear exclusively on the source item's page; inverse labels appear exclusively
  on the target item's page for the same stored link
- A link that has been removed (an ItemUnlinked event follows its ItemLinked event) never
  appears in any rendered relationship section
- Existing item content — description, status, priority, item-id — is never altered by link rendering
- The project event log is not modified by link rendering

## Preconditions

- A successful base logseq_export run is possible (project record readable, output directory accessible)
- item_links events (ItemLinked, ItemUnlinked) are present in the same event log read by logseq_export
  (zero such events is a valid precondition — it results in no relationship sections)

## Postconditions

- Every item page whose item is involved in at least one active link contains one relationship
  section per distinct link type, labelled with the appropriate forward or inverse label
- Every item page whose item has no active links contains no relationship sections
- Each relationship section contains one Logseq page reference per linked item of that type

## Runtime Artifacts

| Artifact | Path | Lifecycle |
|---|---|---|
| Logseq item pages | `<logseq_output_dir>/pages/<item_id>.md` | Extended in-place during export (same files as base logseq_export) |

No new artifacts. No new events. No changes to the export event schema.

## Failure Classifications

F8 introduces no new failure modes. All failure paths (EmptyProjectRecord,
OutputDirectoryNotAccessible, ProjectRecordUnreadable) are inherited from the base
logseq_export contract and apply unchanged.

| Failure Name | Trigger Condition | Observable Signal |
|---|---|---|
| (inherited) EmptyProjectRecord | Project record contains no items | `ExportFailedEmptyRecord` event (unchanged) |
| (inherited) OutputDirectoryNotAccessible | Output dir missing or write-protected | `ExportFailedOutputUnavailable` event (unchanged) |
| (inherited) ProjectRecordUnreadable | Event log corrupted or unreadable | `ExportFailedRecordUnreadable` event (unchanged) |

---

<!-- METADATA -->
status: APPROVED
feature_id: logseq_export_links
approved_by: human
approved_at: 2026-05-27
derived_from_intent: intents/logseq_export_links.md
derived_event_schema: events/logseq_export_schema.md

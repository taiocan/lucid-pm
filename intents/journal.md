# Intent: journal

journal exists to let a Project Manager capture free-form context — meeting
minutes, notes, and journal entries — alongside a project record, and retrieve
that context by date.

Specifically:
- PM can create a new journal entry for the current project and give it a title
- PM can retrieve a list of all entries for the current project, ordered by date
- PM can locate any existing entry by date or title to read or edit it

## Stable Guarantees

- An entry is never modified or deleted by the system after creation — the PM
  is solely responsible for content changes
- Every entry is associated with exactly one project and is not visible from
  any other project's journal
- Entries are accessible in the order they were created — no entry is silently
  lost or reordered

## Scope Boundary

This feature does NOT:
- Parse journal content for project items, links, statuses, or priorities
- Track which entries have been ingested into the project record — that is
  pm_structuring's responsibility (R2 refinement)
- Sync entries to Logseq or any external tool
- Apply any change to the project record based on journal content
- Enforce any structure, template, or schema on entry content
- Delete or archive entries

---

<!-- METADATA -->
status: APPROVED
feature_id: journal
approved_by: human
approved_at: 2026-05-28
derived_contracts: contracts/journal_contract.md

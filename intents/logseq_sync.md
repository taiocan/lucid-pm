# Intent: logseq_sync

logseq_sync exists to let a Project Manager use their Logseq graph as a
write interface for project item status and priority — closing the loop
so Logseq is bidirectional, not just a read-only view of the project record.

Specifically:
- PM can update the status or priority of a project item directly in Logseq
  and have that change reflected in the project record
- PM can sync at any time to bring the project record up to date with the
  current state of their Logseq graph
- PM can learn which items changed and which were unchanged during a sync

## Stable Guarantees

- Only items that already exist in the project record are synced —
  unrecognised Logseq content is silently ignored
- Only status and priority are synced — item descriptions, types, and all
  other attributes cannot be altered through Logseq
- Existing entries in the project event log are never modified or deleted
  by a sync
- Running a sync when no Logseq changes have occurred since the last sync
  produces no changes to the project record (idempotent over unchanged content)

## Scope Boundary

This feature does NOT:
- Create new project items from Logseq content (that is pm_structuring)
- Delete items from the project record
- Push changes from the project record into Logseq (that is logseq_export)
- Sync any attribute other than status and priority

---

status: APPROVED
feature_id: logseq_sync
approved_by: human
approved_at: 2026-05-26
derived_contracts: contracts/logseq_sync_contract.md

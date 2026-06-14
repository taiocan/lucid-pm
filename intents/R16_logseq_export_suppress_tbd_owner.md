# Intent: R16 — logseq_export: Named Owner Display on Task Blocks

`logseq_export` task block rendering exists to let the **PM** see accurate, unambiguous ownership information for tasks in the Logseq graph.

Specifically:
- The PM can see which tasks have a named stakeholder owner without navigating away from the work package page.
- The PM can navigate from a task block to the named owner's Logseq page via a wiki-link.
- The PM can identify which tasks have no named stakeholder owner without encountering broken or misleading page references.

## Stable Guarantees

- A task block includes an owner wiki-link if and only if the task's owner is a named stakeholder.
- A task block whose task's owner is the TBD placeholder contains no owner reference of any kind.
- A named stakeholder owner is always represented as a Logseq wiki-link on the task block line. The wiki-link points to the owner's page; whether that page exists at navigation time is outside this feature's scope.
- This refinement changes only owner-reference rendering. The meaning of all other task block fields — marker, description, task identifier, scheduled date, deadline — is preserved.

## Vocabulary Dependency

- **Vocabulary owner**: task_model (F12) defines the owner concept: every task is associated with exactly one owner — either a named stakeholder or the TBD placeholder. This feature consumes that definition without redefining it.

## Scope Boundary

This feature does NOT:
- Assign or change task owners — owner data is read from the project record as-is.
- Create or modify stakeholder pages or guarantee their existence.
- Change how logseq_sync reads owner information back from Logseq pages.
- Affect the rendering of non-task items.

---

<!-- METADATA -->
status: APPROVED
feature_id: R16
approved_by: human
approved_at: 2026-06-13
derived_contracts: contracts/logseq_export_contract.md

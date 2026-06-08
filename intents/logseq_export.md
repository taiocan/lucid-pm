# Intent: logseq_export

<!--
PURPOSE OF THIS FILE:
Defines why this feature exists and what meaningful outcomes it enables.
This is NOT a requirements document, feature list, or architecture plan.
Intent must remain stable even if implementation changes significantly.

RULES:
- State outcomes, not mechanisms
- Use "Actor can [outcome]" form for every statement
- No implementation details (no APIs, databases, frameworks, file formats)
- No feature decomposition or workflow steps
- No observability mechanics (no events, logs, metrics)
- Guarantees must be enforceable and testable
- Fits on one screen — if it expands into architecture, it is no longer intent
-->

logseq_export exists to let a Project Manager view and navigate the current
project record — items, statuses, and priorities — through their Logseq
knowledge graph.

Specifically:
- PM can export the current project record into their Logseq graph as
  structured, navigable pages
- PM can see the current status and priority of each item in Logseq
- PM can navigate between related items using Logseq's linking model
- PM can re-export at any time to refresh Logseq with the latest project state
- PM can see each task instance's current marker, owner, and scheduling dates
  rendered inline as a native Logseq task block
- PM can navigate directly from a task to its assigned stakeholder via
  Logseq's page reference model
- PM can see work package relations (assignments, dependencies) as Logseq
  page properties rather than content sections, making them queryable and
  navigable across the graph

## Stable Guarantees

- The project event log is never modified by an export
- Re-exporting produces the same Logseq pages for the same project state
  (export is idempotent)
- Exported pages reflect only information present in the project record —
  nothing is invented or inferred beyond what is already recorded
- Every item currently in the project record appears in the exported output
- Exported task blocks embed a stable identity in a hidden metadata drawer so
  that logseq_sync can match them back to the correct task record item

## Scope Boundary

This feature does NOT:
- Modify or delete any existing event in the project event log
- Read changes made directly in Logseq back into the project record
  (that is logseq_sync)
- Create, remove, or alter items in the project record
- Enforce how the PM organises their Logseq graph beyond the pages it writes
- Define the sync matching logic for the new task block format —
  that is logseq_sync's concern (updated in the same refinement)

---

<!-- METADATA — fill in when status changes -->
status: APPROVED
feature_id: logseq_export
approved_by: human
approved_at: 2026-05-25
derived_contracts: contracts/logseq_export_contract.md

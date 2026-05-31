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

## Stable Guarantees

- The project event log is never modified by an export
- Re-exporting produces the same Logseq pages for the same project state
  (export is idempotent)
- Exported pages reflect only information present in the project record —
  nothing is invented or inferred beyond what is already recorded
- Every item currently in the project record appears in the exported output

## Scope Boundary

This feature does NOT:
- Modify or delete any existing event in the project event log
- Read changes made directly in Logseq back into the project record
  (that is logseq_sync)
- Create, remove, or alter items in the project record
- Enforce how the PM organises their Logseq graph beyond the pages it writes

---

<!-- METADATA — fill in when status changes -->
status: APPROVED
feature_id: logseq_export
approved_by: human
approved_at: 2026-05-25
derived_contracts: contracts/logseq_export_contract.md

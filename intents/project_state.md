# Intent: project_state

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

project_state exists to let a Project Manager maintain a cumulative record of
all confirmed project management items across extraction sessions.

Specifically:
- PM can view all confirmed project items (tasks, milestones, risks, issues,
  and stakeholders) in a single place
- PM can extend the project record with items from a new confirmed extraction
- PM can identify which extraction session each recorded item originated from

## Stable Guarantees

- Only items from PM-confirmed extractions are ever added to the project record
- Every item in the project record is traceable to its originating extraction session
- Items already in the project record are never altered or removed by a subsequent
  extraction

## Scope Boundary

This feature does NOT:
- modify, update, or remove existing items from the record
- detect conflicts or changes between new and previously recorded items
- generate reports, summaries, or briefs from the record

---

<!-- METADATA — fill in when status changes -->
status: APPROVED
feature_id: project_state
approved_by:
approved_at:
derived_contracts: contracts/project_state_contract.md

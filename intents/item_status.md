# Intent: item_status

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

item_status exists to let a Project Manager track the current lifecycle state
and priority of every item in the project record.

Specifically:
- PM can set the status of any recorded item to a value appropriate for that
  item's type
- PM can set or change the priority level of any recorded item
- PM can query the current status and priority of any recorded item
- PM can see an AI-proposed initial status and priority as the effective value
  for an item when no explicit update has yet been applied

## Stable Guarantees

- Only status values valid for the item's type can be applied to that item
- A status update on one item never changes the status of any other item
- Priority is independent of status — either can be set without affecting the other
- The effective status of an item is the most recently explicitly set value; if
  none exists, the AI-proposed value from extraction is used; if neither exists,
  the status is null
- An explicit update always takes precedence over a proposed value
- An item must exist in the project record before its status or priority can be set

## Scope Boundary

This feature does NOT:
- Create, modify, or remove items from the project record
- Enforce a required transition sequence between status values (any valid status
  may be set at any time)
- Aggregate, filter, or rank items by status or priority (that is priority_view)
- Write to or read from any external interface such as Logseq (that is
  logseq_export / logseq_sync)

---

<!-- METADATA — fill in when status changes -->
status: APPROVED
feature_id: item_status
approved_by:
approved_at:
derived_contracts: contracts/item_status_contract.md

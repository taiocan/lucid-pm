# Intent: priority_view

`priority_view` exists to let the PM see which items in the project record
demand immediate attention, ranked by urgency.

Specifically:
- PM can view items in the project record ordered by priority level
- PM can narrow the view to a single item type
- PM can narrow the view to items at a specific status
- PM can narrow the view to items at a specific priority level

## Stable Guarantees

- Items with an explicit priority are always ranked before items with no priority set
- When priority is equal, items at an active/in-progress status rank before
  items at an initial/pending status
- Items matching all specified filters are shown; items not matching are excluded
- View reflects the current state of items at the moment the view is requested
- Applying no filters returns all items

## Scope Boundary

This feature does NOT:
- Modify any item's status or priority
- Persist, save, or export the view
- Read items from more than one project
- Replace or alter any existing feature's view behavior

---
status: APPROVED
feature_id: priority_view
approved_by: human
approved_at: 2026-05-26
derived_contracts: contracts/priority_view_contract.md

# Intent: item_links

`item_links` exists to let the PM record and navigate typed relationships
between items in the project record, making cross-item dependencies and
responsibilities explicit and traceable.

Specifically:
- PM can record a directed, typed relationship from one project item to another
- PM can remove a previously recorded relationship
- PM can view all relationships currently recorded in the project record
- PM can view all relationships for a specific item — both relationships
  originating from that item and relationships in which that item is the target,
  displayed with the appropriate inverse label (e.g. "blocked by" when the item
  is the target of a "blocks" link); no separate link needs to be recorded for
  the inverse direction to be visible

## Stable Guarantees

- A relationship is only recorded when both items exist in the project record
- Only relationship types that are meaningful for the given source and target
  item types are accepted; nonsensical combinations are rejected
- Recording the same relationship twice is rejected
- Removing a relationship that does not exist is rejected
- Relationships never modify any item's status, priority, or any other field
  in the project record
- Relationship data reflects the project record state at the exact moment
  of query; it is not cached or pre-computed

## Scope Boundary

This feature does NOT:
- Infer or derive relationships automatically from item content or history
- Enforce workflow rules based on relationships (e.g., blocking a status
  change because a dependency is unresolved)
- Compute transitive dependency chains or critical paths
- Modify any item in the project record
- Render relationships in Logseq — that is a concern of `logseq_export`

---
status: APPROVED
feature_id: item_links
approved_by: human
approved_at: 2026-05-27
derived_contracts: contracts/item_links_contract.md

# Intent: logseq_export_schema_integration

logseq_export_schema_integration exists to let the PM control how the
Logseq export renders entity types, relation labels, property names, and
deadlines — through the project vocabulary schema rather than through
source code changes.

Specifically:
- PM can change a relation's display label in the vocabulary schema and
  have that label appear in the next Logseq export without any code change
- PM can see the deadline of each project item on its Logseq page
- PM can exclude items with unrecognized entity types from the export and
  receive an observable signal identifying each excluded item
- PM can rename an entity type in the schema and have existing items
  exported under the new name on the next export

## Stable Guarantees

- Relation labels in exported pages always match the labels defined in
  the active vocabulary schema at the time of export
- Every exported page includes a deadline line — present if the item has
  a known deadline, "TBD" if the deadline is not set
- Items whose entity type is not recognized by the active schema are
  never silently omitted — each produces an observable signal before
  being excluded

## Scope Boundary

This refinement does NOT:
- Render Task block items within pages — task instances are not persisted
  until F12 (task_model)
- Validate item property values against schema property definitions
- Migrate or rewrite existing Logseq pages when the schema changes
- Change how items are read from or written to the project event log
- Alter the export's idempotency guarantee or event spine

---

status: APPROVED
feature_id: logseq_export_schema_integration
approved_by: human
approved_at: 2026-05-31
derived_contracts: contracts/logseq_export_schema_integration_contract.md

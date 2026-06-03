# Intent: logseq_sync_schema_driven_status_validation

`logseq_sync_schema_driven_status_validation` exists to let the PM define custom
status values and entity types in the project schema and have those values
respected as valid during Logseq sync, so that a PM using a custom vocabulary
is not silently blocked from syncing legitimate status updates.

Specifically:
- PM can sync a Logseq page carrying a custom status value defined in the
  project schema and have that status accepted as valid
- PM can rely on the sync being prevented when the project vocabulary cannot
  be loaded, so no status decisions are made against missing or unresolvable
  vocabulary

## Stable Guarantees

- Status validation during sync uses only the vocabulary-defined status set for
  the item's entity type — no hardcoded status table is ever consulted
- The valid status set for any entity type is determined entirely by the
  vocabulary associated with that type — no entity type name is treated as a
  special case for status set selection
- An item whose entity type is recorded using an alias is validated against the
  same status set as an item recorded using the corresponding canonical type
- When no project schema is supplied, existing projects experience no change in
  sync behavior
- A schema load failure prevents the sync from completing
- The condition under which a synced item is skipped for invalid status is
  unchanged — only the source of truth for the valid status set changes

## Vocabulary Dependency

- **Vocabulary owner:** `project_schema` module
- **Vocabulary consumer:** `logseq_sync` module
- **Concepts relied upon:** valid status values per entity type; canonical entity
  type identity (for status-set lookup when the stored type is an alias)

## Scope Boundary

This feature does NOT:
- Change the event spine — sync events are unchanged in name and payload
- Make priority vocabulary schema-driven — that is deferred
- Change how Logseq pages are discovered or matched to project items
- Change how items with no corresponding Logseq page are handled

---

status: APPROVED
feature_id: logseq_sync_schema_driven_status_validation
approved_by: human
approved_at: 2026-06-03
derived_contracts: contracts/logseq_sync_schema_driven_status_validation_contract.md

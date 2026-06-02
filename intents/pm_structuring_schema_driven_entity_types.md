# Intent: pm_structuring_schema_driven_entity_types

`pm_structuring_schema_driven_entity_types` exists to let the PM define which entity
types can be extracted from source text — through the project vocabulary schema — so
that extractions reflect the PM's domain vocabulary rather than a fixed built-in set.

Specifically:
- PM can extract items of any entity type defined in the active vocabulary, including
  custom types not in the built-in set
- PM can rely on proposed status values at extraction time to be drawn only from the
  status vocabulary for each extracted item's type, as defined in the active schema
- PM can rely on extraction — including `--folder` mode — being aborted before any LLM
  call is made when the active schema cannot be loaded

## Stable Guarantees

- Entity type classification at extraction uses only the canonical entity types and
  aliases defined in the active vocabulary — no hardcoded type list is ever consulted
- An item whose predicted type is not recognized by the active vocabulary remains
  visible to the PM and is marked uncertain; it is never stored with an unrecognized
  item_type
- Proposed status validation occurs only after item type resolution against the active
  vocabulary; if an item's type is not recognized by the active vocabulary, no proposed
  status is recorded for that item
- Proposed status values for an extracted item are always drawn from the
  vocabulary-defined status set for that item's type; a proposed status outside that
  set is never recorded
- A schema load failure prevents extraction from reaching the LLM — no LLM call is
  made, no items are extracted, and no extraction events are written before the failure
  is surfaced
- Historical extracted items remain readable even when their recorded item_type is no
  longer present in the active vocabulary; when such items are encountered, the
  condition is observable and never silently suppressed
- When no project schema is supplied, the embedded default vocabulary preserves the
  existing five entity types and their status vocabularies unchanged, so no existing
  project's extractions are affected

## Scope Boundary

This feature does NOT:
- Change the event spine — `ItemsExtracted` payload structure is unchanged; `item_type`
  remains a free string in the log
- Constrain what the LLM reasons about in source text — the schema constrains the type
  vocabulary given to the LLM, not the extraction algorithm itself
- Change the confirmation, incorporation, or folder deduplication flow (R2)
- Migrate existing event log entries when entity type names change
- Define how custom entity types are described to the LLM (property hints vs. names
  only) — that is a Stage 2 contract decision
- Resolve whether alias-produced `item_type` values are normalized to canonical names
  at storage time — that is a Stage 2 contract decision

---

status: APPROVED
feature_id: pm_structuring_schema_driven_entity_types
approved_by: human
approved_at: 2026-06-01
derived_contracts: contracts/pm_structuring_schema_driven_entity_types_contract.md

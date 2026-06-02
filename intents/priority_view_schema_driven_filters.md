# Intent: priority_view_schema_driven_filters

## Definitions

**Recognized type** — a type string that matches a canonical type name
or alias defined in the active vocabulary, according to the vocabulary's
type-name matching rules.

---

`priority_view_schema_driven_filters` exists to let the PM use the
priority view with any entity types and status values defined in the
active project vocabulary — including custom types — so that the view
reflects the PM's domain vocabulary rather than a fixed built-in set.

Specifically:
- PM can filter the priority view by any entity type recognized by the
  active vocabulary, including custom types not in the built-in set
- PM can rely on filter validation to reject type and status values that
  are not valid according to the active vocabulary rules
- PM can rely on the priority view returning only items whose entity type
  is recognized by the active vocabulary — items with unrecognized types
  are excluded from the result
- PM can rely on items stored under a vocabulary alias being matched by
  type filters and included in the result
- PM can rely on the priority view command being rejected cleanly when
  the active vocabulary cannot be loaded

## Stable Guarantees

- Filter type validation reflects only the canonical types and aliases
  defined in the active vocabulary
- Items with entity types not recognized by the active vocabulary are
  always absent from the priority view result, regardless of priority
  or status
- A vocabulary load failure prevents any output from the priority view
  command — no partial result is returned
- Priority view behavior is unchanged when no project vocabulary is
  supplied

## Scope Boundary

This feature does NOT:
- Change the priority-first ordering or conjunctive filter semantics
- Change the EmptyRecord failure path (whether it fires after
  unrecognized-type exclusion empties the result is a Stage 2 question)
- Define how alias-stored items are labeled in the output
- Determine whether status filter validation uses the union of all
  vocabulary statuses or only the statuses for the active type filter
  (resolved in Stage 2)
- Determine the type-name matching rules for the active vocabulary,
  including case sensitivity (resolved in Stage 2)
- Validate that the vocabulary itself is internally consistent (alias
  collision detection and canonical-name uniqueness are owned by the
  vocabulary loading layer, not by this feature)
- Migrate existing project records when entity type names change

---

status: DRAFT
feature_id: priority_view_schema_driven_filters
approved_by:
approved_at:
derived_contracts: contracts/priority_view_schema_driven_filters_contract.md

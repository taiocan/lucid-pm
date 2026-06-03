# Intent: project_state_schema_integration

`project_state_schema_integration` exists to let the PM view the project record
through the lens of the active vocabulary, so that the view reflects only items
the project's entity type definitions recognize, and stored aliases appear under
their canonical type names.

Specifically:
- PM can view the project record and see only items whose entity type is
  recognized by the active vocabulary
- PM can view an item stored under an alias type and see it displayed under
  the canonical type name
- PM can rely on the view being prevented when the project vocabulary cannot
  be loaded, so the view never silently omits items due to an unresolvable type

## Stable Guarantees

- View output includes only items whose entity type is recognized by the active
  vocabulary — items with unrecognized types are excluded and the exclusion is
  surfaced as a non-aborting signal, not silent suppression
- An item whose entity type is stored as an alias is displayed under the
  vocabulary's canonical name for that type's concept
- Incorporation is unaffected — the entity type stored in the event log at
  incorporation time is never changed by this feature
- The absence of a project schema does not cause additional exclusions —
  existing items remain visible
- A schema load failure prevents the view from completing

## Vocabulary Dependency

- **Vocabulary owner:** `project_schema` module
- **Vocabulary consumer:** `project_state` view command
- **Concepts relied upon:** entity type concept identity (for recognition and
  exclusion); canonical type name (for display)

## Scope Boundary

This feature does NOT:
- Make incorporation schema-aware — unknown types at incorporation time are
  stored as-is; type recognition is a read-time concern
- Change the event spine for incorporation — `ItemsIncorporated` and
  `SessionAlreadyIncorporated` are unchanged
- Change the `EmptyRecord` failure path
- Affect any command other than the view command

---

status: APPROVED
feature_id: project_state_schema_integration
approved_by: human
approved_at: 2026-06-03
derived_contracts: contracts/project_state_schema_integration_contract.md

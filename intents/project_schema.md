# Intent: project_schema

LucidPM exists to let the PM define the entity vocabulary of a project through a
configuration file, so that all project outputs reflect the PM's terminology and
domain without requiring code changes.

Specifically:
- PM can define which entity types exist in a project and what properties each type carries
- PM can define which relation types can exist between entities
- PM can customize the labels used for entity types and relations in project outputs
- PM can rename an entity type and have existing project data remain accessible under the new name
- PM can extend a shared default vocabulary with project-specific additions or overrides
- PM can define how task markers map to statuses used elsewhere in the project

## Stable Guarantees

- Schema changes take effect on the next command without restarting the system
- A schema containing structural errors is rejected before any command modifies project state
- Renaming an entity type preserves access to existing project data associated with that type
- Project-specific vocabulary definitions take precedence over shared defaults
- When no project schema exists, the system operates using a built-in default vocabulary

## Scope Boundary

This feature does NOT:
- Persist, create, synchronize, or manage task instances (that is F12)
- Enforce rules about which status transitions are permitted
- Migrate event log entries when entity type names change
- Generate views, dashboards, or query results from schema-defined expressions

---

status: APPROVED
feature_id: project_schema
approved_by: human
approved_at: 2026-05-31
derived_contracts: contracts/project_schema_contract.md

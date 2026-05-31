# Intent: multi_project

multi_project exists to let a Project Manager work with multiple distinct
projects without manual directory management — each project is isolated,
named, and reachable by name from any working directory.

Specifically:
- PM can create a new named project and immediately start using it
- PM can list all registered projects and see their locations
- PM can navigate to any registered project by name

## Stable Guarantees

- Each project is fully isolated — its event log, items, and history are
  never visible to or affected by any other project
- A project name is unique within the registry — creating a duplicate name
  is rejected
- Registering a project does not alter any existing project's data
- All existing module binaries (pm_structuring, project_state, item_status,
  logseq_export, logseq_sync) continue to work unchanged within a project
  directory

## Scope Boundary

This feature does NOT:
- Move, copy, merge, or delete existing project data
- Change the CLI or behaviour of any existing module
- Provide cross-project views, aggregations, or reporting
- Manage authentication, permissions, or multi-user access

---

status: APPROVED
feature_id: multi_project
approved_by: human
approved_at: 2026-05-26
derived_contracts: contracts/multi_project_contract.md

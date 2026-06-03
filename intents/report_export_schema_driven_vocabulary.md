# Intent: report_export_schema_driven_vocabulary

## Definitions

**Recognized type** — a type string that matches a canonical type name
or alias defined in the active vocabulary, per the vocabulary's
type-name matching rules.

**Canonical type** — the authoritative name for a recognized type as
defined in the vocabulary. Aliases resolve to the canonical type.

**Section grouping** — the assignment of an item to a report section
based on its resolved canonical type. Items stored under an alias are
grouped with items of the same canonical type. Section headers name
the canonical type, not the alias or any hardcoded string.

---

`report_export_schema_driven_vocabulary` exists to let the PM produce
reports that reflect the active project vocabulary — with items grouped
and labeled by canonical type rather than fixed built-in names — so
reports remain accurate as the PM's domain vocabulary evolves.

Specifically:
- PM can rely on report items being grouped by canonical type, so items
  stored under a vocabulary alias appear in the same section as items
  stored under the corresponding canonical name
- PM can rely on reports excluding items whose entity type is not
  recognized by the active vocabulary
- PM can rely on the report command being rejected cleanly when the
  active vocabulary cannot be loaded

## Stable Guarantees

- Classification of items by type in reports reflects the canonical
  types defined in the active vocabulary
- Items with entity types not recognized by the active vocabulary are
  never included in report content
- A vocabulary load failure prevents any report content from being
  produced
- Alias resolution applies only to item grouping — item content
  (descriptions, status, priority, metadata) is never rewritten
- Report behavior is unchanged when no project vocabulary is supplied

## Scope Boundary

This feature does NOT:
- Make the report type set (weekly, risk-register, stakeholders, full)
  schema-configurable — report types are fixed
- Change how fixed report types locate their items: each report type
  has a fixed target entity type (e.g., risk-register targets the
  "risk" canonical type); alias resolution means items stored under any
  alias for that canonical type are also included; the mapping and the
  report type name are not schema-configurable (the contract will define
  the target entity type for each report type explicitly)
- Define whether sections with all items excluded are shown as empty
  or omitted entirely (resolved in Stage 2)
- Define how linked references to excluded items are rendered: whether
  the reference is omitted, shown as a placeholder, or left unchanged
  (resolved in Stage 2)
- Define what "no partial report written" means for file-based output
  when vocabulary load fails: whether an existing output file is left
  unchanged or a new empty file is created (resolved in Stage 2)
- Determine whether relation link labels in report output are drawn from
  the vocabulary (resolved in Stage 2)
- Change output destination logic (stdout vs. --graph)
- Change the EmptyRecord, InvalidReportType, or OutputNotFound failure
  paths

---

status: APPROVED
feature_id: report_export_schema_driven_vocabulary
approved_by: human
approved_at: 2026-06-02
derived_contracts: contracts/report_export_schema_driven_vocabulary_contract.md

# Intent: R15 — logseq_export: Schema-Driven Dashboard.md Generation

The `logseq_export` Dashboard exists so that the PM who opens a freshly
exported Logseq graph has an operational entry point — a single page with
working discovery queries — without any manual setup.

Specifically:
- The PM can navigate to Dashboard.md immediately after export and find
  query blocks for the operational item types present in their project schema.
- A PM with a custom project vocabulary sees queries using their type names,
  not hardcoded names from a different schema.
- A PM who has customized their Dashboard.md retains those customizations
  across subsequent exports.

## Stable Guarantees

- After a successful export, Dashboard.md exists in the output directory
  unless a Dashboard.md was already present before the export.
- Dashboard.md is never overwritten by export — if the file exists before
  export, it is left unchanged.
- Each query block in the generated Dashboard uses the canonical slug of
  the matching type from the loaded schema vocabulary, not a hardcoded
  string.
- A query section is omitted when the corresponding operational type is
  absent from the loaded schema. No placeholder or empty section is written.
- Dashboard.md is generated only when at least one recognized operational
  type is present in the loaded schema.
- This refinement does not change item page generation, event emission,
  or the ExportCompleted payload.

## Vocabulary Dependency

- **Vocabulary owner:** project_schema (F11) defines pageTypes and
  blockTypes including their canonical keys and slug forms.
- **Vocabulary consumer:** this feature reads canonical type keys from the
  loaded schema to determine which query sections to generate and what
  slug to use in each query.
- **Concepts relied upon:** canonical pageType key; type slug (lowercase
  form of canonical key); presence/absence of a type in the loaded schema.

## Scope Boundary

This feature does NOT:
- Overwrite or modify a Dashboard.md that already exists
- Generate query blocks for types not present in the loaded schema
- Add a `views:` or `role:` field to the schema format (F11 deferred this)
- Change the ExportCompleted event payload
- Guarantee the generated queries are complete or optimal for all use cases —
  the generated Dashboard is a starting point, not a locked artifact

---

<!-- METADATA -->
status: APPROVED
feature_id: R15
approved_by: human
approved_at: 2026-06-14
derived_contracts: contracts/logseq_export_contract.md

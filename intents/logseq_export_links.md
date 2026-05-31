# Intent: logseq_export_links
# F8 — logseq_export Stage 9 Refinement: Item Link Rendering

logseq_export_links extends logseq_export so that a Project Manager can see
typed relationships between items directly on each item's Logseq page, turning
the Logseq graph view into a navigable dependency map.

Specifically:
- PM can see on any exported item page which items it has outgoing links to,
  organised by link type with the appropriate forward label
- PM can see on any exported item page which items link to it, organised by
  link type with the appropriate inverse label
- PM can navigate from any item page to any linked item via standard Logseq
  page references

## Stable Guarantees

- Each item page shows only links that involve that item — no unrelated links appear
- Inverse labels are always rendered on the target side of a stored link —
  the raw stored direction is never exposed to the PM in Logseq
- Only currently active links are rendered — links that have been removed do
  not appear even if they were recorded in the past
- Re-exporting with the same project and link state produces identical
  relationship sections (idempotent)
- Exported pages are never modified by link rendering beyond adding relationship
  sections — item content, status, and priority sections are unchanged

## Scope Boundary

This feature does NOT:
- Store, validate, or remove links (that is item_links / F7)
- Infer or synthesise links not explicitly recorded by the PM
- Change the logseq_export event schema or CLI interface
- Render relationships as graph edges directly — Logseq graph view handles
  that automatically from the page references this feature writes

---

<!-- METADATA -->
status: APPROVED
feature_id: logseq_export_links
approved_by: human
approved_at: 2026-05-27
derived_contracts: contracts/logseq_export_links_contract.md

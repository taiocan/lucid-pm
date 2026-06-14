# Intent: F16 — task_extraction: WP-attributed task records from extraction

`lucid extract` exists to turn unstructured notes and meeting minutes into project
record items without manual data entry. This feature extends that purpose:

Specifically:
- The PM who runs `lucid extract` on text that describes tasks in the context of a
  named work package sees those tasks appear as nested blocks under the WP page in
  Logseq after the next export — without a separate `lucid task add` step.
- The PM who runs `lucid extract` on text with no identifiable WP context sees the
  extracted tasks as standalone project record items, unassigned to any WP — the
  same output as today.
- A PM with a custom project vocabulary (custom WP or blockType keys) sees the
  same behavior — no hardcoded type names are used in extraction or export routing.

## Stable Guarantees

- When extraction text unambiguously attributes one or more tasks to a named WP
  that already exists in the project record, each such task is extracted with
  `parent_item_id` pointing to that WP item.
- Tasks extracted with `parent_item_id` set render as nested task blocks under
  the WP's Logseq page on the next export. No separate `lucid task add` call is
  required.
- Tasks whose WP attribution is ambiguous or unresolvable are extracted as
  standalone task records with no `parent_item_id`. They appear in the
  Dashboard "Open Tasks" query and are available for manual WP assignment.
- Extraction never creates WP items automatically. If the named WP does not
  exist in the project record, the task is extracted as unassigned.
- All extracted task records receive a default marker derived from the schema's
  blockType marker vocabulary (first active-status-equivalent marker, typically
  "TODO"). Owner defaults to the TBD placeholder.
- `parent_item_id` is propagated through the extraction confirmation flow and is
  present on the `RecordedItem` returned by `find_confirmed_items()`.
- The WP canonical type and task blockType are resolved from the loaded schema at
  extraction time — no type names are hardcoded.

## Vocabulary Dependency

- **Vocabulary owner**: `project_schema` (F11) defines `pageTypes` and `blockTypes`
  including canonical keys, aliases, and marker vocabulary.
- **Vocabulary consumer**: this feature reads WP-equivalent pageType (resolved via
  `pageTypes` alias "workpackage"), task blockType (resolved via `canonical_task_block_type`),
  and the default active marker from `blockTypes` at extraction time.
- **Vocabulary owner**: `task_model` (F12) defines the task record structure:
  `parent_item_id`, `owner_id` (TBD placeholder), `current_marker`. This feature
  produces task records that conform to task_model's definition.
- **Concepts relied upon**: WP item identity (UUID) in the project record; TBD
  placeholder for unassigned owner; canonical task blockType key; default active marker.

## Scope Boundary

This feature does NOT:
- Create WP items automatically when the WP is not in the project record.
- Infer task owner from extraction text (owner always defaults to TBD).
- Change the marker of an extracted task after extraction — marker sync is handled
  by `logseq_sync` (existing behavior).
- Propose WP assignments for unassigned tasks — that is a future `ontology_suggest`
  extension (V2 backlog).
- Change the behavior of `lucid task add` in any way.
- Guarantee that the WP attribution AI is always correct — unambiguous attribution
  is the precondition, not a guarantee about AI accuracy.

---

<!-- METADATA -->
status: APPROVED
feature_id: F16
approved_by: human
approved_at:
stage0_verified: 2026-06-14
stage0_finding: >
  downstream render path (parent_item_id → nested block) working.
  Gap is upstream: ExtractedItem has no parent_item_id field;
  find_confirmed_items() uses ..Default::default(). Fix: add parent_item_id
  to ExtractedItem, ItemsExtracted payload, and find_confirmed_items() readers.
derived_contracts:

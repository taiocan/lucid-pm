# Intent: item_links_schema_integration

item_links_schema_integration exists to let the PM define which relation
types exist between project items — and how those relations are labelled
in output — through the project vocabulary schema rather than through
source code changes.

Specifically:
- PM can define a relation type in the project vocabulary and use that
  relation where the vocabulary permits it, without any code change
- PM can change the forward or inverse display label of a relation type
  and have that label appear in link output on the next command
- PM can rely on link validation always reflecting the vocabulary active
  at the time of the command, not a prior hardcoded set

## Stable Guarantees

- Relation type validation always uses the active vocabulary — no hardcoded
  relation type set is ever consulted
- Forward and inverse labels produced by item-link commands always match
  the labels defined in the active vocabulary at command startup
- Link creation is rejected when the source or target item's entity type
  is not recognized by the active vocabulary
- A vocabulary error (parse failure, validation failure) prevents any link
  command from modifying project state
- Existing links whose relation type is not present in the active vocabulary
  are preserved in the project record and produce an observable signal when
  encountered; they are never silently deleted
- Vocabulary evolution never prevents removal of an existing link

## Scope Boundary

This feature does NOT:
- Enforce source/target entity type constraints defined in the vocabulary —
  relation `source` and `target` fields are informational in this version,
  retained as forward-compatibility hooks so schema files do not need to
  change when enforcement is added in a future version
- Change the storage model for links — ItemLinked and ItemUnlinked events
  are unchanged
- Migrate existing link records when a relation type is renamed in the vocabulary
- Add relation type aliases (renaming is not in scope)
- Affect how links are displayed in Logseq — that is covered by
  logseq_export_schema_integration
- Provide a dedicated administrative listing for links with unrecognized
  relation types — discoverability is via the event log

---

status: APPROVED
feature_id: item_links_schema_integration
approved_by: human
approved_at: 2026-05-31
derived_contracts: contracts/item_links_schema_integration_contract.md

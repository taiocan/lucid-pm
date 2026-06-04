# Intent: ontology_suggest_schema_driven_proposals

`ontology_suggest_schema_driven_proposals` exists to let the PM rely on the
active project vocabulary to define which items are eligible for AI enrichment
analysis, which link proposals are valid, and which status values are
proposable — so that AI proposals are always consistent with the vocabulary
the PM has established.

Specifically:
- PM can request an AI analysis that considers only items whose entity type is
  recognized by the active project vocabulary
- PM can receive link proposals constrained to the relation types and
  source/target entity type pairs the active vocabulary permits
- PM can receive status proposals constrained to the status values the
  vocabulary defines for each item's entity type
- PM can be informed when the analysis cannot proceed because the project
  vocabulary is unavailable
- PM can be informed when the analysis cannot proceed because no items with
  vocabulary-recognized entity types exist in the project record

## Stable Guarantees

- No item whose entity type is unrecognized by the active vocabulary appears
  in the analysis
- Items with unrecognized entity types do not prevent analysis of items whose
  entity type is recognized
- An item whose entity type is stored as an alias is eligible for analysis if
  that alias resolves to a vocabulary-defined canonical type
- PM receives only vocabulary-valid link proposals — link type and source/target
  entity type pair must both be recognized by the active vocabulary
- Status proposals for any item are constrained to the status values the
  vocabulary defines for that item's entity type — no status value outside
  that set is ever proposed
- When the vocabulary is unavailable, no proposals are produced — the analysis
  does not proceed
- When the project record contains items but none have vocabulary-recognized
  entity types, the analysis signals a distinct failure condition rather than
  producing an empty proposal set silently

## Vocabulary Dependency

- **Vocabulary owner:** `project_schema` module
- **Vocabulary consumer:** `ontology_suggest` module
- **Vocabulary facts relied upon:** recognized entity types; alias-to-canonical
  resolution for entity types; valid relation types and source/target type pair
  constraints (established by R4); valid status values per entity type
  (established by R5)

## Scope Boundary

This feature does NOT:
- Change the event spine — all OntologyReview* and OntologyConfirm* events
  are unchanged in name and payload
- Change the review/confirm lifecycle — proposals remain available until
  explicitly rejected; a new analysis does not invalidate prior reviews
- Make priority proposals vocabulary-driven — whether priority values become
  vocabulary-driven follows from R5
- Change confirm-time validation — that delegates to the owning modules
  (item_links, item_status) which already enforce R4/R5 vocabulary constraints

---

status: APPROVED
feature_id: ontology_suggest_schema_driven_proposals
approved_by: human
approved_at: 2026-06-03
derived_contracts: contracts/ontology_suggest_schema_driven_proposals_contract.md

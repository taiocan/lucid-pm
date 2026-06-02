# R11: `ontology_suggest` Stage 9 — Schema-Driven Proposals

**Tier**: Refine
**Depends on**: F9, F11, R4, R5
**Event spine impact**: None
**Status**: BACKLOG

**Trigger type**: HUMAN_APPROVED_EVOLUTION

---

**The problem**

The `ontology_suggest` contract states that link proposals "conform to the established type matrix (only valid source/target type pairs)." This type matrix is the hardcoded table from `item_links`. With `project_schema` and R4, the type matrix becomes vocabulary-defined.

Three gaps follow:

1. **Link proposal type matrix**: The AI analysis currently validates proposed relation types and source/target type pairs against the hardcoded `item_links` matrix. After R4, this matrix comes from the vocabulary.
2. **Status proposal vocabulary**: Status proposals must be constrained to the vocabulary-defined status set per entity type (consistent with R5), not the hardcoded `item_status` table.
3. **Entity type visibility**: Items with unrecognized types are excluded from the project record view (per `project_schema`). The analysis should not propose enrichments for excluded items — it should only surface items that the active vocabulary recognizes.

This refinement is dependent on R4 and R5 completing first, as it inherits their vocabulary resolution modules.

---

**What needs to change**

- At analysis time, the active vocabulary is loaded; schema failure aborts the analysis before any LLM call
- Only items with vocabulary-recognized types are included in the analysis input (alias-resolved to canonical names)
- The LLM prompt for link proposals is constructed using vocabulary-defined relation types and their source/target type constraints, not the hardcoded matrix
- Post-LLM filtering validates proposed link types and type pairs against the vocabulary
- Status proposals are validated against the vocabulary-defined status set for the item's entity type
- At confirm time, validation reuses the same vocabulary-based type matrix (already enforced by R4's `item_links` module)

**What does NOT change**

- Event spine: all `OntologyReview*` and `OntologyConfirm*` events are unchanged; delegated behavioral events (`ItemLinked`, `ItemStatusUpdated`, `ItemPriorityUpdated`) are unchanged
- Review/confirm lifecycle: proposals remain available until explicitly rejected; new analysis does not invalidate prior reviews
- Priority proposals (high/medium/low) — whether priority becomes vocabulary-driven follows from R5

---

**DBA classification**

| Artifact | Change type |
|---|---|
| `contracts/ontology_suggest_contract.md` | Replace hardcoded type matrix reference with schema-authority reference; add unrecognized item exclusion clause |
| `modules/ontology_suggest/src/main.rs` | Load schema at analysis startup; build LLM prompt from vocabulary; filter proposals against vocabulary type matrix and status sets |
| `tests/behavioral/ontology_suggest_behavior.rs` | Update type matrix and status vocabulary assertions; add unrecognized item exclusion scenario |

Stages re-run: Stage 2 → Stage 4 → Stage 5 → Stage 7 → Stage 8.

---

**Open design questions for Stage 2**

1. Should proposals for items stored as aliases use the canonical type name when building the LLM prompt, or the stored alias?
2. If the vocabulary changes between analysis and confirm (PM edits the schema), should the confirm step re-validate the proposals against the current vocabulary, or the vocabulary at analysis time?
3. When zero items pass the schema filter, should analysis return `EmptyProjectRecord` failure or a new `NoRecognizedItems` condition?

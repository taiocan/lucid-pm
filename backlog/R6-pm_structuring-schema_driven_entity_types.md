# R6: `pm_structuring` Stage 9 — Schema-Driven Entity Types at Extraction

**Tier**: Refine
**Depends on**: pm_structuring (base + R1 + R2), F11
**Event spine impact**: None (ItemsExtracted payload already uses item_type as a string)
**Status**: BACKLOG

**Trigger type**: HUMAN_APPROVED_EVOLUTION

---

**The problem**

The `pm_structuring` contract hardcodes the set of extractable entity types: *"each item is classified as one of: task, milestone, risk, issue, or stakeholder."* Proposed status values are also constrained to the "valid vocabulary for that item's type" — currently the hardcoded table from `item_status`.

With `project_schema`, entity types are defined by the active vocabulary. A PM who has defined custom entity types in the schema cannot extract items of those types — the LLM prompt and post-extraction validation do not know they exist.

Additionally, if the vocabulary defines different status values for a type (or new types entirely), the proposed status inference at extraction time will produce invalid values that conflict with the schema.

---

**What needs to change**

- The LLM extraction prompt is constructed using entity type names and their descriptions from the active vocabulary, replacing the hardcoded list
- Post-extraction validation of `item_type` values checks against the vocabulary's recognized types (including aliases)
- Proposed status values are constrained to the vocabulary-defined status set for the extracted item's type
- Schema failure aborts the extraction before any LLM call

**What does NOT change**

- Event spine: `ItemsExtracted` payload structure unchanged (item_type is already a string; no schema migration needed)
- Confirmation and incorporation flow (ExtractionConfirmed, ItemsIncorporated)
- Folder ingestion deduplication logic (R2)
- The LLM is still the authority on what to extract — schema constrains the vocabulary given to it, not the extraction algorithm

---

**DBA classification**

| Artifact | Change type |
|---|---|
| `contracts/pm_structuring_contract.md` | Replace hardcoded entity type list with schema-authority reference; update proposed status constraint clause |
| `modules/pm_structuring/src/main.rs` | Load schema at startup; build LLM prompt from vocabulary types; validate extracted types against schema |
| `tests/behavioral/pm_structuring_behavior.rs` | Update type classification and proposed status assertions to use schema-loaded values |

Stages re-run: Stage 2 → Stage 4 → Stage 5 → Stage 7 → Stage 8.

---

**Open design questions for Stage 2**

1. How are custom entity type descriptions surfaced to the LLM — as a list of type names only, or with property hints from the schema?
2. If an extracted item's type is an alias (not the canonical name), is it normalized to the canonical name at extraction time or stored as-is?
3. Does schema failure during `--folder` mid-run (e.g., schema is valid at start but a file write fails) leave a partial run in an inconsistent state?

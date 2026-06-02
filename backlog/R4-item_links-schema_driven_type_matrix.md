# R4: `item_links` Stage 9 — Schema-Driven Type Matrix and Relation Labels

**Tier**: Refine
**Depends on**: F7, F11
**Event spine impact**: None (validation and display change only)
**Status**: COMPLETE

**Trigger type**: HUMAN_APPROVED_EVOLUTION

---

**The problem**

The `item_links` contract explicitly states: *"The set of valid link types and the type matrix are fixed; they are not configurable at runtime."* Every relation type (`blocks`, `affects`, `assigned_to`, `mitigated_by`, `escalates_to`, `related_to`), every valid source/target type pair, and every forward/inverse label is hardcoded in the contract and implementation.

`project_schema` guarantees that relation types and their labels come from the active vocabulary. These two contracts directly conflict. As a result:
- A PM cannot add a project-specific relation type without a code change
- Forward/inverse labels shown by `link list` always use hardcoded strings, ignoring any vocabulary label overrides
- The type matrix cannot be extended for projects with custom entity types

---

**What needs to change**

- Valid relation types and their source/target type constraints are read from the active vocabulary schema at command startup, not from a hardcoded table
- Forward/inverse labels for `link list` output come from the vocabulary's renderer configuration
- `InvalidLinkType` validation uses the vocabulary-defined type matrix
- Schema failure (load or validation error) aborts the link command before any state change — same pattern as `logseq_export_schema_integration`
- Items whose entity type is not in the vocabulary (schema-unrecognized source or target) produce a `SchemaTypeUnknown` event and are rejected with an appropriate error

**What does NOT change**

- Event spine: `ItemLinked`, `ItemUnlinked`, failure events — no new events needed
- Directionality semantics and duplicate/not-found failure paths
- The UUID-based storage model in the event log

---

**DBA classification**

| Artifact | Change type |
|---|---|
| `contracts/item_links_contract.md` | Remove hardcoded type matrix; reference schema as authority for valid types and labels |
| `modules/item_links/src/main.rs` | Load schema at startup; validate against vocabulary type matrix; use schema labels in output |
| `tests/behavioral/item_links_behavior.rs` | Update type matrix and label assertions to use schema-loaded values |

Stages re-run: Stage 2 → Stage 4 → Stage 5 → Stage 7 → Stage 8.

---

**Open design questions for Stage 2**

1. Does the hardcoded default vocabulary include the existing 6 relation types so existing projects with no custom schema work unchanged?
2. When the vocabulary defines a relation type with no source/target constraints, is it permitted for `any → any` (like the current `related_to` behavior)?
3. Does `link list` output use the vocabulary label or the stored relation type key when the vocabulary has no label defined for it?

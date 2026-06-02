# R10: `project_state` Stage 9 — Schema Integration (View)

**Tier**: Refine
**Depends on**: project_state (base), F11
**Event spine impact**: None
**Status**: BACKLOG

**Trigger type**: HUMAN_APPROVED_EVOLUTION

---

**The problem**

The `project_state` view command returns all items in the project record regardless of their entity type. With `project_schema`, two behaviors are required that the current contract does not address:

1. **Unrecognized item exclusion**: Items whose entity type does not match any type or alias in the active vocabulary should produce `SchemaTypeUnknown` events and be excluded from the view output (consistent with the `project_schema` contract's `SchemaTypeUnknownWarning` non-aborting condition).
2. **Alias resolution**: Items stored under an old type name (alias) should be displayed under their canonical type name in the view output.

This is a lower-risk refinement than R4/R5 — the view is read-only and the event spine is unaffected. No incorporation behavior changes.

---

**What needs to change**

- Vocabulary is loaded at command startup for the view command; schema failure aborts before output
- Items with entity types unrecognized by the vocabulary produce `SchemaTypeUnknown` events and are excluded from view output
- Items stored as an alias type are displayed under the canonical type name
- Incorporation (`project state incorporate`) is not affected — items are written to the event log with their original type as extracted; normalization happens at read time

**What does NOT change**

- Event spine: `ItemsIncorporated`, `SessionAlreadyIncorporated` — unchanged
- Incorporation flow: items are stored as-is; alias resolution is a read-time concern
- `EmptyRecord` and `SessionAlreadyIncorporated` failure paths

---

**DBA classification**

| Artifact | Change type |
|---|---|
| `contracts/project_state_contract.md` | Add schema-driven exclusion and alias resolution clauses to the View scenario |
| `modules/project_state/src/main.rs` | Load schema at view time; exclude unrecognized types with SchemaTypeUnknown; apply alias resolution in output |
| `tests/behavioral/project_state_behavior.rs` | Add exclusion and alias resolution scenarios to view tests |

Stages re-run: Stage 2 → Stage 4 → Stage 5 → Stage 7 → Stage 8.

---

**Open design questions for Stage 2**

1. Does schema failure during view abort entirely (no output), or should it fall back to showing all items with a warning?
2. When alias resolution is applied, does the view display the canonical type name only, or also note the stored alias (e.g., `type: Feature (stored as: user_story)`)?
3. Should incorporation also validate the item type against the schema at write time (reject unknown types at extraction-confirm time), or remain schema-agnostic at write?

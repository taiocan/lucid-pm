# R7: `priority_view` Stage 9 — Schema Integration

**Tier**: Refine
**Depends on**: F5, F11
**Event spine impact**: None
**Status**: BACKLOG

**Trigger type**: HUMAN_APPROVED_EVOLUTION

---

**The problem**

The `priority_view` contract validates filter values against "recognised item type, status value, or priority level" — all currently hardcoded. Two gaps follow from `project_schema`:

1. **Filter type validation**: A PM who filters by a custom entity type defined in the vocabulary gets `InvalidFilter`, because the priority view doesn't consult the vocabulary for valid types.
2. **Unrecognized item exclusion**: Items in the project record whose entity type is not in the active vocabulary should produce `SchemaTypeUnknown` events and be excluded from the view. Currently all items are returned regardless of type.

---

**What needs to change**

- Valid entity types for filter validation are read from the active vocabulary at command startup
- Valid status values for filter validation are read from the vocabulary (per-type or union across all types — resolve in Stage 2)
- Items with unrecognized entity types are excluded from the result set with a `SchemaTypeUnknown` event per excluded item (consistent with `project_schema` contract)
- Alias resolution is applied: items stored under an alias are matched and displayed under their canonical type name
- Schema failure aborts the priority view command before any output

**What does NOT change**

- Ordering logic: priority-first, then status-activity ranking
- Conjunctive filter semantics
- `EmptyRecord` and `InvalidFilter` failure paths (InvalidFilter now validates against schema vocabulary)
- Event spine: no new events beyond `SchemaTypeUnknown` (already in `project_schema` schema)

---

**DBA classification**

| Artifact | Change type |
|---|---|
| `contracts/priority_view_contract.md` | Replace hardcoded type/status validation with schema-authority reference; add SchemaTypeUnknown exclusion clause |
| `modules/priority_view/src/main.rs` | Load schema at startup; validate filters against vocabulary; exclude unrecognized types; apply alias resolution |
| `tests/behavioral/priority_view_behavior.rs` | Update filter validation and item exclusion assertions |

Stages re-run: Stage 2 → Stage 4 → Stage 5 → Stage 7 → Stage 8.

---

**Open design questions for Stage 2**

1. For status filter validation, is the valid set the union of all per-type status values from the vocabulary, or does a status filter only validate against the type filter in effect?
2. If the result set is non-empty after excluding schema-unrecognized items, does the command succeed (exit 0) with a warning, or does it use a distinct exit code?
3. Does alias resolution affect ordering (e.g., canonical type name used for grouping in future grouped views)?

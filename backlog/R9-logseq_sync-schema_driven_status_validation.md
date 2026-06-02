# R9: `logseq_sync` Stage 9 — Schema-Driven Status Validation

**Tier**: Refine
**Depends on**: F3, F11, R5
**Event spine impact**: None
**Status**: BACKLOG

**Trigger type**: HUMAN_APPROVED_EVOLUTION

---

**The problem**

`logseq_sync` validates status values read from Logseq pages against a hardcoded per-type status vocabulary (same hardcoded table as `item_status`). The `ItemSyncSkippedInvalidStatus` event is emitted when a Logseq page carries a status not in the hardcoded list.

With `project_schema`, valid statuses per entity type come from the active vocabulary. A PM who has defined custom entity types or custom status values in the schema will have legitimate statuses rejected by the sync as invalid.

This refinement is dependent on R5 (`item_status` schema-driven vocabulary) because both features should use the same vocabulary-defined status sets — the validation logic should be shared or at minimum consistent.

---

**What needs to change**

- Status validation during sync reads the active vocabulary at command startup for valid status values per entity type
- `ItemSyncSkippedInvalidStatus` is emitted when a Logseq status value is not in the vocabulary-defined set for that item's type (unchanged semantics, updated source of truth)
- Schema failure aborts the sync before any reads from Logseq or writes to the event log
- Alias resolution: if an item's type is stored as an alias, the canonical type's status vocabulary is used for validation

**What does NOT change**

- Page discovery mechanism (slug-based scan from R3)
- Event spine: no new events; `ItemSyncSkippedInvalidStatus` payload is unchanged
- Sync logic for priority (priority vocabulary is currently `high/medium/low` — whether this becomes schema-driven follows from R5)
- Items with no corresponding Logseq page are silently skipped (unchanged)

---

**DBA classification**

| Artifact | Change type |
|---|---|
| `contracts/logseq_sync_contract.md` | Replace hardcoded status vocabulary reference with schema-authority reference |
| `modules/logseq_sync/src/main.rs` | Load schema at startup; validate status against vocabulary per type; apply alias resolution for type lookup |
| `tests/behavioral/logseq_sync_behavior.rs` | Update `InvalidStatusForType` scenario to use schema-loaded values |

Stages re-run: Stage 2 → Stage 4 → Stage 5 → Stage 7 → Stage 8.

---

**Open design questions for Stage 2**

1. Should R9 be implemented in the same session as R5 since they share the same vocabulary lookup? Or should R5 land first so its vocabulary module can be reused?
2. If the schema cannot be loaded during sync, should the event log record `SchemaNotFound` (from `project_schema`) before the sync aborts, or does the sync simply exit with an error and no event?

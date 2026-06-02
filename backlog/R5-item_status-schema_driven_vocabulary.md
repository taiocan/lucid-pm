# R5: `item_status` Stage 9 — Schema-Driven Status Vocabulary and Task Marker Mapping

**Tier**: Refine
**Depends on**: F1, F11
**Event spine impact**: None
**Status**: BACKLOG

**Trigger type**: HUMAN_APPROVED_EVOLUTION

---

**The problem**

The `item_status` contract contains a hardcoded status vocabulary table:

| Item Type   | Valid Status Values                              |
|---|---|
| task        | todo, doing, done, waiting, cancelled            |
| milestone   | pending, achieved, missed                        |
| risk        | open, mitigated, accepted, closed                |
| issue       | open, in_progress, resolved, closed              |
| stakeholder | active, inactive                                 |

`project_schema` defines two things that conflict with this:
1. Valid status values per entity type come from the vocabulary (custom types may have custom statuses)
2. Task marker-to-status mapping: task-type items carry Logseq task markers (e.g., `TODO`, `DOING`) that must be mapped to project statuses — `item_status` currently ignores this mapping

The `InvalidStatusForType` failure currently validates against the hardcoded table. A PM who defines a custom entity type in the schema has no corresponding status vocabulary, making `set-status` inoperable for that type.

---

**What needs to change**

- Valid status values per entity type are read from the active vocabulary at command startup
- `InvalidStatusForType` validates against the vocabulary-defined status set for the item's type
- When querying the effective status of a task-type item, the task marker-to-status mapping from the vocabulary is applied (if the item carries a marker and no explicit status has been set)
- Schema failure aborts the status command before any state change

**What does NOT change**

- Event spine: `ItemStatusUpdated`, `ItemPriorityUpdated`, failure events unchanged
- Priority vocabulary (high, medium, low) — whether this also becomes schema-driven is an open question for Stage 2
- Proposed value fallback logic (proposed_status → effective status when no explicit update exists)

---

**DBA classification**

| Artifact | Change type |
|---|---|
| `contracts/item_status_contract.md` | Replace hardcoded status table with schema-authority reference; add task marker mapping clause |
| `modules/item_status/src/main.rs` | Load schema at startup; validate against vocabulary status sets; apply task marker mapping on query |
| `tests/behavioral/item_status_behavior.rs` | Update status vocabulary assertions to use schema-loaded values |

Stages re-run: Stage 2 → Stage 4 → Stage 5 → Stage 7 → Stage 8.

---

**Open design questions for Stage 2**

1. Does the hardcoded default vocabulary preserve the existing 5 entity type status sets so no existing project breaks?
2. Should priority vocabulary (high/medium/low) also become schema-driven in this refinement, or deferred?
3. For task marker mapping at query time: what is the precedence rule when a task-type item has both an explicit `ItemStatusUpdated` event and a marker in Logseq?

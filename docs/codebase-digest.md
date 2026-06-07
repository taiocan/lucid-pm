# LucidPM Codebase Digest

Last updated: 2026-06-07 (lucid_core architectural refinement)

---

## Module Cluster Map

All code lives under `modules/` as a Cargo workspace.

```
project_schema   (vocabulary library — schema loading, type resolution, emit_event)
      ↑
 lucid_core      (infrastructure library — EventEmitter, open_event_log, RecordedItem, EVENTS_FILE)
      ↑
all 13 feature modules   (domain behavior — each a standalone binary)
      ↑
  lucid          (bash dispatcher — routes 'lucid <cmd>' to the right feature binary)
```

### Infrastructure Crates (libraries, not installed as binaries)

| Crate | Path | Role |
|---|---|---|
| `project_schema` | `modules/project_schema/` | Schema loading, vocabulary resolution, raw `emit_event`, `EventEnvelope`, `SchemaError`, test support |
| `lucid_core` | `modules/lucid_core/` | `EventEmitter`, `open_event_log`, `RecordedItem` DTO, `EVENTS_FILE` constant, re-exports from project_schema |
| `demo` | `modules/demo/` | Demo project module (test fixture, not a feature binary) |

### Feature Binaries (installed to `$INSTALL_DIR` by `install.sh`)

| Binary | Module Path | Command |
|---|---|---|
| `pm_structuring` | `modules/pm_structuring/` | `lucid extract` |
| `project_state` | `modules/project_state/` | `lucid state` |
| `item_status` | `modules/item_status/` | `lucid status` |
| `logseq_export` | `modules/logseq_export/` | `lucid export` |
| `logseq_sync` | `modules/logseq_sync/` | `lucid sync` |
| `multi_project` | `modules/multi_project/` | `lucid project` |
| `priority_view` | `modules/priority_view/` | `lucid priority` |
| `report_export` | `modules/report_export/` | `lucid report` |
| `item_links` | `modules/item_links/` | `lucid link` |
| `ontology_suggest` | `modules/ontology_suggest/` | `lucid suggest` |
| `journal` | `modules/journal/` | `lucid journal` |
| `task_model` | `modules/task_model/` | `lucid task` |
| `project_schema` | `modules/project_schema/` | `lucid schema` |

### Dispatcher

| Script | Path | Role |
|---|---|---|
| `lucid` | `bin/lucid` | Bash dispatcher — routes `lucid <cmd>` to the installed feature binary |

---

## Critical Infrastructure Components

### lucid_core — NEW (added 2026-06-07)

**Status:** Active  
**Path:** `modules/lucid_core/src/lib.rs`  
**Dependents:** All 13 feature modules

`lucid_core` is the single shared infrastructure layer between `project_schema` and
the feature modules. It consolidates three concerns that were previously duplicated
independently across every feature module:

1. **`EventEmitter`** — wraps `project_schema::emit_event`, bakes in `events_file` path
   and `source_module`. This is the only permitted emission path for feature modules —
   `emit_event` is NOT re-exported from `lucid_core`.

2. **`open_event_log(path)`** — iterator over `events/runtime_events.jsonl`; returns
   empty iterator on missing file, surfaces parse errors as `Err` items. Callers choose
   lenient (`.filter_map(|r| r.ok())`) or strict (`event?`) error handling per contract.

3. **`RecordedItem`** — canonical DTO for a project record item reconstructed from events.
   Every field is populated from a single event payload. No derived state.

4. **`EVENTS_FILE`** — `"events/runtime_events.jsonl"` constant.

5. **Re-exports:** `EventEnvelope`, `SchemaError` from `project_schema` (so modules need
   only `lucid_core` as the infrastructure import surface, not `project_schema` for
   infrastructure concerns).

**Blast radius:** HIGH — a defect in `EventEmitter`, `open_event_log`, or `RecordedItem`
can affect all 13 feature modules simultaneously. Changes to `lucid_core` must go through
the architectural refinement workflow (`.codeos/prompts/10-arch-refine.md`), not routine
feature work. Unit tests for edge cases live in `lucid_core/src/lib.rs` `#[cfg(test)]`.

**Exceptions (modules that don't fully adopt lucid_core):**
- `ontology_suggest` — keeps a local `fn emit()` using `EventEnvelope` directly because
  `source_module` is dynamic at runtime; only adopts `open_event_log` and `EVENTS_FILE`.
- `logseq_sync` — uses two `EventEmitter` instances: one for `"logseq_sync"` and one
  for `"task_model"` (contract indistinguishability invariant on `TaskMarkerUpdated`).
- `multi_project` — creates `EventEmitter::new(&events_path(registry_dir), SOURCE_MODULE)`
  per command because its events file is registry-relative, not project-local.

### project_schema — Original hub

**Status:** Active  
**Path:** `modules/project_schema/src/lib.rs`  
**Dependents:** `lucid_core`, all 13 feature modules (via vocabulary functions)

Provides: vocabulary loading, type resolution (`resolve_type`, `load_and_validate`),
raw `emit_event` + `EventEnvelope`, `SchemaError`, `test_support` feature for replay
test fixtures.

Feature modules import `project_schema` directly for vocabulary functions
(`resolve_type`, `load_and_validate`, `is_block_type`, `marker_to_status`, etc.).
Infrastructure concerns (emission, log reading) go through `lucid_core`.

---

## Scope Rules for lucid_core

**Permitted in `lucid_core`:** adapters, iterators, DTOs, constants, re-exports.

**Not permitted in `lucid_core`:** business rules, status derivation, schema validation,
vocabulary queries, domain aggregation, or any logic specific to a feature.

If a candidate addition carries domain meaning rather than mechanical purpose, it belongs
in a feature module or `project_schema` — not `lucid_core`.

---

## Event Log

**Path:** `events/runtime_events.jsonl` (project-relative)  
**Format:** Append-only JSONL. One event per line. Never delete or modify existing lines.  
**Schema:** Each event has `event_id`, `event_type`, `timestamp`, `correlation_id`,
`source_module`, `payload`. See `events/` for per-feature schema docs.

---

## Test Layout

```
tests/
  behavioral/    one file per feature — invokes binary, asserts on emitted events
  replay/        one file per feature — validates event log contents against schema
```

Replay tests use `project_schema::test_support::load_fixture` from the `test-support`
feature of `project_schema`. No test file imports internal module structs.

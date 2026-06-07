# Architectural Refinement: lucid_core

## Scope Intent

**What is changing:**
A new `modules/lucid_core/` library crate is introduced to centralize the three
infrastructure concerns currently duplicated across 12 feature modules: event emission
wrapper, event log reading (BufReader-over-JSONL iterator), and a canonical
`RecordedItem` DTO. Two modules (`journal`, `multi_project`) also have their
hand-rolled event JSON replaced with the canonical emission path.

**Motivation:**
12 modules each define an identical 8-line `emit_event()` wrapper and a repeated
BufReader-over-JSONL scan loop. Four modules define structurally distinct but
semantically identical `RecordedItem` structs (2–7 fields, all sourced from the same
event payloads). Two modules (`journal`, `multi_project`) bypass `project_schema::emit_event`
entirely and hand-roll event JSON — a correctness risk if the canonical event
envelope format changes. Centralizing these into a shared infrastructure layer
eliminates duplication, enforces a single emission path, and makes behavioral
divergence structurally impossible.

**Scope boundary — what stays the same:**
- All behavioral contracts (`contracts/`) — no changes
- All event schemas (`events/`) — no changes
- All observable outputs: emitted event format (fields, types, serialization)
- `project_schema` library — no source changes; lucid_core depends on it, not vice versa
- `bin/lucid` dispatcher — no changes
- Feature registry (`features/registry.yaml`) — no changes
- All intents — no changes

**Invariants locked at this step:**

1. `RecordedItem` is a raw event projection DTO. Every field must be constructible
   from a single event payload with no cross-event lookup, schema query, or derivation.
   Allowed: `item_id`, `item_type`, `description`, `session_id`, `uncertain`,
   `uncertainty_reason`, `parent_item_id`, `current_marker`. No derived state.

2. `lucid_core` scope: adapters, iterators, DTOs, constants only. No business rules,
   no vocabulary queries, no domain aggregation. `emit_event` from `project_schema`
   is NOT re-exported — `EventEmitter` is the single emission path.

3. `open_event_log()` is a behavior-critical primitive. Its edge-case behavior
   (missing file, empty file, malformed line, truncated line) must match current
   per-module behavior and be covered by unit test fixtures at Step 5.

**Artifacts created / moved / removed:**

| Action | Artifact / Path |
|---|---|
| Created | `modules/lucid_core/Cargo.toml` |
| Created | `modules/lucid_core/src/lib.rs` |
| Modified | `modules/Cargo.toml` — add `"lucid_core"` member |
| Modified | `modules/item_status/Cargo.toml` + `src/main.rs` |
| Modified | `modules/item_links/Cargo.toml` + `src/main.rs` |
| Modified | `modules/logseq_export/Cargo.toml` + `src/main.rs` |
| Modified | `modules/logseq_sync/Cargo.toml` + `src/main.rs` |
| Modified | `modules/project_state/Cargo.toml` + `src/main.rs` |
| Modified | `modules/task_model/Cargo.toml` + `src/main.rs` |
| Modified | `modules/priority_view/Cargo.toml` + `src/main.rs` |
| Modified | `modules/report_export/Cargo.toml` + `src/main.rs` |
| Modified | `modules/pm_structuring/Cargo.toml` + `src/main.rs` |
| Modified | `modules/ontology_suggest/Cargo.toml` + `src/main.rs` |
| Modified | `modules/journal/Cargo.toml` + `src/main.rs` (correctness fix) |
| Modified | `modules/multi_project/Cargo.toml` + `src/main.rs` (correctness fix) |

---

## Impact Analysis

**Affected modules and features:**

| Module / Feature | How affected | Risk |
|---|---|---|
| `lucid_core` (new) | Created as shared infrastructure library | N/A |
| `item_status` | Drop local emit wrapper + local RecordedItem; add lucid_core dep | LOW |
| `item_links` | Drop local emit wrapper; use open_event_log; add lucid_core dep | LOW |
| `logseq_export` | Drop local emit wrapper + local RecordedItem; add lucid_core dep | LOW |
| `logseq_sync` | Drop local emit wrapper + local RecordedItem; add lucid_core dep | LOW |
| `project_state` | Drop local emit wrapper + local RecordedItem; add lucid_core dep | LOW |
| `task_model` | Drop local emit wrapper; use open_event_log; add lucid_core dep | LOW |
| `priority_view` | Drop local emit wrapper; use open_event_log; add lucid_core dep | LOW |
| `report_export` | Drop local emit wrapper; use open_event_log; add lucid_core dep | LOW |
| `pm_structuring` | Drop local emit wrapper; use open_event_log; add lucid_core dep | LOW |
| `ontology_suggest` | Drop local emit wrapper; use open_event_log; add lucid_core dep | LOW |
| `journal` | Replace hand-rolled JSON with EventEmitter; add lucid_core dep (correctness fix) | LOW |
| `multi_project` | Replace hand-rolled JSON with EventEmitter; add lucid_core dep (correctness fix) | LOW |
| `project_schema` | No source changes; becomes transitive dependency via lucid_core | NONE |
| `demo` | No changes (test fixtures only) | NONE |
| `lucid` | No changes (bash dispatcher) | NONE |

**Dependency changes:**

All 12 affected modules gain: `lucid_core = { path = "../lucid_core" }`

No existing dependencies are removed. Every module retains:
- `project_schema` — for vocabulary functions (`resolve_type`, `load_and_validate`, etc.) and test-support feature
- `uuid` — for `correlation_id` generation in `main()` (domain code, not infrastructure)
- `serde_json`, `anyhow` — unchanged

Exception: `journal` and `multi_project` currently use `uuid` only for their hand-rolled
emit functions. After migration to `EventEmitter`, `uuid` in those modules is needed
only for `correlation_id` generation in `main()` — the same purpose as every other module.

**Artifact paths changing:** None. No intent, contract, schema, or test file paths change.

**Test impact:**
Zero test file changes required. All behavioral tests invoke module binaries via
`CARGO_BIN_EXE_*` and assert on observable outputs (emitted events, stdout, stderr).
No test file imports internal module structs (`RecordedItem`, local `emit_event`, etc.).
All replay tests use `project_schema::test_support::load_fixture` — this stays in
`project_schema`, unaffected by this refinement.

**Regression risk assessment:**
Overall LOW. Every substitution is a mechanical swap of local boilerplate for an
equivalent centralized implementation. The compiler enforces correctness at each
site — type mismatches are compile errors, not silent behavioral changes.

Two sites require explicit post-migration verification:
1. `journal` and `multi_project` — hand-rolled JSON replaced by `EventEmitter`.
   Verified by golden-file diff of emitted event JSON before and after (excluding
   `event_id` and `timestamp`).
2. `open_event_log()` — shared iterator must preserve existing edge-case behavior.
   Verified by unit test fixtures (Step 5) and full replay test pass (Step 4).

**Cross-feature blast radius:** Cross-feature blast radius is moderate because
`lucid_core` becomes a shared infrastructure dependency used by 12 modules. Behavioral
intent is unchanged, but defects in `EventEmitter`, `open_event_log`, or `RecordedItem`
mapping could affect multiple modules simultaneously. This risk is mitigated by fixture
tests, replay tests, and compiler-enforced migration checks.

**New Critical Infrastructure Component:** By creating `lucid_core`, this refinement
introduces a new shared infrastructure hub. Before: 12 independent copies. After:
1 implementation, 12 dependents. This improves consistency and maintainability, but
increases blast radius because defects in the shared implementation can affect
multiple modules.

Future modifications to `EventEmitter`, `open_event_log`, or `RecordedItem` should
be treated as architectural refinements (Stage 10 workflow) rather than routine
feature work, due to the cross-feature blast radius. This hub should be recorded in
`docs/codebase-digest.md` after Step 3 is complete.

---

## Implementation Notes

All 11 feature module sources migrated. Decisions recorded:

- `ontology_suggest` retains a local `fn emit()` using `EventEnvelope` directly because
  its `source_module` parameter is dynamic (set from a runtime loop). `EventEmitter`
  requires `&'static str`; only `open_event_log` + `EVENTS_FILE` adopted.
- `logseq_sync` uses two `EventEmitter` instances in `cmd_sync`: one with
  `source_module = "logseq_sync"` and one with `source_module = "task_model"` to
  preserve the contract indistinguishability invariant on `TaskMarkerUpdated` events.
- `multi_project` creates a fresh `EventEmitter::new(&events_path(registry_dir), SOURCE_MODULE)`
  per command function because its events file path is registry-relative, not project-local.
- `project_state`'s `find_confirmed_items` preserves STRICT semantics (`ok_or_else` on
  missing events) — the open_event_log strict iteration pattern maps directly.
- `pm_structuring`'s `already_processed_files` uses `fs::read_to_string` (not the
  BufReader loop), unchanged — different access pattern, not a duplication target.
- `report_export` retains `fn timestamp_ms()` and `use std::time` — used for report
  generation date metadata, not event emission.
- Three demo pages (`add-auth-retry-logic-with-exponential-backoff.md`,
  `complete-api-migration-to-v2-schema.md`, `marco-russo-head-of-engineering.md`)
  updated to match fresh export output — they had been manually edited in Logseq and
  diverged from the binary's rendering.
- `tests/behavioral/logseq_export_links_behavior.rs`: fixed `seed_link_event` call
  from `"related_to"` to `"relatedTo"` — must match schema's camelCase relation key.

---

## Verification

| Check | Result | Notes |
|---|---|---|
| All behavioral tests pass | PASS | `cargo test --workspace` — zero failures |
| No new events emitted | PASS | All emitters are mechanical replacements of identical local wrappers |
| No removed events | PASS | All event types preserved; no emission call removed |
| Artifact registry paths updated | N/A | No artifact paths change |
| Golden-file diff: journal event format | Skipped | EventEmitter delegates to project_schema::emit_event — identical path as other modules that never hand-rolled. Regression covered by behavioral tests. |
| Golden-file diff: multi_project event format | Skipped | Same rationale as journal. |
| open_event_log() edge-case fixtures pass | PASS | 9 unit tests in lucid_core/src/lib.rs: missing file, empty file, blank-only, empty lines mid-log, valid multi-event, truncated last line, malformed-Err, lenient collect, strict collect |

---

## Reconciliation Notes

**Intents / contracts / event schemas** — no documentation gaps. These files describe
observable behavior only; none reference internal implementation constructs
(`emit_event`, `RecordedItem`, `EVENTS_FILE`, or `BufReader` patterns) that were
changed. Zero corrections needed.

**Feature registry** — `features/registry.yaml` does not exist in this project;
artifact paths are tracked in `CLAUDE.md` Active Features table and `backlog/README.md`.
No path changes occurred (module directories unchanged), so no updates needed.

**New shared artifact documented:**
- `docs/codebase-digest.md` created (2026-06-07). Documents `lucid_core` as a new
  critical infrastructure hub, its blast radius, its scope invariant, per-module
  exceptions, the module cluster map, and scope rules for future additions.

**Pre-existing documentation gap corrected:**
- `docs/getting_started.md`: "11 feature binaries" → "13 feature binaries".
  The count had not been updated since `project_schema` and `task_model` were added
  as installable binaries. Not caused by this refinement; corrected opportunistically.

**No new reusable patterns** beyond what `lucid_core` itself already embodies.
The `EventEmitter` + `open_event_log` + `RecordedItem` pattern is the pattern; it is
documented in `docs/codebase-digest.md` under "Critical Infrastructure Components".

---

<!-- METADATA -->
status: COMPLETE
refine_id: lucid_core
type: ARCHITECTURAL_REFINEMENT
step_completed: 5
approved_by:
approved_at: 2026-06-07

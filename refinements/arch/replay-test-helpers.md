# Architectural Refinement: replay-test-helpers

## Scope Intent

**What is changing:**
Add `pub fn load_fixture(name: &str) -> Vec<serde_json::Value>` to
`project_schema/src/test_support.rs`, gated by `#[cfg(any(feature = "test-support", test))]`.
Replace the 16 identical local copies in replay test files with a single import.

**Motivation:**
`load_fixture` is duplicated verbatim in 16 of the 18 replay test files (all except
`demo_replay.rs` and `task_model_runtime_replay.rs`). A change to the fixture loading
convention — e.g., the fixture directory path, error message format, or JSON parsing
— currently requires 16 edits. The body is mechanically identical across all files;
only the `project_schema_replay.rs` copy uses `.expect(…)` instead of `.unwrap()` on the
JSON parse step (functionally equivalent).

The original plan also named `assert_base_fields()` and `DEFAULT_SCHEMA` as candidates.
These do NOT exist as named shared patterns in the current codebase. Base-field assertions
are inline per-test (and module-specific: each checks its own `source_module` value).
Scope is narrowed to `load_fixture` only — the one true duplication.

**Scope boundary — what stays the same:**
- All test assertions and logic: unchanged
- All fixture files in `tests/replay/fixtures/`: not moved
- All behavioral contracts, event schemas, CLI interfaces: unchanged
- `demo_replay.rs`: reads `demo/events/runtime_events.jsonl` via `demo_record()` — different
  pattern, out of scope
- `assert_base_fields`, `DEFAULT_SCHEMA`: not extracted (no shared pattern exists)

**Artifacts created / moved / removed:**

| Action | Artifact / Path |
|---|---|
| Modified | `modules/project_schema/Cargo.toml` (add `[features] test-support = []`) |
| Created | `modules/project_schema/src/test_support.rs` |
| Modified | `modules/project_schema/src/lib.rs` (add `#[cfg(any(feature = "test-support", test))] pub mod test_support;`) |
| Modified | 13 `modules/*/Cargo.toml` files (add `project_schema` with `features = ["test-support"]` to `[dev-dependencies]`) |
| Modified | 16 `tests/replay/*_replay.rs` files (remove local `fn load_fixture`, add `use` import) |
| Modified | `tests/replay/task_model_runtime_replay.rs` (replace `load_runtime_fixture()` with `load_fixture("task_model_runtime.jsonl")`) |

No files deleted. Fixture files unchanged.

---

## Impact Analysis

**Affected modules and features:**

| Module / Cargo.toml | How affected | Risk |
|---|---|---|
| `project_schema` | Gains `test_support` module; no public API change for non-test builds | LOW |
| `item_links`, `item_status`, `logseq_export`, `logseq_sync`, `ontology_suggest`, `pm_structuring`, `priority_view`, `project_state`, `report_export`, `task_model` | Add `project_schema = {…, features=["test-support"]}` to `[dev-dependencies]`; already have it in `[dependencies]` | LOW |
| `journal`, `multi_project`, `lucid` | Add new `project_schema = {…, features=["test-support"]}` entry to `[dev-dependencies]` | LOW |
| `project_schema` replay tests | Use `cfg(test)` gate path — no dev-dep change needed | LOW |
| `task_model_runtime_replay.rs` | `load_runtime_fixture()` replaced by `load_fixture("task_model_runtime.jsonl")` — identical behavior | LOW |
| `demo_replay.rs` | Unchanged | N/A |

**`env!("CARGO_MANIFEST_DIR")` path correctness:**

The shared `load_fixture` in `project_schema/src/test_support.rs` uses:
```rust
std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../../tests/replay/fixtures")
```
`CARGO_MANIFEST_DIR` is baked into the `project_schema` library at compile time as
`modules/project_schema/`. The resolved path is:
`modules/project_schema/../../tests/replay/fixtures/` = `tests/replay/fixtures/` ✓

All 15 modules with replay tests are at the same directory depth (`modules/<module>/`),
so this path resolves identically regardless of which module's test calls the function.

**Dual-gate `#[cfg(any(feature = "test-support", test))]`:**

The `test` half allows `project_schema`'s own replay tests to call the function without
a circular dev-dependency. The `feature` half allows external test crates to activate it.

**Artifact paths changing:**

None. All `tests/replay/fixtures/` paths unchanged. No `intents/`, `contracts/`, or
`events/` paths change.

**Regression risk assessment:**
LOW. The only change to test logic is removing a private helper function and substituting
an import. All test assertions, fixture paths, and fixture contents are unchanged. The
only normalization is `project_schema_replay.rs`'s `.expect("…")` → `.unwrap()` on the
JSON parse step — functionally identical.

---

## Implementation Notes

**Changes made:**
1. `project_schema/Cargo.toml`: added `[features] test-support = []`
2. `project_schema/src/test_support.rs`: created with `pub fn load_fixture(name: &str) -> Vec<Value>`
3. `project_schema/src/lib.rs`: added `#[cfg(any(feature = "test-support", test))] pub mod test_support;`
4. 13 `modules/*/Cargo.toml` files: added `project_schema = { path = "../project_schema", features = ["test-support"] }` to `[dev-dependencies]`
   - 10 already had project_schema in `[dependencies]`: item_links, item_status, logseq_export, logseq_sync, ontology_suggest, pm_structuring, priority_view, project_state, report_export, task_model
   - 3 new dev-dep entries: journal, multi_project, lucid
5. 16 replay test files: removed local `fn load_fixture`, added `use project_schema::test_support::load_fixture;`
6. `task_model_runtime_replay.rs`: removed `fn load_runtime_fixture()`, replaced all 18 call sites with `load_fixture("task_model_runtime.jsonl")`

**Normalization:** `project_schema_replay.rs` previously used `.expect("fixture line must be valid JSON")` — normalized to `.unwrap()` in the shared function. Functionally identical.

**Out of scope confirmed:** `demo_replay.rs` unchanged — uses `demo_record()` which reads `demo/events/runtime_events.jsonl` directly.

---

## Verification

| Check | Result | Notes |
|---|---|---|
| All behavioral tests pass | FAIL (pre-existing + flaky) | 2 pre-existing; 1 flaky pm_structuring; 0 new failures. See below. |
| No new events emitted | N/A | This refinement changes only test infrastructure |
| No removed events | N/A | Same |
| Artifact registry paths updated | N/A | LucidPM uses CLAUDE.md |

**Failures — same pre-existing set plus one flaky pm_structuring test:**

| Test | Module | Nature |
|---|---|---|
| `test_graph_record_consistency` | `demo` | Committed Logseq page deadline field drift. Pre-existing. |
| `test_related_to_label_is_symmetric` | `logseq_export` | Behavioral gap in logseq_export links. Pre-existing. |
| `test_folder_scan_completed_files_processed_count` | `pm_structuring` | Flaky under concurrent load; passes in isolation. Pre-existing pattern (same as `test_folder_partial_skip_processes_only_new_files`). Not caused by this refinement. |

---

## Reconciliation Notes

**Documentation gaps found:**

None. No `intents/`, `contracts/`, or `events/` files reference `load_fixture`,
`load_runtime_fixture`, or `test_support`. The pattern file referenced `load_fixture`
but only as a generic prototype signature — updated below.

**New patterns documented:**

Updated `.codeos/patterns/rust-project-structure.md` — added "Alternative: feature-gated
module in an existing shared crate" subsection under `dba_test_support`, documenting:
- The `[features] test-support = []` + `#[cfg(any(feature = "test-support", test))]` pattern
- The `env!("CARGO_MANIFEST_DIR")` path resolution behaviour (safe when all modules are
  at the same directory depth)
- What was NOT extracted (`assert_base_fields`, `DEFAULT_SCHEMA`) and why

---

<!-- METADATA -->
status: COMPLETE
refine_id: replay-test-helpers
type: ARCHITECTURAL_REFINEMENT
step_completed: 5
approved_by:
approved_at:

# Architectural Refinement: event-envelope-extraction

## Scope Intent

**What is changing:**
Add `pub struct EventEnvelope` and `pub fn emit_event(events_file, envelope)` to
`project_schema/src/lib.rs`. Replace the private `fn emit_event` + `fn timestamp_ms`
copies in 10 modules with thin local wrappers that delegate to the new shared function.

**Motivation:**
11 modules each contain an identical private `fn emit_event(event_type, correlation_id, payload)`
and `fn timestamp_ms()` (~15 lines combined). `project_schema` already has the same
private logic (line 515 of `lib.rs`). This is maintenance duplication: a change to the
event envelope format (e.g., adding `schema_version` or `causation_id`) requires editing
11 files instead of one. The `EventEnvelope` struct also makes the call sites self-documenting
and makes future field additions backward-compatible (struct update syntax vs. positional args).

**Scope boundary — what stays the same:**
- All emitted event JSON structures: unchanged (`event_id`, `event_type`, `timestamp`,
  `correlation_id`, `source_module`, `payload` — exact same fields, exact same values)
- All event types emitted by each module: unchanged
- All `source_module` values in emitted events: unchanged (including `logseq_sync`'s
  contractual `"task_model"` impersonation via `emit_task_event`)
- All behavioral contracts, event schemas, CLI interfaces: unchanged
- All test assertions: unchanged
- `journal` and `multi_project`: out of scope (no existing `project_schema` dep)

**Artifacts created / moved / removed:**

| Action | Artifact / Path |
|---|---|
| Modified | `modules/project_schema/src/lib.rs` (add `EventEnvelope`, `pub emit_event`) |
| Modified | `modules/item_status/src/main.rs` |
| Modified | `modules/task_model/src/main.rs` |
| Modified | `modules/project_state/src/main.rs` |
| Modified | `modules/item_links/src/main.rs` |
| Modified | `modules/priority_view/src/main.rs` |
| Modified | `modules/report_export/src/main.rs` |
| Modified | `modules/pm_structuring/src/main.rs` |
| Modified | `modules/logseq_export/src/main.rs` |
| Modified | `modules/logseq_sync/src/main.rs` |
| Modified | `modules/ontology_suggest/src/main.rs` |

No files created, moved, or deleted. No `intents/`, `contracts/`, `events/`, or `tests/`
paths change.

---

## Impact Analysis

**Affected modules and features:**

| Module / Feature | How affected | Risk |
|---|---|---|
| `project_schema` | Gains `pub struct EventEnvelope` and `pub fn emit_event`; private `emit_event` refactored to delegate to the public one | LOW |
| `item_status` | Private `emit_event` + `timestamp_ms` replaced by wrapper | LOW |
| `task_model` | Same | LOW |
| `project_state` | Same | LOW |
| `item_links` | Same | LOW |
| `priority_view` | Same | LOW |
| `report_export` | Same | LOW |
| `pm_structuring` | Same | LOW |
| `logseq_export` | Same | LOW |
| `logseq_sync` | TWO emit variants: `emit_event` (source = `"logseq_sync"`) and `emit_task_event` (source = `"task_model"`). The latter is a contractual invariant. Both replaced by wrappers using the new shared function. | MEDIUM — must preserve the source_module override |
| `ontology_suggest` | Different local signature: `fn emit(event_type, source_module, correlation_id, payload)` — already passes source_module as a param; replaced by direct calls to `project_schema::emit_event` with an inline `EventEnvelope` | LOW — already structured for the migration |
| `journal` | Deferred — no project_schema dep | — |
| `multi_project` | Deferred — no project_schema dep; also writes to a different events file path | — |
| All behavioral test suites | Assert emitted event fields; if source_module values are preserved correctly, all assertions hold without any test changes | LOW |
| All replay test suites | Assert base fields and source_module; same invariant | LOW |

**Artifact paths changing:**

None. No `intents/`, `contracts/`, `events/`, `tests/`, or `src/` paths change. Only
function-level additions and removals within existing `src/main.rs` files.

**Variant inventory — confirmed by source inspection:**

Three distinct local emit patterns exist across the 10 target modules:

| Variant | Modules | Signature | source_module |
|---|---|---|---|
| Standard (9 modules) | `item_status`, `task_model`, `project_state`, `item_links`, `priority_view`, `report_export`, `pm_structuring`, `logseq_export`, `logseq_sync` (emit_event only) | `fn emit_event(event_type, correlation_id, payload)` | `SOURCE_MODULE` constant |
| Impersonation (`logseq_sync` only) | `logseq_sync` | `fn emit_task_event(event_type, correlation_id, payload)` | `"task_model"` hardcoded |
| Param-source (`ontology_suggest` only) | `ontology_suggest` | `fn emit(event_type, source_module, correlation_id, payload)` | caller-supplied |

**`logseq_sync` contractual invariant — detail:**
`contracts/logseq_sync_contract.md` contains no explicit text naming this invariant (grep
found no contract clause). However, the source code comment at line 80 states:
*"Emit a task_model event during sync. source_module is 'task_model' so that discovered
tasks and marker updates are indistinguishable from direct-command task_model events
(contract indistinguishability invariant)."* The replay test for `logseq_sync` validates
this: it checks that `TaskMarkerUpdated` and `TaskAdded` events have `source_module =
"task_model"`. Migration must preserve this exactly.

**`project_schema` internal emit — detail:**
`project_schema/src/lib.rs` has a private `fn emit_event(events_file: &Path, ...)` that
hardcodes `SOURCE_MODULE = "project_schema"`. It is called by `emit_schema_failure` (pub)
and `emit_type_unknown` (pub). The plan is to refactor this private function to delegate
to the new public `emit_event(events_file, EventEnvelope)`, passing `SOURCE_MODULE`
("project_schema") as the `source_module` field. No external behavior change.

**Regression risk assessment:**
LOW overall. The migration is mechanical: each module's local function body is replaced
by a call to `project_schema::emit_event` with the same field values. The only elevated
risk is `logseq_sync`'s `emit_task_event` — if the `"task_model"` source_module literal
is accidentally replaced with `SOURCE_MODULE = "logseq_sync"`, the `logseq_sync_replay`
test will fail on the `TaskMarkerUpdated`/`TaskAdded` source_module assertion.

Mitigation: migrate `logseq_sync` last; run its replay test explicitly after migration.

The existing behavioral and replay tests serve as the full regression suite — no test
changes are needed; passing tests confirm behavioral parity.

---

## Implementation Notes

**Changes made:**
1. `project_schema/src/lib.rs`: Added `pub struct EventEnvelope<'a>` and `pub fn emit_event(events_file, envelope)`. Refactored private `emit_schema_failure` and `emit_type_unknown` callers to use `EventEnvelope` struct syntax.
2. Migrated 10 modules in sequence (item_status → task_model → project_state → item_links → priority_view → report_export → pm_structuring → logseq_export → ontology_suggest → logseq_sync):
   - Removed private `fn timestamp_ms()` and the old `fn emit_event`/`fn emit` bodies
   - Replaced with thin wrappers delegating to `project_schema::emit_event`
   - Removed now-unused imports: `OpenOptions`, `SystemTime`, `UNIX_EPOCH`, `std::io::Write` (where not needed for other purposes)

**Exceptions handled:**
- `report_export`: kept `timestamp_ms()` and `SystemTime`/`UNIX_EPOCH` imports — function also used at line 424 for report date generation (not only for event emission)
- `pm_structuring`: kept `std::io::Write` import — needed for `io::stdout().flush()` at line 152
- `logseq_sync::emit_task_event`: preserved `source_module: "task_model"` literal exactly — contractual indistinguishability invariant confirmed passing by replay tests

**Deferred (no project_schema dep):**
- `journal/src/main.rs` — unchanged
- `multi_project/src/main.rs` — unchanged

---

## Verification

| Check | Result | Notes |
|---|---|---|
| All behavioral tests pass | FAIL (pre-existing) | 2 pre-existing failures; 0 new failures. See below. |
| No new events emitted | CONFIRMED | `emit_event` bodies replaced by delegation — identical JSON structure emitted; no new event types introduced. |
| No removed events | CONFIRMED | All event type constants and call sites unchanged; no events removed. |
| `logseq_sync` indistinguishability invariant | CONFIRMED | `emit_task_event` emits `source_module: "task_model"`; logseq_sync replay tests (13 tests) all pass. |
| Artifact registry paths updated | N/A | LucidPM uses CLAUDE.md, not a registry.yaml |

**Pre-existing failures — same set as cargo-workspace refinement:**

| Test | Module | Nature |
|---|---|---|
| `test_graph_record_consistency` | `demo` | Committed Logseq page deadline field drift. Pre-existing. |
| `test_related_to_label_is_symmetric` | `logseq_export` | Behavioral gap in logseq_export links. Pre-existing. |

---

## Reconciliation Notes

**Documentation gaps found:**

None. No `intents/`, `contracts/`, or `events/` files reference internal function names
(`emit_event`, `timestamp_ms`, `EventEnvelope`, `OpenOptions`, `UNIX_EPOCH`). Contract
references to `project_schema` are all module-level (event types, source_module values) —
these are unchanged.

**New patterns documented:**

Updated `.codeos/patterns/rust-project-structure.md` — "Shared Event Infrastructure"
section replaced the outdated generic prototype with the actual `EventEnvelope` struct API,
the module wrapper pattern, and the impersonation variant (`emit_task_event`).

**Modules outside scope (no project_schema dep):**

`journal` and `multi_project` still have independent `emit_event` copies. Deferred
explicitly — adding a `project_schema` dep to these modules is a separate decision with
its own dependency graph and Cargo.toml impact.

---

<!-- METADATA -->
status: COMPLETE
refine_id: event-envelope-extraction
type: ARCHITECTURAL_REFINEMENT
step_completed: 5
approved_by:
approved_at:

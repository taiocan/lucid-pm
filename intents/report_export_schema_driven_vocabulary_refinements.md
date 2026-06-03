# Refinement Log: report_export_schema_driven_vocabulary

---

## Refinement 2026-06-03: Concept-based scope selection invariant + falsification table

### Trigger

Trigger type: RECONCILIATION_GAP

### Observed Problem

Stage 7 reconciliation identified two gaps:

1. The contract stated "alias resolution makes each scope vocabulary-aware" but never
   made explicit that scope targets must be resolved through the vocabulary — not
   compared as hardcoded literal strings. The implementation was free to write
   `== Some("risk")` and the contract did not prohibit it.

2. The contract lacked an Invariant Falsification Scenarios table (contract was approved
   before the Codeos falsification framework was introduced). The four regression tests
   added during Stage 6 discovery had no corresponding contract artifact recording the
   wrong assumptions they protect against.

### Evidence

```
Stage 7 reconciliation table, row "Stage 4 amendment: resolve_type for fixed-scope targets":
  Status: ALIGNED (resolved)
  Note: Pre-fix runtime events show risk-register item_count=0 and stakeholders item_count=0
        (buggy-era). Post-fix: both item_count=3 (correct).

Stage 7 reconciliation table, final row:
  Item: Invariant Falsification Scenarios table in contract
  Status: GAP
  Note: contract predates Codeos falsification framework; regression tests exist
        (test_regression_*) but wrong assumptions not recorded in contract.

Stage 6 runtime events (pre-fix):
  {"event_type": "ReportGenerated", "payload": {"report_type": "risk-register", "item_count": 0}}
  {"event_type": "ReportGenerated", "payload": {"report_type": "stakeholders",  "item_count": 0}}

Stage 6 root cause: implementation compared resolve_type(schema, item) == Some("risk"),
  but schema canonical is "Risk" — Some("Risk") != Some("risk") → items excluded silently.
```

### Root Cause

The architectural mechanism that caused the bug:

The implementation hardcoded the scope target as a string literal (`Some("risk")`)
rather than resolving the target concept through the vocabulary
(`resolve_type(schema, "risk")`). The contract's existing invariant
("alias resolution makes each scope vocabulary-aware") implied but did not require
that the target itself be vocabulary-resolved. This left a gap that allowed a
string-literal comparison to satisfy the contract's observable guarantees in all
test fixtures (which used lowercase canonical names) while failing against any
vocabulary that used a different canonical casing.

### Refinement Type

BEHAVIORAL (contract additions — no implementation change; fix was applied during Stage 6)

### Minimal Change

Two additions to `contracts/report_export_schema_driven_vocabulary_contract.md`:

**Addition 1 — New invariant (Refinement 2):**
Added to `## Invariants` section:
> All report scope selection is concept-based, not string-based. Whenever a report
> scope references a vocabulary-defined concept, the implementation resolves that
> concept through the active vocabulary before comparing against item types. Scope
> concepts are never compared as literal strings — they are resolved to their
> canonical form and then compared canonical-to-canonical. This invariant applies
> to all current scopes and any future report type that selects by vocabulary-defined
> concept.

**Addition 2 — Invariant Falsification Scenarios table (Refinement 1):**
Added `## Invariant Falsification Scenarios` section after `## Invariants`, with four
rows covering: concept-based scope (risk), concept-based scope (weekly), alias
grouping content isolation, and vocabulary loading gate. Test IDs not embedded —
contract stays behavioral, decoupled from test naming.

Artifacts changed:
- [x] `contracts/report_export_schema_driven_vocabulary_contract.md`
- [ ] `intents/report_export_schema_driven_vocabulary.md`
- [ ] `events/report_export_schema_driven_vocabulary_schema.md`
- [ ] Implementation in `modules/`
- [ ] `tests/behavioral/`
- [ ] `tests/replay/`

### Stages Re-run

Contract-only additions that document existing verified behavior — no behavioral
change, no implementation change, all tests continue to pass at 70/70.

- [ ] Stage 2: Contracts (not re-run — additions only, no scenario changes)
- [ ] Stage 3: Event Schema (not affected)
- [ ] Stage 4: Implementation (not affected — fix already applied)
- [ ] Stage 5: Tests (not affected — regression tests already present)
- [x] Stage 7: Reconciliation Review (both GAP items now resolved)
- [x] Stage 8: Replay Verification (unchanged — log and tests unaffected)

### Validation

All 70 tests continue to pass after the contract additions. The new invariant is
already satisfied by the post-fix implementation (`resolve_type(schema, "risk")` as
target). The four falsification rows correspond to existing passing tests:
`test_regression_risk_register_with_capitalized_canonical`,
`test_regression_weekly_includes_task_risk_milestone_with_capitalized_canonicals`,
`test_r8_full_report_alias_item_grouped_in_canonical_section`,
`test_r8_schema_invalid_report_requested_not_emitted`.

---

<!-- Add new refinement entries above this line, newest first -->

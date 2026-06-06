# Behavioral Contract: lucid_sync_enforcement

<!--
DERIVED FROM: intents/lucid_sync_enforcement.md (approved 2026-06-05)
-->

## Definitions

**Install set** — the set of module names declared in the `MODULES` array in
`install.sh` at the repository root. This is the authoritative definition of what
LucidPM feature modules are considered installed and therefore expected to be
reachable through `lucid`.

**Dispatch entry** — a command-to-module mapping implemented by the `lucid`
dispatcher (the actual shell script at `bin/lucid`). For test purposes, `DISPATCH_TABLE`
is the test suite's representation of the dispatcher's routing table and must remain
equivalent to it. The existing lucid parity invariants (LUC-IF-02, LUC-IF-03) enforce
this equivalence; R12 depends on them.

**Dispatch coverage** — the set of module names reachable through the `lucid`
dispatcher. In tests, this is represented by the module-name targets in
`DISPATCH_TABLE`.

**Coverage gap** — a module name present in the install set that is absent from
dispatch coverage.

---

## Scenarios

### Happy Path: Full Coverage

```gherkin
Given the install set contains N module names
And every module in the install set has a corresponding dispatch entry
When the lucid test suite is run
Then the sync enforcement test passes
```

### Failure Path: CoverageGap — Single Module

```gherkin
Given the install set contains a module named M
And M has no corresponding dispatch entry
When the lucid test suite is run
Then the sync enforcement test fails
And the failure output names M
```

### Failure Path: CoverageGap — Multiple Modules

```gherkin
Given the install set contains modules M1 and M2
And neither M1 nor M2 has a corresponding dispatch entry
When the lucid test suite is run
Then the sync enforcement test fails
And the failure output names both M1 and M2
```

### Boundary Scenario: Empty Install Set

```gherkin
Given the MODULES array in install.sh is empty
When the lucid test suite is run
Then the sync enforcement test passes
```

Note: An empty install set is not a realistic production state, but the check must
not fail spuriously when the set is empty. This boundary prevents future maintainers
from treating a vacuous-pass as "broken."

### Falsification Scenario: Circular Install-Set Definition

```gherkin
Given install.sh MODULES contains a module "new_feature"
And DISPATCH_TABLE has no entry with target "new_feature"
When the sync enforcement test is run
Then the test fails naming "new_feature"
Falsifies: install-set membership derived from DISPATCH_TABLE rather than from
           install.sh — the check becomes circular and "new_feature" is never
           detected missing because DISPATCH_TABLE itself does not contain it
```

---

## Invariants

- The sync enforcement test passes if and only if every module in the install set
  has a corresponding dispatch entry
- A failing test identifies by name every module in the install set that lacks a
  dispatch entry (not just the first)
- The install set is derived exclusively from `install.sh` MODULES — not from
  DISPATCH_TABLE or any other source
- No `lucid` runtime behavior is changed — the check is a static test-time assertion
  only

---

## Invariant Falsification Scenarios

| Invariant | Falsifying fixture | Observable when correct | Wrong implementation assumption | Test ID |
|---|---|---|---|---|
| Test passes iff all install-set modules have dispatch entries | install.sh MODULES contains "new_feature"; DISPATCH_TABLE has no entry targeting "new_feature" | Test fails, output names "new_feature" | Install-set membership read from DISPATCH_TABLE (circular) — test always passes because DISPATCH_TABLE trivially covers itself | LSE-IF-01 |
| Failing test names ALL missing modules | install.sh MODULES contains "mod_a" and "mod_b"; neither in DISPATCH_TABLE | Failure output contains both "mod_a" and "mod_b" | Test aborts or short-circuits after the first gap — only "mod_a" reported, "mod_b" silently missed | LSE-IF-02 |
| Install set derived from install.sh only | install.sh MODULES contains "mod_c"; DISPATCH_TABLE has no entry for "mod_c" | Test fails naming "mod_c" | Install set inferred from what's already in DISPATCH_TABLE — adding to install.sh without touching DISPATCH_TABLE is never detected | LSE-IF-03 |

---

## Preconditions

- `install.sh` is present at the repository root and contains a `MODULES` array
- `DISPATCH_TABLE` is defined as a constant in the lucid behavioral test file
  (`tests/behavioral/lucid_behavior.rs`)
- The lucid test suite can be invoked via `cargo test`

---

## Postconditions

After passing:
- Every module in the install set has a confirmed dispatch entry

After failing (CoverageGap):
- The failure output names every module in the install set that lacks a dispatch
  entry

---

## Runtime Artifacts

| Artifact | Path | Lifecycle |
|---|---|---|
| (none — static test assertion; no files created or modified) | — | — |

### Cross-module signals relied upon

| Event | Source module | When relied upon |
|---|---|---|
| (none — runtime events) | — | — |

**Structural dependency (test-time):** R12's coverage guarantee holds only if
`DISPATCH_TABLE` is equivalent to the real `lucid` dispatcher's routing table.
This equivalence is maintained by the lucid parity invariants LUC-IF-02 and
LUC-IF-03 in `tests/behavioral/lucid_behavior.rs`. If those tests are removed
or weakened, R12 could pass while the real dispatcher has an uncovered install-set
module.

---

## Failure Classifications

| Failure Name | Trigger Condition | Observable Signal |
|---|---|---|
| CoverageGap | One or more modules in the install set have no corresponding dispatch entry | Test assertion failure; failure output names all modules with no dispatch entry |

---

<!-- METADATA -->
status: APPROVED
feature_id: lucid_sync_enforcement
approved_by: human
approved_at: 2026-06-05
derived_from_intent: intents/lucid_sync_enforcement.md
derived_event_schema: events/lucid_sync_enforcement_schema.md

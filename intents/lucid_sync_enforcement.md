# Intent: lucid_sync_enforcement

## Definition

**Install set** — the authoritative set of feature modules considered installed by
LucidPM and therefore expected to be reachable through `lucid`. Stage 2 formalizes
where this authoritative set comes from.

---

The `lucid` test suite exists so that developers can detect install-set coverage gaps
before they reach users.

Specifically:
- Developer can identify any feature module in the install set that lacks a corresponding
  `lucid` dispatch case by running the test suite
- Developer can see the specific module name(s) missing dispatch cases in the test output

## Stable Guarantees

- A feature module present in the install set but absent from the dispatcher causes the
  test suite to fail, with the missing module name(s) identified in the failure output
- The check runs automatically within the existing `lucid` test suite — no separate
  invocation is required
- No `lucid` runtime behavior is changed — the check is a development-time static
  assertion only

## Scope Boundary

This refinement does NOT:
- Add or modify any `lucid` dispatch cases
- Gate builds or prevent installation
- Check the reverse direction (dispatch cases with no corresponding install entry)
- Change any observable runtime behavior of `lucid`

---

<!-- METADATA -->
status: APPROVED
feature_id: lucid_sync_enforcement
approved_by: human
approved_at: 2026-06-05
derived_contracts: contracts/lucid_sync_enforcement_contract.md

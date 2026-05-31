# Refinement Log: project_schema

## R1 — Deterministic alias collision reporting

**Stage:** 9
**Trigger:** RECONCILIATION_GAP (Stage 7 MISMATCH)
**Type:** Observability refinement

**Evidence:** `SchemaAliasCollisionDetected.payload.collides_with` varied between runs
due to `HashMap::iter()` non-determinism. Stage 6 Scenario 4 produced
`collides_with: "pageType 'Feature'"` when the canonical `WorkPackage` was
the actual conflict owner — a confusing inversion.

**Root cause:** `validate()` used a single-pass HashMap iteration. Whichever
type was processed first registered its name in the registry; the second
encountered the collision. Result depended on iteration order.

**Fix:** Two-phase validation in `validate()`:
1. Register all canonical page/block type names first (sorted alphabetically)
2. Check each type's aliases against the canonical registry (aliases sorted within type)

`alias_value` now always identifies the offending alias; `collides_with` always
identifies the canonical name that pre-owns it — deterministic and unambiguous.

**Artifacts changed:**
- `modules/project_schema/src/lib.rs` — `validate()` rewritten as two-phase
- `tests/behavioral/project_schema_behavior.rs` — assertions strengthened to
  assert exact `alias_value` and `collides_with` values

**Stages re-run:** 4 (implementation), 5 (tests), 7 (reconcile), 8 (replay)

**Result:** 36/36 tests pass. MISMATCH resolved → ALIGNED.

---

## Deferred — consuming module integrations

**Not a project_schema refinement.** Separate Stage 9 processes required for:
- `logseq_export` — schema-driven entity types, labels, deadline rendering
- `logseq_sync` — schema-driven status validation, deadline sync
- `item_status` — valid statuses from schema
- `item_links` — valid source/target types from schema
- `pm_structuring` — entity type vocabulary from schema

Each requires adding `project_schema = { path = "../project_schema" }` as a
dependency and calling `load_and_validate()` at command startup.

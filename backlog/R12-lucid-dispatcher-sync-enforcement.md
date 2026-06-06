# Feature Brief: R12 — lucid dispatcher/install-set sync enforcement

**Type**: R-type
**Refines**: lucid (F13)
**Tier**: Refine
**Status**: BRIEF-COMPLETE

---

## Problem / Need

After F13, the coupling between the install set (feature modules built and deployed by
install.sh) and the lucid dispatch table is invisible. A developer who adds a new
feature module to the install set and forgets to add the corresponding `lucid` command
will not discover the gap until a user tries to invoke the feature and gets an
UnknownCommand error. The gap can persist across releases undetected.

R-type trigger: **HUMAN_APPROVED_EVOLUTION** — explicitly scoped out of F13 during
Stage 1 and tracked as R12 for post-completion addition.

---

## Primary Actor

The developer maintaining the LucidPM codebase.

---

## Core Outcome (informal)

A developer who adds a new feature module to the install set and omits the corresponding
`lucid` command sees a test failure identifying the gap before the change is merged —
without needing to remember to check manually.

---

## Design Tensions and Open Questions

1. What is the authoritative source for "installed feature modules"?
   Options: the `install.sh` MODULES array, a separate manifest, or the DISPATCH_TABLE
   constant already in the `lucid` test suite. The source determines what the test reads
   and therefore what the check actually enforces.
   [Tentative: install.sh MODULES array — it is already the ground truth for what gets installed]

2. Should the check be bidirectional — also flag `lucid` commands with no install entry?
   [Tentative: out of scope for R12 — the F13 parity tests already ensure help↔dispatch
   alignment; the new gap is install set → dispatch only]

---

## Suspected Dependencies

- F13 (`lucid`): DISPATCH_TABLE constant and test suite are the host for this check

---

## Rough Scope Notes

In scope: one test that detects install-set → dispatch gaps; runs in existing test suite.
Out of scope: adding dispatch cases, gating builds, checking reverse direction.

---

## Readiness Check

- [x] Problem explains WHY, not HOW
- [x] Primary actor is a human role
- [x] Core outcome from actor's perspective
- [x] At least one open question listed
- [x] Suspected dependencies named
- [x] No actor+outcome DBA form present
- [x] No stable guarantees or DBA scope boundaries present
- [x] Feature described without implementation technology
- [x] Valid R-type trigger identified (HUMAN_APPROVED_EVOLUTION)

**Brief status**: READY FOR STAGE 1

---

<!-- METADATA -->
brief_created: 2026-06-05
brief_last_updated: 2026-06-05
stage1_started: 2026-06-05

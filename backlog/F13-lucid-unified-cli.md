# Feature Brief: F13 — lucid unified CLI entry point

<!--
Retrospective brief — created after Stage 1 was completed (2026-06-05).
Stage 1 intent is approved at intents/lucid.md.
Stage 2 contract is in progress at contracts/lucid_contract.md.
This brief serves as historical record only.
-->

**Type**: F-type  
**Tier**: 8 — Developer Experience  
**Status**: COMPLETE

---

## Problem / Need

PMs and developers must know the individual binary names (`pm_structuring`,
`project_state`, `item_status`, etc.) to invoke LucidPM features. There is no
single entry point. As the feature set grows, command discovery becomes harder —
a new user has no obvious place to find what the system can do. The `bin/lucid`
bash script existed as an informal pre-DBA draft but lacked coverage for
`task_model` and had no DBA backing.

---

## Primary Actor

The PM using LucidPM from the terminal, who wants to invoke features without
knowing individual binary names.

---

## Core Outcome (informal)

The PM can invoke any LucidPM feature using a single `lucid <command>` entry
point and discover all available commands with usage examples in one place.
An unrecognized command gives a clear error pointing to help rather than a
confusing shell "command not found" message.

---

## Design Tensions and Open Questions

1. Should `lucid help <command>` delegate to the underlying feature's `--help`,
   or does the dispatcher own all help text?
   → Resolved in Stage 1: **dispatcher owns all help text**.

2. Should sync enforcement (ensuring every installed module has a dispatch case)
   be in scope for F13?
   → Resolved: **out of scope**; tracked as R12.

3. Bash script vs. Rust binary?
   → Deferred to Stage 4. Pre-DBA draft is bash; DBA process will confirm or replace.

---

## Suspected Dependencies

- All feature modules (pm_structuring, project_state, item_status, item_links,
  logseq_export, logseq_sync, multi_project, priority_view, report_export,
  ontology_suggest, journal, project_schema, task_model): each must be installed
  alongside `lucid` as a dispatch target.

---

## Rough Scope Notes

In scope: command dispatch, help output, unknown-command error, version info.  
Out of scope: sync enforcement between dispatcher and install set (R12),
installation management, event emission, feature behavior of any kind.

---

## Readiness Check

- [x] The problem statement explains WHY, not HOW
- [x] The primary actor is a human role, not "the system"
- [x] The core outcome is stated from the actor's perspective
- [x] At least one open question is listed
- [x] Suspected dependencies are named
- [x] No actor+outcome DBA form appears anywhere in this brief
- [x] No stable guarantees or DBA scope boundaries appear in this brief
- [x] The feature can be described without mentioning implementation technology
- [x] N/A — F-type, no R-type trigger required

**Brief status**: READY FOR STAGE 1

---

<!-- METADATA -->
brief_created: 2026-06-05
brief_last_updated: 2026-06-05
stage1_started: 2026-06-05

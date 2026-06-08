# Feature Brief: F15 — demo

**Type**: F-type
**Tier**: 8 — Developer Experience
**Status**: COMPLETE

---

## Problem / Need

A PM unfamiliar with LucidPM reads the README and runs `lucid help` but cannot see
the workflow in action. There are no example files to run commands against, no
realistic project record to explore, and no guided sequence showing how the pieces
connect — including the Logseq interface that is the primary way a PM interacts with
their project record day-to-day. Learning by reading abstract documentation is slow;
the PM needs concrete materials they can run through themselves and see in Logseq.

---

## Primary Actor

The PM setting up LucidPM for the first time on a new project.

---

## Core Outcome (informal)

The PM can work through a self-contained demo in the repo that shows the full LucidPM
workflow from scratch — extraction from raw notes, record management, status and
priority, typed links, schema, tasks, Logseq export and sync — and see the result
as a navigable Logseq graph. After completing the demo, the PM knows which commands
to run in their own project and what to expect in Logseq at each step.

---

## Design Tensions and Open Questions

1. What does the demo directory contain? Both raw input notes (for the PM to extract
   from) and a pre-populated project record (so the PM can see the end state and
   run state/status/export commands immediately)?
   [Decision: BOTH — raw notes for extraction AND pre-populated record so the PM
   can explore the complete end state]

2. Does the demo directory include a Logseq graph directory pre-populated with
   exported pages, or does the PM generate it themselves by running `lucid export`?
   [Tentative: both — pre-exported Logseq pages in the graph dir so the PM can
   open Logseq immediately, AND the walkthrough shows how to regenerate them]

3. Should the walkthrough show `lucid suggest` (requires AI API key) or skip it?
   [Tentative: include a note about it but mark it as optional — the rest of the
   demo should be runnable without an API key]

4. What is the Logseq graph name and directory structure within demo/ — is it
   `demo/logseq/` or a separately named graph directory?

---

## Suspected Dependencies

- F13 (`lucid`): walkthrough uses `lucid` commands throughout
- F2 (`logseq_export`): demo includes exported Logseq pages
- F3 (`logseq_sync`): walkthrough shows sync loop
- All feature modules installed: extraction, state, status, links, export, sync,
  schema, tasks, suggest (optional), report, journal are all demonstrated

---

## Rough Scope Notes

In scope: `demo/` directory containing raw input notes, a pre-populated project
record (events/runtime_events.jsonl), project-schema.yaml, a paired Logseq graph
directory with pre-exported pages, and WALKTHROUGH.md covering the full from-scratch
workflow with Logseq as the primary output interface.

Out of scope: modifying `lucid help`, changing any existing feature behavior,
a "joining existing project" walkthrough path, requiring internet access for the
non-suggest parts of the demo.

---

## Readiness Check

- [x] Problem explains WHY, not HOW
- [x] Primary actor is a human role
- [x] Core outcome from actor's perspective — includes Logseq as key interface
- [x] At least one open question listed
- [x] Suspected dependencies named
- [x] No actor+outcome DBA form present
- [x] No stable guarantees or DBA scope boundaries present
- [x] Feature described without implementation technology
- [x] N/A — F-type, no R-type trigger required

**Brief status**: READY FOR STAGE 1

---

<!-- METADATA -->
brief_created: 2026-06-05
brief_last_updated: 2026-06-05
stage1_started: 2026-06-05

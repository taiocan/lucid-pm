# Feature Brief: F14 — logseq-plugin

**Type**: F-type
**Tier**: 8 — Developer Experience
**Status**: BRIEF-DRAFT

---

## Problem / Need

The PM must leave Logseq and open a terminal to run LucidPM commands (sync, export,
suggest, extract), interrupting their workflow. Logseq is where project data is read
and edited, but all LucidPM operations must be initiated from outside it. The context
switch is friction that accumulates across a working session.

---

## Primary Actor

The PM using Logseq Desktop as their primary project management interface.

---

## Core Outcome (informal)

The PM can trigger LucidPM commands directly from Logseq Desktop without switching to
a terminal. The current Logseq graph is automatically recognized as the active LucidPM
project. Command results — success or error — are visible without leaving Logseq.

---

## Design Tensions and Open Questions

1. **Which commands are worth exposing?**

   Candidates: `sync`, `export`, `suggest`, `extract`. The first three are
   zero-argument operations against the current project — natural fits for a
   slash command. `extract` is different: it takes input (text or a folder path),
   so invoking it from a slash command raises the question of how the PM provides
   that input (typed inline, file picker, clipboard?). Exposing `extract` without
   input handling is incomplete; excluding it narrows the plugin's usefulness for
   new-content ingestion.

   [Tentative: expose sync, export, suggest in v1; treat extract as a stretch goal
   pending a decision on input handling]

2. **How should the plugin resolve the active LucidPM project?**

   Option A — Infer from graph path: Logseq exposes the current graph's root
   directory. If the PM runs LucidPM from the same directory (i.e., the Logseq
   graph IS the LucidPM project), inference is automatic and zero-config. Breaks
   if the graph directory and the LucidPM project directory diverge (e.g., graph
   at `~/logseq/work` but LucidPM data at `~/projects/work`).

   Option B — Explicit plugin setting: the PM configures the project path once in
   plugin settings. Reliable but requires setup; the PM must know and enter the
   path.

   Option C — Both, with inference as default: try inference first; fall back to
   the explicit setting if the inferred path has no LucidPM project data. Most
   flexible but requires defining what "has no LucidPM project data" means
   (presence of `events/runtime_events.jsonl`? a specific config file?).

   [Tentative: Option C — infer by default, explicit setting as override; detection
   via presence of `events/` directory in the inferred path]

3. **How is command output presented?**

   Option A — Modal dialog: blocks until dismissed; fits multi-line output (sync
   report, suggest proposals). Disruptive for quick confirmations.

   Option B — Toast / notification: non-blocking; suits "sync complete (3 items
   updated)" confirmations. Too small for rich output like suggest proposals.

   Option C — Written back into a Logseq block: permanent record; the PM can
   refer back to it. Pollutes the graph with operational noise; requires deciding
   which page or block to write into.

   The tension: command output volumes differ significantly. `sync` emits a
   structured change list. `suggest` emits multi-item AI proposals. A single
   presentation mechanism may not suit all commands.

   [Tentative: modal for all commands in v1 — consistent and handles variable
   output length; toast as a stretch goal for quick-confirmation commands]

4. **Should the plugin ship in this repo or as a separate package?**

   Option A — In this repo under `plugin/`: keeps everything co-located; simpler
   to develop and test against local `lucid`. Installation requires the PM to
   manually load the unpacked plugin in Logseq (developer workflow only).

   Option B — Separate repo / npm package: standard Logseq plugin distribution
   path; installable via Logseq's plugin marketplace without manual steps.
   Introduces a separate release pipeline.

   Option C — Develop in this repo, publish as a separate package: best of both
   for development, but adds publishing overhead.

   The tension: if the intended audience is the PM using this repo (not the general
   Logseq community), manual loading is acceptable and a separate package adds
   cost without benefit. If the plugin is meant for broader distribution, a
   separate package is necessary.

   [Tentative: in this repo under `plugin/` for v1 — audience is users of this
   codebase; marketplace distribution is a future concern]

---

## Suspected Dependencies

- F13 (`lucid`): the plugin shells out to `lucid`; all commands go through the unified
  CLI entry point

---

## Rough Scope Notes

In scope: slash commands for sync, export, suggest, extract; project path resolution
from graph path; output surfacing (at minimum modal or notification); Logseq Desktop
only (requires shell access from plugin sandbox).

Out of scope: Logseq web or mobile; bidirectional sync UI beyond what `lucid sync`
already provides; inline block rendering of command output (optional stretch only).

---

## Readiness Check

- [x] The problem statement explains WHY, not HOW
- [x] The primary actor is a human role, not "the system"
- [x] The core outcome is stated from the actor's perspective
- [x] At least one open question is listed
- [x] Suspected dependencies are named (even if marked uncertain)
- [x] No actor+outcome DBA form appears anywhere in this brief
- [x] No stable guarantees or DBA scope boundaries appear in this brief
- [x] The feature can be described without mentioning implementation technology
- [ ] (R-type only) N/A — F-type

**Brief status**: READY FOR STAGE 1

---

<!-- METADATA -->
brief_created: 2026-06-06
brief_last_updated: 2026-06-06
stage1_started: 2026-06-06

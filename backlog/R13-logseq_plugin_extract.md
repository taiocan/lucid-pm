# Feature Brief: R13 — logseq_plugin extract command

**Type**: R-type  
**Refines**: logseq_plugin (F14)  
**Tier**: 8 — Developer Experience  
**Status**: BRIEF-DRAFT

---

## Problem / Need

Users who work in Logseq Desktop paste unstructured text — meeting notes, raw documents,
copied emails — into Logseq daily journal pages as part of their capture workflow. To
structure that content with LucidPM today, they must leave Logseq, switch to a terminal,
and run `lucid extract` manually. This context switch interrupts the capture-to-structure
loop that LucidPM is supposed to close. The logseq_plugin already delegates Sync, Export,
and Suggest to the companion server — but extraction, the first step in the workflow,
is absent.

**Trigger**: human-approved evolution request.

---

## Primary Actor

The knowledge worker who uses Logseq as their primary text-capture environment and
LucidPM to structure project information from that captured content.

---

## Core Outcome (informal)

After this refinement exists, the user can open a Logseq journal page containing
unstructured notes and trigger LucidPM extraction from within Logseq using a slash
command — without switching to a terminal. The extracted items land in the project
record the same way they would if run from the command line, and the full extraction
output appears as a Logseq notification.

---

## Design Tensions and Open Questions

1. **How does the plugin resolve the filesystem path of the current journal page?**
   Logseq journal pages are stored as files in the vault (e.g., `journals/<date>.md`).
   The plugin can pass this path directly to `lucid extract` without any temp file or
   changes to `pm_structuring`. The question is: does the Logseq API expose the full
   filesystem path of the current page, or only the page title/name? If only the name,
   the plugin would need to reconstruct the path from the vault root and the journal
   naming convention. Stage 1 to verify what the API provides.

2. **What happens when a journal page is extracted more than once?** Two sub-cases:
   (a) the page is extracted again with no changes — should this be a no-op, a warning,
   or allowed to re-run silently? (b) new text is appended to a previously extracted
   journal page and extraction is triggered again — does `lucid extract` deduplicate,
   or will previously extracted items appear twice in the project record? Stage 1/2
   to determine the correct behaviour and whether deduplication is in scope for v1.

3. **What if the current page is not a journal page?** Command is journal-scoped —
   what should the plugin show if the user runs it on a non-journal page?

4. **What if the extraction yields no items?** Notify the user that no items were
   extracted from this page (not a silent no-op, not an error).

---

## Design Decisions Already Made

- **Input**: the current page's vault file is passed as a path to `lucid extract` —
  no raw text extraction, no temp files, no changes to `pm_structuring`
- **Content format**: full Logseq markdown as stored on disk
- **Success notification**: full extraction output (same text `lucid extract` prints)
- **Empty/no-data case**: notification informing user no items were extracted

---

## Suspected Dependencies

- **logseq_plugin (F14)**: the feature being refined — new slash command added here
- **pm_structuring**: `lucid extract` is the underlying command; no changes expected
  since it already accepts file paths — Stage 1 to confirm
- **companion server** (`plugin/server/lucid_plugin_server.py`): receives the page
  file path from the plugin and invokes `lucid extract <path>`

---

## Rough Scope Notes

In scope (rough): one new slash command "LucidPM Extract"; operates only on journal pages;
extracts into the configured project (same `explicit_project_path` or graph-inferred path
logic as existing commands); full output shown in success notification; "no items extracted"
notification when extraction yields nothing; correct behaviour when a journal page is
extracted more than once (re-extraction of unchanged page; re-extraction after appending
new text) — exact behaviour to be defined in Stage 1/2.

Out of scope (rough): extraction on non-journal pages; choosing which project to extract
into at command time; selecting specific blocks; streaming or paginated output; any changes
to existing Sync/Export/Suggest commands.

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
- [x] (R-type only) A valid refinement trigger is identified in the Problem section

**Brief status**: READY FOR STAGE 1

---

<!-- METADATA -->
brief_created: 2026-06-13
brief_last_updated: 2026-06-13
stage1_started:

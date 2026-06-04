# LucidPM — CLAUDE.md

This project uses **Declarative Behavioral Architecture (DBA)** / **Intent-Driven System (IDS)** methodology.

## Toolkit

The DBA toolkit is at `.codeos/` (symlinked from `/home/arc/projects/claude/Codeos`).

**At the start of every Claude Code session:**
1. Read `.codeos/CLAUDE.md` — full DBA operating instructions
2. Read `.codeos/prompts/00-session-start.md` — session orientation template
3. Check the Active Features table below for current stage status
4. Ask what the human wants to work on this session, then STOP and wait

## Project Intent

<!-- Human fills in after running dba-init.sh — what this project exists to do -->
The purpose of this project is to evaluate the functionality of Codeos by developing a simple project management assistant.

## Active Features

<!-- Human maintains this table — update stage and status as work progresses -->

| Feature ID | Description | Current Stage | Status |
|---|---|---|---|
| pm_structuring | structuring non-structured texts into project management structure; folder ingestion with deduplication (R2) | 9 | COMPLETE |
| project_state | cumulative project record across extraction sessions | 8 | COMPLETE |
| item_status | lifecycle status and priority tracking for project record items | 9 | COMPLETE |
| logseq_export | export project record as navigable Logseq pages | 9 | COMPLETE |
| logseq_sync | sync Logseq status/priority changes back into the project record | 9 | COMPLETE |
| multi_project | manage multiple isolated named projects via a shared registry | 9 | COMPLETE |
| priority_view | priority-ranked filtered view of all project record items | 9 | COMPLETE |
| report_export | generate structured project reports in multiple formats | 9 | COMPLETE |
| item_links | typed directed links between project record items | 9 | COMPLETE |
| logseq_export_links | render item_links relationships on Logseq item pages | 9 | COMPLETE |
| ontology_suggest | AI-generated proposals for links, status, and priority enrichment | 9 | COMPLETE |
| journal | free-form notes and meeting minutes in dated txt/md files per project | 9 | COMPLETE |
| R2 (pm_structuring) | folder ingestion with deduplication via event log — --folder mode | 9 | COMPLETE |
| project_schema | schema-driven entity vocabulary, renderer config, deadline universality, alias support | 9 | COMPLETE |
| R4 (item_links) | schema-driven type matrix and relation labels | 9 | COMPLETE |
| R5 (item_status) | schema-driven status vocabulary and task marker mapping | 9 | COMPLETE |
| R6 (pm_structuring) | schema-driven entity types at extraction | 9 | COMPLETE |
| R7 (priority_view) | schema-driven filter validation and unrecognized item exclusion | 9 | COMPLETE |
| R8 (report_export) | schema-driven section grouping, canonical labels, and unrecognized item exclusion | 9 | COMPLETE |
| R9 (logseq_sync) | schema-driven status validation; alias resolution; SyncFailedSchemaInvalid | 9 | COMPLETE |
| R10 (project_state) | schema-driven view: vocabulary-filtered inclusion, canonical type display, SchemaLoadFailed | 9 | COMPLETE |
| R11 (ontology_suggest) | schema-driven proposals: vocabulary-filtered analysis, alias resolution, SchemaLoadFailed, NoRecognizedItems, filtering observability | 9 | COMPLETE |

Stages: 1-Intent / 2-Contract / 3-Schema / 4-Implement / 5-Tests / 6-Observe / 7-Reconcile / 8-Replay / 9-Refine

Status: DRAFT / APPROVED / IN_PROGRESS / COMPLETE

## Human Approval Gates

Every stage output requires explicit human approval before Claude advances.

After presenting any stage output, Claude **STOPS** and states: `AWAITING HUMAN APPROVAL`

Valid approval signals: `APPROVED`, `approved`, `yes proceed`, `lgtm`
Anything else is treated as a revision request.

## Runtime Events

All runtime events are appended to: `events/runtime_events.jsonl`
This file is **append-only**. Claude must never delete or modify existing lines.

## Project-Specific Conventions

<!-- Human fills in any project-specific event prefixes, module names, tech stack -->
Language/runtime: Rust
Test framework: cargo test
Event prefix: Extraction (all pm_structuring events prefixed with Extraction)

# LucidPM — Feature Backlog

> AI-powered structured project knowledge extraction and operational management system with Logseq as a human-facing projection layer.

Each feature has its own file in this folder. This index tracks ordering, dependencies, and status. New backlog entries should be drafted using `.codeos/templates/feature-brief.md` before entering Stage 1.

## Event Spine Note

**No restructuring required for any feature below.**

Existing `pm_structuring` and `project_state` event schemas are untouched. Multi-project isolation is achieved through separate working directories (one `events/runtime_events.jsonl` per project), not by adding project_id to existing events.

---

## Feature Index

| ID | Feature | Tier | Depends On | Event Spine Impact | Status |
|---|---|---|---|---|---|
| F1 | `item_status` | 1 — Foundation | project_state | New schema (new feature) | COMPLETE |
| R1 | `pm_structuring` Stage 9 — proposed status/priority | Refine | F1 | Additive (two nullable fields to ItemsExtracted) | COMPLETE |
| F2 | `logseq_export` | 1 — Foundation | F1, R1 | New schema (new feature) | COMPLETE |
| F3 | `logseq_sync` | 2 — Interaction | F1, F2 | New schema (new feature) | COMPLETE |
| F4 | `multi_project` | 2 — Interaction | none | New schema (new feature) | COMPLETE |
| F5 | `priority_view` | 3 — Analytics | F1 | New schema (new feature) | COMPLETE |
| F6 | `report_export` | 3 — Analytics | F1, F2 | New schema (new feature) | COMPLETE |
| F7 | `item_links` | 4 — Relationships | project_state, F2 | New schema (new feature) | COMPLETE |
| F8 | `logseq_export_links` | 1 — Foundation (F2 Stage 9) | F2, F7 | Additive (no new events) | COMPLETE |
| F9 | `ontology_suggest` | 5 — AI Assistance | project_state, F1, F7 | New schema + reader refinements | COMPLETE |
| F10 | `journal` | 6 — Context & Notes | none | New schema (new feature) | COMPLETE |
| R2 | `pm_structuring` Stage 9 — folder ingestion | Refine | F10 | Additive (source_file field to ItemsExtracted) | COMPLETE |
| R3 | `logseq_export` + `logseq_sync` Stage 9 — canonical Logseq format | Refine | F2, F3 | None (format change only) | COMPLETE |
| F11 | `project_schema` | 1 — Foundation | project_state, item_status, logseq_export, logseq_sync, item_links | None (configuration layer only; additive to 5 existing modules) | COMPLETE |
| R4 | `item_links` Stage 9 — schema-driven type matrix and relation labels | Refine | F7, F11 | None | COMPLETE |
| R5 | `item_status` Stage 9 — schema-driven status vocabulary and task marker mapping | Refine | F1, F11 | None | COMPLETE |
| R6 | `pm_structuring` Stage 9 — schema-driven entity types at extraction | Refine | pm_structuring, F11 | None | COMPLETE |
| R7 | `priority_view` Stage 9 — schema integration | Refine | F5, F11 | None | COMPLETE |
| R8 | `report_export` Stage 9 — schema integration | Refine | F6, F11 | None | COMPLETE |
| R9 | `logseq_sync` Stage 9 — schema-driven status validation | Refine | F3, F11, R5 | None | COMPLETE |
| R10 | `project_state` Stage 9 — schema integration (view) | Refine | project_state, F11 | None | COMPLETE |
| R11 | `ontology_suggest` Stage 9 — schema-driven proposals | Refine | F9, F11, R4, R5 | None | COMPLETE |
| F12 | `task_model` — task persistence and lifecycle sync | 7 — Task Layer | F11, F3, F7 | New schema (TaskAdded, TaskMarkerUpdated, TaskAddFailedParentNotFound, TaskAddFailedSchemaInvalid, TaskAddFailedTaskTypeNotDefined) | COMPLETE |
| F13 | `lucid` — unified CLI entry point | 8 — Developer Experience | all features | None (dispatcher only) | COMPLETE |
| R12 | `lucid` Stage 9 — dispatcher/MODULES sync enforcement | Refine | F13 | None | BACKLOG |
| F14 | `logseq-plugin` — Logseq plugin for LucidPM commands | 8 — Developer Experience | F13 | None (shell invocation only) | BACKLOG |
| F15 | `demo` — self-contained demo project and walkthrough for onboarding | 8 — Developer Experience | F13, all features installed | None (static files only) | COMPLETE |
| R13 | `logseq_plugin` Stage 9 — Extract slash command; invoke `lucid extract` on current journal page from Logseq UI | 8 — Developer Experience | F14 (logseq_plugin) | None (shell invocation only) | BACKLOG |
| R14 | `logseq_plugin` Stage 9 — Workflow step guidance in Extract and Export success messages | Refine | F14 (logseq_plugin) | None | BACKLOG |
| R15 | `logseq_export` Stage 9 — Schema-driven Dashboard.md generation on export | Refine | F2, F11 | None | BACKLOG |
| R16 | `logseq_export` Stage 9 — Suppress unassigned owner wiki-link in task blocks | Refine | F12, F14 | None | BACKLOG |
| F16 | `task_extraction` — extraction creates task records assigned to existing work packages | 7 — Task Layer | pm_structuring (R6), F12, F7, F11 | Additive (new task record creation events from extraction path) | BACKLOG |

Note: F2 depends on R1 so that Logseq export can include AI-proposed values from extraction.

---

## Implementation Recommendation

Build in this order and stop when the system meets your needs:

```
pm_structuring   ✅ COMPLETE
project_state    ✅ COMPLETE
F1  item_status  ✅ COMPLETE
─────────────────────────────────────────────────────────────
R1  pm_structuring Stage 9   ✅ COMPLETE
    item_status Stage 9       ← fallback to proposed values when none set explicitly
─────────────────────────────────────────────────────────────
F2  logseq_export      ✅ COMPLETE
─────────────────────────────────────────────────────────────
F4  multi_project      ✅ COMPLETE
F3  logseq_sync        ✅ COMPLETE
─────────────────────────────────────────────────────────────
F5  priority_view      ✅ COMPLETE
F6  report_export      ✅ COMPLETE
─────────────────────────────────────────────────────────────
F7  item_links         ✅ COMPLETE
F8  logseq_export_links ✅ COMPLETE
─────────────────────────────────────────────────────────────
F9  ontology_suggest   ✅ COMPLETE
─────────────────────────────────────────────────────────────
F10 journal            ✅ COMPLETE
R2  pm_structuring Stage 9  ✅ COMPLETE
─────────────────────────────────────────────────────────────
F11 project_schema     ✅ COMPLETE
─────────────────────────────────────────────────────────────
R4  item_links Stage 9         ✅ COMPLETE
R5  item_status Stage 9        ✅ COMPLETE
─────────────────────────────────────────────────────────────
R6  pm_structuring Stage 9     ✅ COMPLETE
R7  priority_view Stage 9      ✅ COMPLETE
R8  report_export Stage 9      ✅ COMPLETE
─────────────────────────────────────────────────────────────
R9  logseq_sync Stage 9        ✅ COMPLETE
R10 project_state Stage 9      ✅ COMPLETE
─────────────────────────────────────────────────────────────
R11 ontology_suggest Stage 9   ✅ COMPLETE
─────────────────────────────────────────────────────────────
F12 task_model                 ✅ COMPLETE
─────────────────────────────────────────────────────────────
F13 lucid unified CLI          ← single entry point for all features;
                                  bin/lucid exists as pre-DBA draft;
                                  full DBA process required
R12 lucid Stage 9              ← dispatcher/MODULES sync enforcement;
                                  one test/lint rule; depends on F13
─────────────────────────────────────────────────────────────
F14 logseq-plugin              ← invoke LucidPM commands from Logseq Desktop;
                                  JS plugin using child_process → lucid;
                                  depends on F13
R13 logseq_plugin extract      ← LucidPM Extract slash command; passes current
                                  page vault file path to lucid extract via
                                  companion server; depends on F14
R16 logseq_export Stage 9      ← suppress [[TBD]] owner wiki-link on task blocks;
                                  domain predicate is_assigned(); depends on F12
R14 logseq_plugin Stage 9      ← next-step guidance in Extract and Export success
                                  messages; depends on F14
R15 logseq_export Stage 9      ← schema-driven Dashboard.md on export; type slugs
                                  from loaded schema; idempotent; depends on F2, F11
─────────────────────────────────────────────────────────────
F15 demo                       ← self-contained demo project + WALKTHROUGH.md;
                                  covers full workflow including edge cases;
                                  static files only, no code changes
─────────────────────────────────────────────────────────────
F16 task_extraction            ← extraction creates task records (task_model) linked
                                  to existing WPs via item_links; schema-driven types;
                                  unresolvable tasks → unassigned (no WP creation);
                                  depends on pm_structuring R6, F12, F7, F11
```

F1 + R1 + F2 deliver: extraction with AI-suggested state → structured record → live Logseq pages.
That is the complete core loop. Everything else is refinement.

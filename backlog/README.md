# LucidPM — Feature Backlog

> AI-powered structured project knowledge extraction and operational management system with Logseq as a human-facing projection layer.

Each feature has its own file in this folder. This index tracks ordering, dependencies, and status.

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

Note: F2 depends on R1 so that Logseq export can include AI-proposed values from extraction.

---

## Implementation Recommendation

Build in this order and stop when the system meets your needs:

```
pm_structuring   ✅ COMPLETE
project_state    ✅ COMPLETE
F1  item_status  ✅ COMPLETE
─────────────────────────────────────────────────────────────
R1  pm_structuring Stage 9   ← LLM proposes status/priority at extraction time
    item_status Stage 9       ← fallback to proposed values when none set explicitly
─────────────────────────────────────────────────────────────
F2  logseq_export      ← after R1; exports proposed values alongside confirmed ones
─────────────────────────────────────────────────────────────
F4  multi_project      ← when you have a second real project to track
F3  logseq_sync        ← when you want Logseq to be the write interface
─────────────────────────────────────────────────────────────
F5  priority_view      ← if CLI prioritization view is needed
F6  report_export      ← if regular reporting is needed
─────────────────────────────────────────────────────────────
F7  item_links         ← when cross-item relationships need to be tracked and
                          visible in Logseq graph view
F8  logseq_export_links ← render F7 links on Logseq pages (F2 Stage 9 refinement)
─────────────────────────────────────────────────────────────
F9  ontology_suggest   ← on-demand AI enrichment: propose links + status/priority
                          across the full existing record; PM reviews and confirms
─────────────────────────────────────────────────────────────
F10 journal            ← free-form notes and meeting minutes alongside the record;
                          no parsing, no Logseq sync, pure context layer
R2  pm_structuring Stage 9  ← --folder <path> mode: scan journal/ for new .txt/.md
                               files, skip already-ingested (tracked via event log),
                               process new files through existing LLM extraction pipeline
─────────────────────────────────────────────────────────────
F11 project_schema     ← schema-driven entity vocabulary, renderer config,
                          deadline universality, alias support
```

F1 + R1 + F2 deliver: extraction with AI-suggested state → structured record → live Logseq pages.
That is the complete core loop. Everything else is refinement.

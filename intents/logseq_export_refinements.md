# Refinement Log: logseq_export

---

## Refinement 2026-05-29 (R3): Canonical Logseq format — slug filenames, outline blocks, human links

### Trigger

Trigger type: HUMAN_APPROVED_EVOLUTION

### Observed Problem

The database-oriented page format conflicts with Logseq's native conventions:
- UUID filenames make the graph sidebar and search unreadable
- `title::` is redundant (Logseq uses the filename as the page title)
- `## Section` markdown headers break Logseq's outline/collapsing model
- `## Navigation` section is noise — `tags::` and linked references serve this purpose
- `## Description` duplicates the page title
- UUID `[[links]]` in relationship sections are unidentifiable without lookup

### Evidence

User review of the live Logseq graph and comparison against Logseq's canonical conventions,
as documented in `feature_backlog.md` (R3 entry).

### Root Cause

The original export format was designed for database readability (explicit UUIDs, markdown structure)
rather than Logseq's outline-centric, backlink-first model. Each design choice that works in a
relational context (UUID IDs, section headers, redundant title property) actively harms usability
in Logseq's graph and query model.

### Refinement Type

BEHAVIORAL

### Minimal Change

Replace `render_page()`, `render_relationship_sections()`, and extend `cmd_export()` in
`modules/logseq_export/src/main.rs`:
- New `description_to_slug()` and `build_slug_map()` helpers
- Slug-named files instead of UUID-named files
- Canonical page format: `tags::` for type, outline bullet sections, slug-based links
- Stale page cleanup on each export

Artifacts changed:
- [ ] `intents/logseq_export.md` — no change
- [x] `contracts/logseq_export_contract.md` — new page format, slug invariants, stale cleanup postcondition
- [ ] `events/logseq_export_schema.md` — no change
- [x] Implementation in `modules/logseq_export/src/main.rs`
- [x] `tests/behavioral/logseq_export_behavior.rs`
- [x] `tests/behavioral/logseq_export_links_behavior.rs`
- [ ] `tests/replay/` — no change

### Stages Re-run

- [x] Stage 2: Contracts — APPROVED 2026-05-29
- [ ] Stage 3: Event Schema — not re-run (schema unchanged)
- [x] Stage 4: Implementation — APPROVED 2026-05-30
- [x] Stage 5: Tests — APPROVED 2026-05-30
- [x] Stage 6: Runtime Execution — 13 items exported to slug-named pages, UUID pages deleted
- [x] Stage 7: Reconciliation Review — 16 items all ALIGNED
- [x] Stage 8: Replay Verification — 35 events schema-conformant, 15 chains complete, 31/31 replay tests pass

### Validation

- `cargo test --manifest-path modules/logseq_export/Cargo.toml` — 53 tests (22 behavioral + 13 links behavioral + 10 replay + 8 links replay) — all pass
- Runtime export of 13 live items confirmed slug filenames, canonical format, stale UUID page cleanup
- Refinement marked COMPLETE 2026-05-30

---

## Refinement 2026-05-29: Human-readable page titles; remove item-id noise index page

### Trigger

Trigger type: HUMAN_APPROVED_EVOLUTION

### Observed Problem

Screenshots of the live Logseq graph showed three problems:
1. Every item page displayed as "Untitled" in the linked references panel — no human-readable name visible anywhere in the graph
2. An `item-id` index page appeared in the graph with 13 linked references (one per exported item) — a meaningless node with no navigational value
3. Relationship section links rendered as raw UUIDs (`[[372529b8-d713-4688-a8aa-2aa8390a8db0]]`) — unidentifiable without external lookup

### Evidence

Observed directly in Logseq UI screenshots (2026-05-29):
- `status` page: 13 linked references, each headed "Untitled" showing only UUID + metadata
- `item-id` page: 13 linked references from all item pages (caused by `item-id::` property syntax)
- `priority` page: 13 linked references, all "Untitled"
- `stakeholder` type page: linked references showing "Untitled" parent pages

### Root Cause

`render_page()` in `modules/logseq_export/src/main.rs` emitted the following as its first lines:

```
item-id:: {uuid}
type:: {item_type}
status:: {status}
priority:: {priority}
```

Two mechanisms caused the problems:
1. No `title::` property was written — Logseq has no human display name to show in linked references, falling back to "Untitled" (or raw UUID filename depending on version)
2. `item-id::` uses Logseq's double-colon property syntax — Logseq interprets this as a page property and creates an `item-id` index page, populating it with every item page as a linked reference

UUID links in relationship sections are a downstream consequence of (1): once `title::` is present, Logseq resolves `[[uuid]]` links to the human title automatically.

### Refinement Type

BEHAVIORAL

### Minimal Change

Change `render_page()` to emit `title:: {description}` as the first line and replace `item-id::` with a plain-text bullet `- item-id: {uuid}` under a `## Metadata` section.

Artifacts changed:
- [ ] `intents/logseq_export.md` — no change
- [x] `contracts/logseq_export_contract.md` — added two invariant clauses + page format reference block
- [ ] `events/logseq_export_schema.md` — no change
- [x] Implementation in `modules/logseq_export/src/main.rs` — `render_page()` only
- [x] `tests/behavioral/logseq_export_behavior.rs` — added `test_export_page_has_human_readable_title`
- [x] `tests/behavioral/logseq_export_links_behavior.rs` — updated stale `item-id::` assertion
- [ ] `tests/replay/` — no change (replay tests verify event payloads only, not page format)

### Stages Re-run

- [x] Stage 2: Contracts — APPROVED 2026-05-29
- [ ] Stage 3: Event Schema — not re-run (schema unchanged)
- [x] Stage 4: Implementation — APPROVED 2026-05-29
- [x] Stage 5: Tests — APPROVED 2026-05-29
- [x] Stage 6: Runtime Execution — 13 items exported, events captured
- [x] Stage 7: Reconciliation Review — 15 items all ALIGNED
- [x] Stage 8: Replay Verification — 12 events, 6 chains, 0 violations, 10/10 replay tests pass

### Validation

- `cargo test --manifest-path modules/logseq_export/Cargo.toml` — 52 tests (21 behavioral + 13 links behavioral + 10 replay + 8 links replay) — all pass
- Runtime export of 13 live items confirmed `title:: <description>` as first line and `- item-id: <uuid>` as plain text on all pages
- 6 runtime correlation chains in `events/runtime_events.jsonl` — all complete, no broken chains, no undeclared events
- Refinement marked COMPLETE 2026-05-29

---

<!-- Add new refinement entries above this line, newest first -->

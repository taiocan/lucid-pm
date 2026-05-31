# F8: `logseq_export_links`

**Tier**: 1 — Foundation (F2 Stage 9 refinement)
**Depends on**: F2, F7
**Event spine impact**: Additive (no new events)
**Status**: COMPLETE

---

**What this is**
A Stage 9 refinement of F2 (`logseq_export`). No new module, no new CLI, no new events. The existing `logseq_export` binary is extended to read `ItemLinked`/`ItemUnlinked` events from the project event log and append type-specific relationship sections to each item's Logseq page.

**Intent sketch**
When the PM exports the project to Logseq, each item page shows its typed relationships to other items — outgoing links with forward labels, incoming links with inverse labels — so that Logseq's graph view renders the full project dependency map automatically.

**What changes in logseq_export**
- Reads `ItemLinked`/`ItemUnlinked` events to derive the current active link set (mirrors F7 `build_links()` logic)
- For each item page: appends relationship sections grouped by link type
- Forward labels on source-side: Blocks, Affects, Assigned To, Mitigated By, Escalated To, Related To
- Inverse labels on target-side: Blocked By, Affected By, Owns, Mitigates, Escalations, Related To
- Sections omitted entirely if no links of that type exist for the item
- Re-export is idempotent: same links → same sections

**No new events.** `ExportCompleted` payload already carries `items_exported` — no schema change needed.

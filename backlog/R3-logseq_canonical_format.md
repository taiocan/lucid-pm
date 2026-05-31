# R3: `logseq_export` + `logseq_sync` Stage 9 — Canonical Logseq Format

**Tier**: Refine
**Depends on**: F2, F3
**Event spine impact**: None (format change only)
**Status**: COMPLETE

**Trigger type**: HUMAN_APPROVED_EVOLUTION

---

**The problem with the current format**

The current export is database-oriented:

```md
title:: migration to new auth service by June 30
type:: milestone
status:: pending
priority:: high

## Description
migration to new auth service by June 30

## Metadata
- item-id: 09dc05b8-3428-4871-997a-6556b65a4cea

## Navigation
See [[milestone]] for all items of this type.

## Blocked By
- [[c50cc01b-83b9-4cd8-99cc-5222b01dec6a]]
```

Problems:
- `title::` is redundant — Logseq already uses the filename as the page title
- UUID filenames (`09dc05b8-....md`) make the graph sidebar and search unreadable
- UUID `[[links]]` in relationship sections are unidentifiable without lookup
- `## Section` markdown headers break Logseq's outline/collapsing model
- `## Navigation` adds noise — Logseq's linked references and tags already do this
- `## Description` duplicates the page title — same text appears twice

---

**The canonical target format**

Page filename: `migration-to-new-auth-service-by-june-30.md`
(slug derived from description: lowercase, spaces→hyphens, special chars stripped, max 120 chars)

Page content:

```md
type:: milestone
status:: pending
priority:: high
tags:: milestone

- item-id: 09dc05b8-3428-4871-997a-6556b65a4cea

- Blocked by
    - [[write migration runbook before cutover]]

- Affected by
    - [[cyber security audit completion]]
```

Changes from current format:

| Current | Canonical | Reason |
|---|---|---|
| UUID filename `<uuid>.md` | Slug filename `<desc-slug>.md` | Filename = page title = page identity in Logseq |
| `title:: <description>` | Removed | Redundant — filename already is the title |
| `type:: <type>` | Kept | Queryable property |
| `status:: <status>` | Kept | Queryable property + readable by logseq_sync |
| `priority:: <priority>` | Kept | Queryable property + readable by logseq_sync |
| `## Description` + text | Removed | Page title IS the description |
| `## Metadata` + `- item-id: <uuid>` | `- item-id: <uuid>` (top-level bullet) | UUID preserved for sync back-reference |
| `## Navigation / See [[type]]` | `tags:: <item_type>` | Tags enable Logseq queries; no section needed |
| `## <Label>` + `- [[uuid]]` | `- <Label>` indented `- [[slug]]` | Outline indentation vs. markdown headers |
| `[[c50cc01b-...]]` relationship links | `[[write migration runbook before cutover]]` | Human-readable backlinks |

---

**Slug generation rules**

1. Lowercase the full description
2. Replace all non-alphanumeric characters (except hyphens) with hyphens
3. Collapse consecutive hyphens into one
4. Strip leading/trailing hyphens
5. Truncate to 120 characters at a word boundary
6. If two items produce the same slug: append `-2`, `-3` etc. (determined at export time by scanning items in order)

Examples:
- `"migration to new auth service by June 30"` → `migration-to-new-auth-service-by-june-30`
- `"Customer API Migration (v2)"` → `customer-api-migration-v2`
- `"Fix bug!!! ASAP"` → `fix-bug-asap`

---

**Impact on `logseq_export`**

Changes confined to `render_page()` and `cmd_export()`:

1. `render_page()` — new format string: no `title::`, no `## Description`, no `## Navigation`; `tags::` added; relationship sections use indented bullet blocks; links use slug names
2. `cmd_export()` — slug generation before writing; slug map passed to `render_page()` so relationship links reference targets by slug rather than UUID
3. **Stale page cleanup** — `cmd_export()` scans `pages/` for existing `.md` files not in the current slug set and deletes them, preventing UUID-named ghost pages from accumulating

New helpers: `description_to_slug(desc: &str) -> String` and `build_slug_map(items: &[RecordedItem]) -> HashMap<String, String>` (UUID → slug, with collision resolution)

---

**Impact on `logseq_sync`**

Currently finds item pages by UUID filename:
```rust
let page_path = pages_dir.join(format!("{}.md", item.item_id));
```

New discovery approach:
1. Scan all `.md` files in `pages/`
2. For each file, read the `- item-id: <uuid>` bullet line
3. Build a `HashMap<uuid, PathBuf>` (item_id → page file path)
4. Use this map to find each item's page for sync

`parse_page_properties()` is unchanged — still reads `status::` and `priority::`.

New helper: `build_item_page_map(pages_dir: &Path) -> HashMap<String, PathBuf>`

---

**What does NOT change**

- Event schemas for both features — no new events, no payload changes
- `parse_page_properties()` in logseq_sync — `status::` and `priority::` reading unchanged
- `item_id` UUID preserved in every page as a top-level bullet — sync traceability maintained
- `logseq_sync` event emission logic — only page discovery changes

---

**DBA classification**

| Artifact | Feature | Change type |
|---|---|---|
| `contracts/logseq_export_contract.md` | logseq_export | Update page format spec; add stale-page cleanup postcondition |
| `contracts/logseq_sync_contract.md` | logseq_sync | Update page discovery mechanism spec |
| `events/logseq_export_schema.md` | logseq_export | No change |
| `events/logseq_sync_schema.md` | logseq_sync | Possibly add `SyncSkippedItemNotExported` — resolve in Stage 2 |
| `modules/logseq_export/src/main.rs` | logseq_export | `render_page()`, `cmd_export()`, new slug helpers |
| `modules/logseq_sync/src/main.rs` | logseq_sync | Replace UUID filename lookup with page scan + map |
| `tests/behavioral/logseq_export_behavior.rs` | logseq_export | Update all page content assertions |
| `tests/behavioral/logseq_sync_behavior.rs` | logseq_sync | Update page discovery test setup |

Stages re-run for each feature in order: Stage 2 → Stage 4 → Stage 5 → Stage 6 → Stage 7 → Stage 8.
Implement `logseq_export` first — sync depends on export having written slug-named files.

---

**Open design questions for Stage 2**

1. **Stale page cleanup scope**: Delete only UUID-pattern pages, or all pages not in current slug set?
2. **logseq_sync missing-page behavior**: Silent skip with warning, or emit a new `SyncSkippedItemNotExported` event?
3. **tags:: value**: Single tag `tags:: milestone` or list `tags:: milestone, project`?

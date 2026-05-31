# F2: `logseq_export`

**Tier**: 1 — Foundation
**Depends on**: F1, R1
**Event spine impact**: New schema (new feature)
**Status**: COMPLETE

---

**Intent sketch**
The PM can push the current project record (items + statuses) into their Logseq graph as structured pages and blocks. Logseq becomes the primary read interface; the event log remains the write authority. Export is one-way and idempotent — re-running it overwrites previously exported pages.

**What gets written to Logseq**

| Source | Logseq Output |
|---|---|
| Each `task` item | Block on project page with `TODO`/`DOING`/`DONE` marker, `item-id::`, `session::`, `priority::` properties |
| Each `milestone` item | Dedicated page `[[Milestone - <description>]]` with `type:: milestone`, `status::`, `target-date::` |
| Each `risk` item | Block on `[[Risk Register - <project>]]` page with `status::`, `priority::`, `item-id::` |
| Each `issue` item | Block on `[[Issue Log - <project>]]` page with `status::`, `priority::`, `item-id::` |
| Each `stakeholder` item | Page `[[<stakeholder name>]]` with `role:: stakeholder`, `project::` backlink |
| Project summary | Page `[[<project name>]]` with backlinks to all item-type pages and session list |

**Logseq property conventions**
```
item-id:: c3d4e5f6-a7b8-...
type:: task
status:: doing
priority:: high
session:: f76996ec
project:: SmartHome Hub v2
uncertain:: false
```

**Runtime artifact** (must be declared in contract)
Writes `.md` files into the user's Logseq graph directory. Path is supplied at runtime via `--graph` flag. No files are created in the project directory.

**CLI**: `logseq_export --graph /path/to/logseq --project "SmartHome Hub v2"`

**Integration with existing features**
- Reads `project_state` events to enumerate items and sessions
- Reads `item_status` events to get current status per item
- No changes to any existing schema

**Event spine (new)**
```
ExportRequested             ← OBSERVATIONAL
  ↓
  ├─ (no items in record)
  │    ExportFailedEmptyRecord
  │
  └─ (items found)
       ExportCompleted          ← payload: pages_written, items_exported per type
```

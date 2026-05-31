# F10: `journal`

**Tier**: 6 — Context & Notes
**Depends on**: none
**Event spine impact**: New schema (new feature)
**Status**: COMPLETE

---

**Intent sketch**
The PM can write free-form notes, meeting minutes, or journal entries in `.txt` or `.md` files inside a `journal/` folder within the project directory. Entries are dated and browsable — they are context-only artifacts that complement the project record without feeding into it.

LLM-based ingestion of journal files into the project record is handled by **R2** (`pm_structuring` Stage 9 refinement), not by this feature.

**CLI**
```
journal new [--title "Sprint planning"]    # creates journal/YYYY-MM-DD-<slug>.md
journal list                               # lists entry files by date descending
journal open <filename>                    # prints path (for $EDITOR or viewer)
```

**File layout**
```
<project-dir>/
  journal/
    2026-05-28-sprint-planning.md
    2026-05-29-standup.txt
```

**Scope boundary**
- Does NOT parse journal content for items, links, or status — that is R2 + `pm_structuring`
- Does NOT track which files have been ingested — that is R2's responsibility
- Does NOT sync journal entries to Logseq
- Does NOT emit behavioral events that modify the project record

**Event spine (new)**
```
JournalEntryCreated    ← BEHAVIORAL: filename, title, created_at
JournalListRequested   ← OBSERVATIONAL
JournalListReturned    ← BEHAVIORAL: entries[]
```

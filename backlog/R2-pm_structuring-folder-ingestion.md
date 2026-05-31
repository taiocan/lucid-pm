# R2: `pm_structuring` Stage 9 — Folder Ingestion

**Tier**: Refine
**Depends on**: F10
**Event spine impact**: Additive (source_file field to ItemsExtracted)
**Status**: COMPLETE

---

**What this is**
A Stage 9 refinement of `pm_structuring`. Adds a `--folder <path>` mode that scans a directory (e.g., the `journal/` folder created by F10) for `.txt`/`.md` files and processes only files not yet ingested.

**Deduplication mechanism**
The event log is the source of truth. Each `ItemsExtracted` event gains an optional `source_file` field. On each `--folder` run, `pm_structuring` reads the log for prior `ItemsExtracted` events carrying a `source_file` value, builds a set of already-processed filenames, and skips them. No external state file required.

**Schema change (additive)**
`ItemsExtracted.payload.source_file: string | null` — null for interactive stdin sessions, filename for folder-mode runs.

**CLI**
```
pm_structuring --folder journal/            # process all new files in folder
pm_structuring --folder journal/ --yes      # non-interactive (auto-confirm each file)
```

**Scope boundary**
- Does NOT create or manage journal files — that is F10
- Does NOT change the extraction pipeline — same LLM call, same confirmation flow per file
- One `ItemsExtracted` + `ExtractionConfirmed` + `ItemsIncorporated` chain per file

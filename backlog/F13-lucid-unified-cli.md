# F13: `lucid` — Unified CLI entry point

**Tier**: 8 — Developer Experience
**Depends on**: all feature modules
**Event spine impact**: None (dispatcher only — no new events)
**Status**: BACKLOG

---

**Pre-DBA note**

A file `bin/lucid` exists in the repository and is installed by `install.sh`. It was
written outside the DBA process — no intent, contract, or event schema backs it. It is
treated here as an informal draft that may inform Stage 1, but carries no approval
authority. The DBA implementation stage will either confirm it or replace it.

---

**Intent sketch**

The PM can invoke any LucidPM feature through a single `lucid <command>` entry point
without needing to know the names of individual binaries. The dispatcher is the
authoritative CLI surface for LucidPM: its `help` output is complete, every installed
feature is reachable, and it stays in sync with the installed binary set as new features
are added.

---

**Key behaviors**

- `lucid <command> [args]` dispatches to the corresponding feature binary
- `lucid help` lists every available command with a usage example for each
- Every binary in the `install.sh` MODULES list has a corresponding dispatch case
- Adding a new feature to MODULES without updating the dispatcher is detectable
  (lint step or documented coupling convention)
- Unknown commands produce a clear error and point to `lucid help`

---

**Current coverage gap (as of F12)**

| Feature | Binary | lucid command |
|---|---|---|
| pm_structuring | pm_structuring | `extract` ✅ |
| project_state | project_state | `state` ✅ |
| item_status | item_status | `status` ✅ |
| item_links | item_links | `link` ✅ |
| logseq_export | logseq_export | `export` ✅ |
| logseq_sync | logseq_sync | `sync` ✅ |
| multi_project | multi_project | `project` ✅ |
| priority_view | priority_view | `priority` ✅ |
| report_export | report_export | `report` ✅ |
| ontology_suggest | ontology_suggest | `suggest` ✅ |
| journal | journal | `journal` ✅ |
| project_schema | project_schema | `schema` ✅ |
| task_model | task_model | ❌ missing |

---

**Open design questions for Stage 1**

1. Should `lucid` be a bash script (current draft) or a thin Rust binary that provides
   richer error handling, version introspection, and tab completion?
2. How is dispatcher/MODULES sync enforced — comment convention, CI lint, or generated
   at install time?
3. Does `lucid help <command>` delegate to the underlying binary's `--help`, or does
   the dispatcher maintain its own help text?

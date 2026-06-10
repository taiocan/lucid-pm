# L6 — Manual Acceptance Checklist

Run this checklist in real Logseq Desktop after all L1–L4 layers pass.

Prerequisites:
- Plugin loaded as unpacked developer plugin from `plugin/`
- Companion server running in WSL: `python3 plugin/server/lucid_plugin_server.py`
- `explicit_project_path` set to the demo project path (e.g. `/home/arc/projects/claude/LucidPM/demo`)

---

| # | Contract | Check | Expected | Result |
|---|----------|-------|----------|--------|
| 1 | HP6 | Plugin loads | No errors in DevTools console | |
| 2 | HP6 | Slash commands | All three visible in `/` palette: Sync, Export, Suggest | |
| 3 | HP1 | `/LucidPM Sync` | Success notification appears; content matches `lucid sync` output | |
| 4 | FP1 | Clear `explicit_project_path`, use graph not linked to a LucidPM project; invoke `/LucidPM Sync` | `ActiveProjectNotResolved` in error message | |
| 5 | HP6 | Set `explicit_project_path`, restart Logseq, check plugin settings | Path persists across sessions | |

Record pass/fail in the Result column. All five must pass before marking logseq_plugin Stage 9 fully verified.

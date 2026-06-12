# L6 — Manual Acceptance Checklist

Items 1 (plugin loads without errors) and 4 (ActiveProjectNotResolved) are now covered by
the automated L5.5 Electron harness. Only UI-specific behavior that requires real Logseq
Desktop remains here.

Prerequisites:
- Plugin loaded as unpacked developer plugin from `plugin/`
- Companion server running in WSL: `python3 plugin/server/lucid_plugin_server.py`
- `explicit_project_path` set to the demo project path (e.g. `/home/arc/projects/claude/LucidPM/demo`)

---

| # | Contract | Check | Expected | Result |
|---|----------|-------|----------|--------|
| 1 | HP6 | Type `/` inside an editor block | All three slash commands visible: **LucidPM Sync**, **LucidPM Export**, **LucidPM Suggest** | |
| 2 | HP1 | Select **LucidPM Sync** from the `/` palette | Success notification appears; message contains lucid sync output | |
| 3 | HP6 | Set `explicit_project_path`, restart Logseq, run **LucidPM Sync** | Command uses the restored path (not graph inference); sync succeeds — test BEHAVIOR, not settings file | |

All three must pass before updating Stage 6 artifact FP2 to VERIFIED.

Once passed, regenerate Stage 7:
```bash
python3 plugin/scripts/render_reconciliation.py \
  events/logseq_plugin_stage6_observation.json \
  > events/logseq_plugin_stage7_reconciliation.md
```

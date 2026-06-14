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
| 1 | HP6 | Type `/` inside an editor block | All four slash commands visible: **LucidPM Sync**, **LucidPM Export**, **LucidPM Suggest**, **LucidPM Extract** | |
| 2 | HP1 | Select **LucidPM Sync** from the `/` palette | Success notification appears; message contains lucid sync output | |
| 3 | HP6 | Set `explicit_project_path`, restart Logseq, run **LucidPM Sync** | Command uses the restored path (not graph inference); sync succeeds — test BEHAVIOR, not settings file | |
| 4 | R13-HP1 | Open a daily journal page; run **LucidPM Extract** | Success notification appears; notification mentions running **LucidPM Export** to surface items; output from `lucid extract` is visible | |
| 5 | R13-HP2 | Open a journal page with no extractable content; run **LucidPM Extract** | Notification indicates no items were extracted (no Export guidance shown) | |
| 6 | R13-FP1 | Navigate to a non-journal Logseq page; run **LucidPM Extract** | Error notification: page is not a journal page; no extraction performed | |
| 7 | R13-FP2 | Open Logseq with no page selected; run **LucidPM Extract** | Error notification: no page is currently open | |

Items 1–3 must pass before updating Stage 6 artifact FP2 to VERIFIED.
Items 4–7 must pass before updating R13 Stage 6 artifact to VERIFIED.

Once passed, regenerate Stage 7:
```bash
python3 plugin/scripts/render_reconciliation.py \
  events/logseq_plugin_stage6_observation.json \
  > events/logseq_plugin_stage7_reconciliation.md
```

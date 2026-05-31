# F3: `logseq_sync`

**Tier**: 2 — Interaction
**Depends on**: F1, F2
**Event spine impact**: New schema (new feature)
**Status**: COMPLETE

---

**Intent sketch**
The PM makes status changes directly in Logseq (changing `TODO` to `DONE`, editing a `status::` property) and the system detects those changes and writes the corresponding `ItemStatusUpdated` events back into `runtime_events.jsonl`. This closes the loop so Logseq is a true bidirectional interface, not just a read-only view.

**How it works**
- Reads exported Logseq `.md` files, parses task markers and block properties
- Compares current Logseq state against last-known state (derived from `item_status` events in event log)
- For each detected change: emits `ItemStatusUpdated` event
- CLI: `logseq_sync --graph /path/to/logseq` (run after editing in Logseq)

**Logseq alignment**
- Parses `TODO`/`DOING`/`DONE`/`WAITING`/`CANCELLED` markers
- Parses `status::` and `priority::` property changes
- Uses `item-id::` property to correlate blocks back to event log items

**Integration with existing features**
- Depends on F1 (item_status event schema) and F2 (Logseq pages must exist)
- No changes to pm_structuring or project_state schemas

**Event spine (new)**
```
SyncRequested               ← OBSERVATIONAL
  ↓
  ├─ (no changes detected)
  │    SyncCompletedNoChanges
  │
  └─ (changes detected)
       ItemStatusUpdated × N     ← one per changed item (reuses F1 event)
       SyncCompleted             ← payload: changes_detected, items_updated
```

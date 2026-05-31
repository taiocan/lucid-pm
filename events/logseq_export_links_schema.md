# Event Schema: logseq_export_links
# F8 — logseq_export Stage 9 Refinement: Item Link Rendering

<!--
DERIVED FROM:
- intents/logseq_export_links.md
- contracts/logseq_export_links_contract.md

F8 is a purely additive rendering refinement. It introduces no new events.
All F8 contract scenarios are covered by the existing logseq_export event schema.
-->

## Schema Change

**None.** F8 extends the behavior of the `logseq_export` binary but does not
add, remove, or modify any events. Implementation must continue to emit only
the events defined in `events/logseq_export_schema.md`.

## Event Coverage

F8 reuses the following events from `events/logseq_export_schema.md` without modification:

| Event | Category | F8 Relevance |
|---|---|---|
| `ExportRequested` | OBSERVATIONAL | Emitted at the start of every export, unchanged |
| `ExportCompleted` | BEHAVIORAL | Emitted on success; relationship sections are part of the pages written — no payload change needed |
| `ExportFailedEmptyRecord` | FAILURE | Applies before link rendering is reached |
| `ExportFailedOutputUnavailable` | FAILURE | Applies before link rendering is reached |
| `ExportFailedRecordUnreadable` | FAILURE | Applies before link rendering is reached |

## Event Flow

```text
ExportRequested               ← unchanged

  ↓ (record unreadable)
ExportFailedRecordUnreadable  ← unchanged

  ↓ (record empty)
ExportFailedEmptyRecord       ← unchanged

  ↓ (output dir inaccessible)
ExportFailedOutputUnavailable ← unchanged

  ↓ (all preconditions pass — link rendering is part of this path)
ExportCompleted               ← unchanged; pages_written includes pages with relationship sections
```

## Coverage Check

| Contract Scenario | Covered By | Status |
|---|---|---|
| HP1: Outgoing link → forward label on source page | ExportCompleted | COVERED |
| HP2: Incoming link → inverse label on target page | ExportCompleted | COVERED |
| HP3: Item with no links → no relationship sections | ExportCompleted | COVERED |
| HP4: Removed link → not rendered | ExportCompleted | COVERED |
| HP5: Multiple link types → separate sections | ExportCompleted | COVERED |
| HP6: Idempotent re-export | ExportCompleted | COVERED |
| (inherited) EmptyProjectRecord | ExportFailedEmptyRecord | COVERED |
| (inherited) OutputDirectoryNotAccessible | ExportFailedOutputUnavailable | COVERED |
| (inherited) ProjectRecordUnreadable | ExportFailedRecordUnreadable | COVERED |

---

<!-- METADATA -->
status: APPROVED
feature_id: logseq_export_links
approved_by: human
approved_at: 2026-05-27
derived_from_intent: intents/logseq_export_links.md
derived_from_contract: contracts/logseq_export_links_contract.md
extends_schema: events/logseq_export_schema.md

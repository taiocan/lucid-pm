# Event Schema: logseq_export

<!--
DERIVED FROM:
- intents/logseq_export.md
- contracts/logseq_export_contract.md
-->

## Naming Convention

See `.codeos/templates/conventions.md`.

## Required Base Fields (all events)

```json
{
  "event_id": "uuid-v4",
  "event_type": "EventName",
  "timestamp": 1710000000000,
  "correlation_id": "uuid-v4",
  "source_module": "logseq_export",
  "payload": {}
}
```

`correlation_id` is mandatory and must propagate through the entire execution chain.

## Event Definitions

### ExportRequested

- category: OBSERVATIONAL
- emitted when: the PM triggers an export
- payload:
  - `output_dir`: `string` — path to the designated Logseq output directory

### ExportCompleted

- category: BEHAVIORAL
- emitted when: all items from the project record have been successfully written as Logseq pages
- payload:
  - `output_dir`: `string` — path to the Logseq output directory written to
  - `item_count`: `u32` — number of items exported (recognized types only)
  - `pages_written`: `array<string>` — list of page file paths written
  - `items_excluded_type_unknown`: `u32` — number of items skipped because their type was not recognized by the loaded schema (added R10)

### ExportFailedEmptyRecord

- category: FAILURE
- emitted when: the project record contains no items at export time
- payload:
  - `failure_reason`: `string` — `"empty_project_record"`

### ExportFailedOutputUnavailable

- category: FAILURE
- emitted when: the target Logseq output directory is missing or not writable
- payload:
  - `failure_reason`: `string` — `"output_directory_not_accessible"`
  - `output_dir`: `string` — path that was attempted

### ExportFailedRecordUnreadable

- category: FAILURE
- emitted when: the project record source is corrupted or cannot be parsed
- payload:
  - `failure_reason`: `string` — `"project_record_unreadable"`
  - `error_detail`: `string` — description of the parse or read error

## Event Flow

```text
ExportRequested               ← emitted on: PM triggers export

  ↓ (record unreadable)
ExportFailedRecordUnreadable

  ↓ (record empty)
ExportFailedEmptyRecord

  ↓ (output dir inaccessible)
ExportFailedOutputUnavailable

  ↓ (all preconditions pass)
ExportCompleted
```

## Coverage Check

| Contract Failure | Event Here | Status |
|---|---|---|
| EmptyProjectRecord | ExportFailedEmptyRecord | COVERED |
| OutputDirectoryNotAccessible | ExportFailedOutputUnavailable | COVERED |
| ProjectRecordUnreadable | ExportFailedRecordUnreadable | COVERED |

---

<!-- METADATA -->
status: APPROVED
feature_id: logseq_export
approved_by: human
approved_at: 2026-05-25
refined_at: 2026-06-14 (R15: no schema changes — Dashboard.md is a file artifact, not an event)
derived_from_intent: intents/logseq_export.md
derived_from_contract: contracts/logseq_export_contract.md

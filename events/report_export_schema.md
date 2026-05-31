# Event Schema: report_export

<!--
DERIVED FROM:
- intents/report_export.md (actors, outcomes)
- contracts/report_export_contract.md (state transitions, failure modes)
-->

## Naming Convention

See `docs/conventions.md` (source: `.codeos/templates/conventions.md`).

## Required Base Fields (all events)

```json
{
  "event_id": "uuid-v4",
  "event_type": "EventName",
  "timestamp": 1710000000000,
  "correlation_id": "uuid-v4",
  "source_module": "report_export",
  "payload": {}
}
```

`correlation_id` is mandatory and must propagate through the entire execution chain.

## Valid Report Types

`weekly` | `risk-register` | `stakeholders` | `full`

## Event Definitions

### ReportRequested

- category: OBSERVATIONAL
- emitted when: PM initiates report generation
- payload:
  - `report_type`: `string` — the report type requested by the PM
  - `graph_path`: `string | null` — Logseq graph directory path if supplied;
    null when output is directed to stdout

### ReportGenerated

- category: BEHAVIORAL
- emitted when: the report was produced successfully — record has items, report
  type is valid, and graph path (if specified) exists
  (covers happy paths 1, 2, and 3)
- payload:
  - `report_type`: `string` — the report type produced
  - `output_destination`: `string` — `"stdout"` when no graph path was given;
    the absolute graph directory path otherwise
  - `report_file`: `string | null` — path of the markdown file written when
    `output_destination` is a graph path; null when output is stdout
  - `item_count`: `integer` — total number of items included in the report
  - `generated_at`: `integer` — timestamp (ms) at which the report was generated

### ReportFailedEmptyRecord

- category: FAILURE
- emitted when: the project record contains no items
  (contract failure: EmptyRecord)
- payload:
  - `failure_reason`: `string` — always `"empty_record"`

### ReportFailedInvalidType

- category: FAILURE
- emitted when: the requested report type is not one of the valid values
  (contract failure: InvalidReportType)
- payload:
  - `failure_reason`: `string` — always `"invalid_report_type"`
  - `report_type`: `string` — the unrecognised value that was supplied

### ReportFailedOutputNotFound

- category: FAILURE
- emitted when: a graph path was specified but that directory does not exist
  (contract failure: OutputNotFound)
- payload:
  - `failure_reason`: `string` — always `"output_not_found"`
  - `graph_path`: `string` — the path that was not found

## Event Flow

```text
ReportRequested                     ← PM initiates report generation
  ↓
  ├─ (report_type not recognised)
  │    ReportFailedInvalidType
  │
  ├─ (record contains no items)
  │    ReportFailedEmptyRecord
  │
  ├─ (graph_path specified but directory does not exist)
  │    ReportFailedOutputNotFound
  │
  └─ (type valid, record has items, output path exists or not needed)
       ReportGenerated
```

## Coverage Check

| Contract Failure | Event Here | Status |
|---|---|---|
| EmptyRecord | ReportFailedEmptyRecord | COVERED |
| InvalidReportType | ReportFailedInvalidType | COVERED |
| OutputNotFound | ReportFailedOutputNotFound | COVERED |

---
status: APPROVED
feature_id: report_export
approved_by: human
approved_at: 2026-05-27
derived_from_intent: intents/report_export.md
derived_from_contract: contracts/report_export_contract.md

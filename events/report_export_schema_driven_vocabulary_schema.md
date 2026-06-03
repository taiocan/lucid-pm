# Event Schema: report_export_schema_driven_vocabulary

DERIVED FROM:
- intents/report_export_schema_driven_vocabulary.md
- contracts/report_export_schema_driven_vocabulary_contract.md
AMENDS: events/report_export_schema.md

## Design Notes

R8 introduces no new report_export events and no structural payload
changes. The only event-level change is a behavioral amendment to
ReportRequested (vocabulary loading gate). ReportGenerated is unchanged
— its existing `item_count` field already means "items included in the
report," which naturally excludes items not included due to unrecognized
type. No redefinition is needed.

Vocabulary loading occurs before ReportRequested. If vocabulary loading
fails, ReportRequested is not emitted. This does not change the semantics
of ReportRequested — it remains the observational event signalling that
report generation has been accepted and is proceeding.

The observable commitment for unrecognized item exclusion is exactly one
SchemaTypeUnknown event per excluded item (contract postcondition),
sharing the same correlation_id as the report_export invocation. The
contract does not specify ordering relative to ReportGenerated.

## Required Base Fields (all events)

```json
{
  "event_id":       "uuid-v4",
  "event_type":     "EventName",
  "timestamp":      1710000000000,
  "correlation_id": "uuid-v4",
  "source_module":  "report_export",
  "payload":        {}
}
```

`correlation_id` is mandatory and must propagate through the execution
chain.

---

## Behavioral Amendments to Existing Events

### ReportRequested — behavioral amendment (no structural change)

Vocabulary loading occurs before this event is emitted. If vocabulary
loading fails, ReportRequested is not emitted. Payload structure is
unchanged.

---

## Cross-module events relied upon (not emitted by this module)

| Event | Source module | Contract clause |
|---|---|---|
| `SchemaParseError` | `project_schema` | FP1: vocabulary file has a syntax error |
| `SchemaValidationFailed` | `project_schema` | FP1: vocabulary file violates a structural rule |
| `SchemaTypeUnknown` | `project_schema` | HP3/HP4/HP5: exactly one event per excluded item; shares correlation_id with the report_export invocation |

---

## Event Flow

```text
[report_export command]
  ↓
  Vocabulary loading
  ├─ (parse or structural validation error)
  │    <SchemaParseError or SchemaValidationFailed — project_schema>
  │    command exits — ReportRequested not emitted
  │
  └─ (vocabulary loads successfully)
       ↓
       ReportRequested               ← existing; behavioral amendment
       ↓
       ├─ (report_type not recognised)
       │    ReportFailedInvalidType  ← existing; unchanged
       │
       ├─ (record contains no items before exclusion)
       │    ReportFailedEmptyRecord  ← existing; unchanged
       │
       ├─ (graph_path specified but does not exist)
       │    ReportFailedOutputNotFound ← existing; unchanged
       │
       └─ (type valid, record has items, output path ok)
            <SchemaTypeUnknown — project_schema, per excluded item,
             sharing correlation_id; ordering relative to
             ReportGenerated is not specified by this contract>
            ReportGenerated          ← existing; no schema change
```

---

## Coverage Check

| Contract Scenario | Event(s) | Status |
|---|---|---|
| HP1: Full report groups by canonical vocabulary type | Covered by generated report content; no event-schema change required | COVERED — by report output |
| HP2: Risk-register/stakeholders include alias-stored items | Covered by generated report content; no event-schema change required | COVERED — by report output |
| HP3: Unrecognized items excluded; command completes | `SchemaTypeUnknown` (project_schema, per excluded item) + `ReportGenerated` | COVERED |
| HP4: No recognized items for section T → section omitted | `ReportGenerated` (section absent from report content) | COVERED — by report output |
| HP5: All items excluded → empty report, not EmptyRecord | `SchemaTypeUnknown` (per item) + `ReportGenerated` (item_count=0) | COVERED |
| FP1: SchemaInvalid | `SchemaParseError` or `SchemaValidationFailed` — project_schema; ReportRequested not emitted | COVERED — cross-module |

| Contract Failure | Event | Status |
|---|---|---|
| SchemaInvalid | project_schema module events (cross-module) | COVERED |
| EmptyRecord | `ReportFailedEmptyRecord` (existing; unchanged) | COVERED |
| InvalidReportType | `ReportFailedInvalidType` (existing; unchanged) | COVERED |
| OutputNotFound | `ReportFailedOutputNotFound` (existing; unchanged) | COVERED |

---

## Completeness check

- [x] Every contract scenario has at least one event or report-output observable
- [x] Every named failure has exactly one failure event or cross-module signal
- [x] Event flow shows events only — no processing steps
- [x] Coverage Check complete, no MISSING items
- [x] `correlation_id` in required base fields
- [x] No new observable introduced beyond what the approved contract specifies
- [x] Validation ordering not prescribed
- [x] Cross-module events relied upon listed separately from events emitted by this module

---

status: APPROVED
feature_id: report_export_schema_driven_vocabulary
approved_by: human
approved_at: 2026-06-02
derived_from_intent: intents/report_export_schema_driven_vocabulary.md
derived_from_contract: contracts/report_export_schema_driven_vocabulary_contract.md
amends_event_schema: events/report_export_schema.md

# Event Schema: pm_structuring_schema_driven_entity_types

DERIVED FROM:
- intents/pm_structuring_schema_driven_entity_types.md
- contracts/pm_structuring_schema_driven_entity_types_contract.md
AMENDS: events/pm_structuring_schema.md

## Design note

R6 introduces no new pm_structuring events. All changes are behavioral amendments
to existing events plus the cross-module schema failure gate that precedes them.

1. `ItemsExtracted` (existing) — three behavioral amendments:
   a. `item_type` now accepts any vocabulary-recognized type; unrecognized predictions
      receive the sentinel value `"unknown"` (see § Item type representation).
   b. The valid `proposed_status` table is replaced by vocabulary authority (see §
      Valid proposed_status).
   c. `uncertain` and `uncertainty_reason` fields (already present) are used to
      communicate unrecognized type detection.

Schema failures (FP1: SchemaInvalid) are owned by the `project_schema` module and
emitted before any pm_structuring event. `pm_structuring` emits no wrapper schema event.

## Item type representation (Stage 3 decision)

**Unrecognized predicted types** are stored as `item_type: "unknown"` in the
`ItemsExtracted` payload. Rationale:
- Preserves the non-nullable string type of `item_type`, maintaining backward
  compatibility with all downstream parsing
- Clearly signals an unresolvable type without embedding the LLM's unvalidated
  prediction in the record
- The LLM's prediction is preserved in `uncertainty_reason` for PM review

**Alias normalization**: item_type values are stored exactly as produced by the LLM
after vocabulary validation — pm_structuring performs no post-LLM normalization.
The extraction prompt context uses canonical type names from the vocabulary; aliases
are recognized as valid during validation, and the exact string produced by the LLM
(canonical or alias) is stored. This preserves backward compatibility with existing
event logs that use lowercase type names (e.g., "task", "milestone").

Stage 3 decision: aliases are first-class persisted values. Downstream consumers
(item_status, item_links, priority_view, etc.) must treat canonical names and their
aliases as equivalent when reading item_type values from the event log. A consumer
that keys behavior off exact string matching against canonical names only will fail
to recognise alias-stored items.

## Replay note

Extraction outputs are deterministic with respect to (source text, active vocabulary,
LLM response). Given the same extracted LLM response and active vocabulary, type
validation and proposed-status validation always produce the same outcome. Vocabulary
changes after extraction do not alter the stored item_type values in historical
ItemsExtracted events.

## Required Base Fields (all events)

```json
{
  "event_id":       "uuid-v4",
  "event_type":     "EventName",
  "timestamp":      1710000000000,
  "correlation_id": "uuid-v4",
  "source_module":  "pm_structuring",
  "payload":        {}
}
```

`correlation_id` is mandatory and must propagate through the entire execution chain.

---

## Behavioral Amendments to Existing Events

### ItemsExtracted — behavioral amendments (additive only, no structural change)

**Amendment 1 — item_type authority:**

The `item_type` field previously accepted only the hardcoded set
(`task`, `milestone`, `risk`, `issue`, `stakeholder`).

It now accepts any type name recognized by the active vocabulary (canonical name or
alias). When the LLM predicts a type not recognized by the active vocabulary, the
stored value is `"unknown"` rather than the LLM's prediction.

**Amendment 2 — proposed_status authority:**

The static valid-proposed_status table in `events/pm_structuring_schema.md` is
superseded. Proposed status values are now validated against the vocabulary's status
set for the extracted item's type at extraction time:
- If the LLM's proposed_status is in the vocabulary's status set for the item's type
  → stored as proposed_status
- If the LLM's proposed_status is not in that set, or if item_type is `"unknown"`
  → proposed_status stored as null

**Amendment 3 — unrecognized type detection via existing fields:**

Items whose predicted type is not vocabulary-recognized use the existing `uncertain`
and `uncertainty_reason` fields — no new payload fields are introduced:
- `uncertain`: set to `true`
- `uncertainty_reason`: set to a string identifying the unrecognized prediction,
  e.g., `"type not recognized by active vocabulary: <predicted_value>"`

All three amendments apply to both stdin and `--folder` modes. Payload structure
(field names and types) is unchanged.

### TextSubmitted — behavioral amendment (no structural change)

Schema loading occurs before `TextSubmitted` is emitted. If schema loading fails,
`TextSubmitted` is not emitted for that invocation. Payload structure unchanged.

---

## Cross-module events (from project_schema — emitted to same event log)

Schema failures are emitted by the `project_schema` module with
`source_module: "project_schema"`. The built-in default vocabulary ensures
SchemaNotFound cannot occur — only parse and validation failures are possible.

| Event | When emitted |
|---|---|
| `SchemaParseError` | Project schema file present but has a syntax error |
| `SchemaValidationFailed` | Project schema file parses but violates a structural rule |

When either fires, the pm_structuring command does not continue — no `TextSubmitted`
or extraction events are emitted.

Event name verification: `TextSubmitted`, `ItemsExtracted`, `ExtractionConfirmed`,
`ExtractionRejected`, `ExtractionFailedEmptyInput`, `ExtractionFailedNoContent`,
`ExtractionFailedApiRequest`, `FolderScanRequested`, `FolderScanCompleted`,
`ExtractionFailedFolderNotFound` match the canonical names in
`events/pm_structuring_schema.md` exactly.

---

## Event Flow

```text
── stdin extraction ─────────────────────────────────────────────────────────
[extraction command]
  ↓
  Schema loading
  ├─ (schema parse or validation error)
  │    <SchemaParseError or SchemaValidationFailed from project_schema module>
  │    command exits — no pm_structuring event emitted
  │
  └─ (schema loads successfully)
       ↓
       TextSubmitted               ← existing; unchanged
       ↓
       ├─ (empty input)
       │    ExtractionFailedEmptyInput     ← existing; unchanged
       │
       ├─ (no extractable content)
       │    ExtractionFailedNoContent      ← existing; unchanged
       │
       ├─ (API error)
       │    ExtractionFailedApiRequest     ← existing; unchanged
       │
       └─ (items extracted)
            ItemsExtracted                 ← existing; behavioral amendments 1–3
              item_type: vocabulary-recognized name, or "unknown"
              proposed_status: from vocabulary status set, or null
              uncertain/uncertainty_reason: set for unrecognized type items
            ↓
            ├─ (PM confirms)
            │    ExtractionConfirmed       ← existing; unchanged
            │
            └─ (PM rejects)
                 ExtractionRejected        ← existing; unchanged

── --folder extraction ───────────────────────────────────────────────────────
[--folder command]
  ↓
  Schema loading  ← same gate as stdin; schema failure aborts before FolderScanRequested
  ├─ (schema fails) → <project_schema failure event>; command exits
  │
  └─ (schema loads)
       ↓
       FolderScanRequested                 ← existing; unchanged
       ↓
       [per-file pipeline — same amendments apply to each file's ItemsExtracted]
       ↓
       FolderScanCompleted                 ← existing; unchanged
```

---

## Coverage Check

| Contract Scenario | Event(s) | Status |
|---|---|---|
| HP1: Schema vocabulary governs type classification | `ItemsExtracted.item_type` (behavioral amendment 1) | COVERED — by design |
| HP2: Unrecognized predicted type → uncertain + `item_type="unknown"` | `ItemsExtracted` (amendments 1 + 3): `item_type="unknown"`, `uncertain=true`, `uncertainty_reason` set | COVERED |
| HP3: Proposed status from vocabulary status set | `ItemsExtracted.proposed_status` (amendment 2) | COVERED — by design |
| HP4: Proposed status null when type unrecognized | `ItemsExtracted.proposed_status=null` when `item_type="unknown"` | COVERED — by amendment 2 |
| HP5: Out-of-vocabulary proposed status → null | `ItemsExtracted.proposed_status=null` | COVERED — by amendment 2 |
| HP6: Historical item_type preserved | No event change — ItemsExtracted events written once and never modified | COVERED — by invariant |
| FP1: SchemaInvalid aborts before LLM call | `SchemaParseError` or `SchemaValidationFailed` from project_schema; no pm_structuring events | COVERED — cross-module |

| Contract Failure | Event Here | Status |
|---|---|---|
| SchemaInvalid | `project_schema` module events (cross-module) | COVERED |

---

status: APPROVED
feature_id: pm_structuring_schema_driven_entity_types
approved_by: human
approved_at: 2026-06-01
derived_from_intent: intents/pm_structuring_schema_driven_entity_types.md
derived_from_contract: contracts/pm_structuring_schema_driven_entity_types_contract.md
amends_event_schema: events/pm_structuring_schema.md

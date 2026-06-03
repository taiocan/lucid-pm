# Behavioral Contract: report_export_schema_driven_vocabulary

DERIVED FROM: intents/report_export_schema_driven_vocabulary.md
AMENDS: contracts/report_export_contract.md — adds vocabulary-driven
section grouping and labeling, unrecognized item exclusion, and
SchemaInvalid failure path. EmptyRecord, InvalidReportType, and
OutputNotFound failure paths remain in force unchanged. Ordering,
read-only behavior, and output destination logic are unchanged.

## Definitions

**Recognized type** — a type string that matches a canonical type name
or alias defined in the active vocabulary, per the vocabulary's
type-name matching rules.

**Canonical type** — the authoritative name for a recognized type as
defined in the vocabulary. Aliases resolve to the canonical type.

**Section grouping** — the assignment of an item to a report section
based on its resolved canonical type. Items stored under an alias are
grouped with items of the same canonical type. Section headers name
the canonical type.

Note: existing report-type scopes remain unchanged from the base
`report_export` contract. Alias resolution applies when determining
whether an item belongs to a report's existing scope — items stored
under an alias for the scope's target type are included.

## Scenarios

### Happy Path 1: Full report groups items by canonical vocabulary type

```gherkin
Given the active vocabulary is loaded successfully
And the vocabulary defines one or more canonical types
And the project record contains recognized items of at least one type
When the PM requests a full report
Then items are grouped into sections by resolved canonical type
And each section header names the canonical type
And items stored under a vocabulary alias appear in the section for
  their resolved canonical type, not under the alias string
```

### Happy Path 2: Risk-register and stakeholders reports include alias-stored items

```gherkin
Given the active vocabulary is loaded successfully
And the project record contains items whose stored entity type is an
  alias for the report's fixed target canonical type
When the PM requests a risk-register or stakeholders report
Then items whose stored type is the alias are included in the report
And any items whose stored type is the corresponding canonical type are
  also included
And all such items appear in the same report section
```

### Happy Path 3: Unrecognized items excluded; command completes

```gherkin
Given the active vocabulary is loaded successfully
And the project record contains one or more items whose entity type
  is not recognized by the active vocabulary
When the PM requests any report type
Then each unrecognized-type item is excluded from all report content
And exactly one SchemaTypeUnknown signal is produced for each excluded item
And the command completes successfully — exclusion is not a failure
And recognized-type items are included in the report normally
```

### Happy Path 4: Section omitted when no recognized items exist for it

```gherkin
Given the active vocabulary is loaded successfully
And the project record contains no recognized items whose entity type
  resolves to canonical type T
When the PM requests a report that would include a section for type T
Then the section for canonical type T is omitted from the report entirely
And the command completes successfully
And sections for canonical types that have recognized items are
  unaffected
```

### Happy Path 5: All items excluded — empty report produced, not EmptyRecord

```gherkin
Given the active vocabulary is loaded successfully
And every item in the project record has an unrecognized entity type
When the PM requests any report type
Then report content contains no item sections (all sections omitted
  per HP4 applied globally)
And exactly one SchemaTypeUnknown is produced for each excluded item
And EmptyRecord is not triggered — the project record is not empty;
  all its items are unrecognized
```

Note: EmptyRecord fires only when the project record itself contains no
items before any exclusion is applied.

### Failure Path 1: SchemaInvalid

```gherkin
Given the vocabulary file is present but cannot be loaded due to a
  parse or structural validation error
When the PM requests any report type
Then the command fails before any report content is produced
And no output is written to stdout
And no output file is created or modified
  (Stage 2 decision: on vocabulary load failure, file-based output is
  not touched — no empty file is created, no existing file is modified)
And the project record is unchanged
```

## Invariants

- Items are grouped under the canonical type determined by the active
  vocabulary
- Items with entity types not recognized by the active vocabulary are
  absent from all report content
- Alias resolution applies only to item grouping — item content
  (descriptions, status, priority, metadata) is never rewritten
- A vocabulary load failure prevents any report output from being
  produced — no partial output to stdout or to a file
- The report type set and each report type's entity scope are not
  schema-configurable; alias resolution makes each scope vocabulary-aware
- All report scope selection is concept-based, not string-based.
  Whenever a report scope references a vocabulary-defined concept (e.g.,
  risk-register targets the risk concept, stakeholders targets the
  stakeholder concept), the implementation resolves that concept through
  the active vocabulary before comparing against item types. Scope
  concepts are never compared as literal strings — they are resolved to
  their canonical form and then compared canonical-to-canonical. This
  invariant applies to all current scopes and any future report type
  that selects by vocabulary-defined concept.
- All invariants from the existing report_export contract remain in
  force

## Invariant Falsification Scenarios

<!--
Added Stage 9 — 2026-06-03. Retrofitted from regression tests discovered
during Stage 6 runtime execution. Test IDs are not embedded here; the
contract describes behavior. Tests prove it.
-->

| Invariant | Falsifying fixture | Observable when correct | Wrong assumption |
|---|---|---|---|
| All report scope selection is concept-based, not string-based | Vocabulary where the risk concept is canonical `Risk` with alias `risk`; item stored as `risk` | risk-register includes the item; `item_count = 1` | Scope target is hardcoded lowercase — `== Some("risk")` instead of resolving the concept through the vocabulary |
| All report scope selection is concept-based, not string-based | Same vocabulary; weekly report run with item stored as `risk` (alias) | Open Risks section populated | Weekly scope hardcodes `"risk"` for its risk concept rather than resolving through vocabulary |
| Alias resolution applies only to item grouping — content not rewritten | Alias-stored item (`sprint`, resolving to `Task` canonical) in full report | Item's original description, status, and priority unchanged in output | Grouping step rewrites any item field during canonical resolution |
| Vocabulary loading gate precedes `ReportRequested` | Invalid schema YAML present; project record non-empty | `ReportRequested` absent; schema failure event emitted; stdout empty | `ReportRequested` is emitted before vocabulary validation completes |

## Preconditions

- All preconditions from the existing report_export contract apply
- Report generation requires a valid vocabulary; the default vocabulary
  is embedded — SchemaNotFound cannot occur

## Postconditions

- After success: report content contains only items with recognized
  entity types, grouped by canonical type under canonical section
  headers; the project record is unchanged
- After success with exclusions: exactly one SchemaTypeUnknown has been
  produced for each excluded item; the project record is unchanged
- On SchemaInvalid: no report output has been produced; no output file
  has been created or modified; the project record is unchanged
- On EmptyRecord, InvalidReportType, OutputNotFound: unchanged from
  the existing report_export contract

## Runtime Artifacts

No new artifacts are introduced by this feature. Existing artifacts
(the stdout output and the optional `<graph-path>/<report-name>.md`
file) are declared in the base `report_export` contract. The only
behavioral change is that on SchemaInvalid, neither artifact is written
(see FP1 postcondition above).

### Cross-module signals relied upon

| Event | Source module | When relied upon |
|---|---|---|
| `SchemaParseError` | `project_schema` | Vocabulary file has a syntax error (FP1) |
| `SchemaValidationFailed` | `project_schema` | Vocabulary file violates a structural rule (FP1) |
| `SchemaTypeUnknown` | `project_schema` | Item's entity type not recognized by vocabulary (HP3, HP4, HP5) |

## Failure Classifications

| Failure Name | Trigger Condition | Observable Signal |
|---|---|---|
| SchemaInvalid | Vocabulary file present but cannot be loaded | Cross-module vocabulary error from project_schema; no report output produced; no file created or modified |
| EmptyRecord | Project record contains no items before exclusion | ReportFailedEmptyRecord emitted; unchanged from base contract |
| InvalidReportType | --type value not in {weekly, risk-register, stakeholders, full} | ReportFailedInvalidType emitted; unchanged from base contract |
| OutputNotFound | Specified --graph directory does not exist | ReportFailedOutputNotFound emitted; unchanged from base contract |

---

status: APPROVED
feature_id: report_export_schema_driven_vocabulary
approved_by: human
approved_at: 2026-06-02
derived_from_intent: intents/report_export_schema_driven_vocabulary.md
amends_contract: contracts/report_export_contract.md
derived_event_schema: events/report_export_schema_driven_vocabulary_schema.md

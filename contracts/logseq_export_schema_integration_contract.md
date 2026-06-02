# Behavioral Contract: logseq_export_schema_integration

DERIVED FROM: intents/logseq_export_schema_integration.md
AMENDS: contracts/logseq_export_contract.md (adds clauses; does not replace)

## Scenarios

### Happy Path 1: Schema-driven relation labels applied on export

```gherkin
Given the active vocabulary schema defines forward and inverse labels for a relation type
And the project record contains items with links of that relation type
When the PM triggers an export
Then each linked item's page uses the forward label from the schema as the outgoing
  relationship section header
And each linked item's target page uses the inverse label from the schema as the
  incoming relationship section header
And the labels match the vocabulary schema at export time, not any prior hardcoded value
```

### Happy Path 2: Deadline present on every exported page

```gherkin
Given the project record contains one or more items
When the PM triggers an export
Then every exported page includes a deadline property line
And items that have a recorded deadline show that date as the deadline value
And items without a recorded deadline show "TBD" as the deadline value
```

### Happy Path 3: Renamed entity type exported under new name

```gherkin
Given the vocabulary schema defines type Y with type X as an alias
And the project record contains items recorded under type X
When the PM triggers an export
Then those items are exported with their Logseq page type property set to Y
And their pages are otherwise complete — status, priority, deadline, and links present
```

### Happy Path 4: Unrecognized entity type excluded with observable signal

```gherkin
Given the vocabulary schema does not recognize an item's entity type (no alias match)
And the project record contains other items with recognized types
When the PM triggers an export
Then the unrecognized item is excluded from the exported pages
And a SchemaTypeUnknown event is emitted for each excluded item before export proceeds
And all recognized items are exported normally
And an ExportCompleted event is emitted at the end
```

### Failure Path 1: SchemaUnavailable — export aborts

```gherkin
Given the vocabulary schema cannot be loaded or fails validation
When the PM triggers an export
Then a schema FAILURE event is emitted by the vocabulary module
  (SchemaNotFound, SchemaParseError, SchemaValidationFailed, or
  SchemaAliasCollisionDetected — per events/project_schema_schema.md)
And no Logseq pages are written
And no ExportCompleted event is emitted
And the project event log is otherwise unchanged
```

---

## Invariants

- Relation labels in exported pages always match those in the active vocabulary
  schema at export time; hardcoded label values are never used
- Every exported page includes a deadline property line — either a date or "TBD"
- Items excluded due to unrecognized entity type always produce a SchemaTypeUnknown
  event; they are never silently omitted
- The vocabulary schema is loaded before any page is written; a schema failure
  prevents all page writes in that export run
- All invariants from the existing logseq_export contract remain in force

## Preconditions

- All preconditions from the existing logseq_export contract apply
- A loadable and valid project vocabulary definition is accessible

## Postconditions

- Exported pages reference relation labels from the vocabulary schema renderer
- Every exported page carries a deadline property line
- Items with unrecognized entity types are absent from the output and have
  SchemaTypeUnknown events in the event log

## Runtime Artifacts

No new artifacts beyond those declared in the existing logseq_export contract.
The vocabulary schema is read-only at export time and is not written.

## Failure Classifications

| Failure Name | Trigger Condition | Observable Signal |
|---|---|---|
| SchemaUnavailable | Vocabulary schema cannot be loaded or fails structural validation | Schema FAILURE event from `project_schema` module (see events/project_schema_schema.md); no ExportCompleted; no pages written |
| ItemTypeUnrecognized | Item's entity type not in vocabulary schema (no alias match) | SchemaTypeUnknown event per excluded item; export continues for recognized items |

Note: SchemaUnavailable maps to events in the `project_schema` event schema, not a
new `logseq_export` event. The observable signal is the schema event + absence of
ExportCompleted. This is a cross-module observable, consistent with DBA multi-module
event chains.

---

status: APPROVED
feature_id: logseq_export_schema_integration
approved_by: human
approved_at: 2026-05-31
derived_from_intent: intents/logseq_export_schema_integration.md
amends_contract: contracts/logseq_export_contract.md
derived_event_schema: events/logseq_export_schema_integration_schema.md

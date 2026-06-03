# Behavioral Contract: project_state_schema_integration

DERIVED FROM: intents/project_state_schema_integration.md
AMENDS: contracts/project_state_contract.md — replaces the unconstrained view
with vocabulary-filtered inclusion and canonical type display; adds a
SchemaLoadFailed failure path for the view command. Incorporation, EmptyRecord,
and SessionAlreadyIncorporated scenarios from the base contract remain in force
unchanged.

## Definitions

**Recognized type** — a stored type representation that resolves to a concept
defined in the active vocabulary. Recognition is determined by successful
resolution, not by string matching.

**Unrecognized type** — a stored type representation that resolves to no concept
in the active vocabulary.

**Active vocabulary** — the vocabulary used by the view command to determine
whether a stored entity type resolves to a recognized concept and what canonical
name to display for that concept. When no project schema is supplied, the
embedded default vocabulary is active.

## Scenarios

### Happy Path 1: View includes only recognized items

```gherkin
Given the active vocabulary is loaded successfully
And the project record contains items with both recognized and unrecognized
  entity types
When the PM requests the project record
Then only items with recognized entity types are present in the result
And for each excluded item, a non-aborting SchemaTypeUnknown signal is produced
  identifying the item and its unrecognized type
And the view command completes successfully — exclusion is not a failure
And items with recognized entity types are returned with their details
```

### Happy Path 2: Alias items displayed under canonical type name

```gherkin
Given the active vocabulary is loaded successfully
And the vocabulary defines a canonical type T with alias A
And the project record contains an item whose entity type is stored as A
When the PM requests the project record
Then the item is included in the result
And the item's displayed type is T, the canonical name, not A
```

### Happy Path 3: No schema — no additional exclusions

```gherkin
Given no project schema file is supplied
And the project record contains items of default entity types
When the PM requests the project record
Then the view produces no additional exclusions compared to pre-R10 behavior
And no SchemaTypeUnknown signals are produced for previously visible items
```

### Happy Path 4: All items have unrecognized types — empty result, not EmptyRecord

```gherkin
Given the active vocabulary is loaded successfully
And every item in the project record has an unrecognized entity type
When the PM requests the project record
Then the result contains no items
And a SchemaTypeUnknown signal is produced for each excluded item
And no EmptyRecord failure is signalled — EmptyRecord requires the project
  record itself to contain no items before exclusion is applied
```

### Failure Path 1: SchemaLoadFailed

```gherkin
Given the project schema file cannot be loaded (absent, unreadable, or
  structurally invalid)
When the PM requests the project record
Then the view command fails before any output is produced
And a view-command schema failure signal is recorded
And no items are displayed
And the project record is unchanged
```

## Invariants

- **Concept Dependency Invariant:** Item inclusion and exclusion are determined
  by whether the stored entity type resolves to a recognized vocabulary concept —
  only the result of concept resolution governs these decisions; no stored type
  string is compared directly against any vocabulary representation
- **Display invariant:** Display uses the canonical type name, which is the
  representation associated with the resolved concept; an item stored under an
  alias shows the canonical name in view output; this is not a ban on
  representations in display — it is a prescription for *which* representation
  is used
- An item whose entity type resolves to a recognized vocabulary concept is
  always included; an item whose entity type resolves to no concept is always
  excluded
- The absence of a project schema does not cause additional exclusions —
  items visible before R10 remain visible
- A schema load failure always prevents any view output from being produced —
  no partial result is returned
- Unrecognized-type exclusion is always non-aborting — the view command
  completes for all recognized items even when some are excluded; one
  SchemaTypeUnknown signal is produced per excluded item (consistent with the
  per-item precedent established in R7)
- Incorporation is unaffected — stored entity types in the event log are never
  changed; concept resolution applies at read time only
- All invariants from the base project_state contract remain in force

## Vocabulary Dependency

**Vocabulary owner:** project_schema module
**Vocabulary consumer:** project_state view command
**Concepts operated on:** entity type concept identity (for recognition and
exclusion); canonical type name (for display)

## Invariant Falsification Scenarios

| Invariant | Falsifying fixture | Observable when correct | Wrong implementation assumption | Test ID |
|---|---|---|---|---|
| Concept Dependency — canonical type included | Vocabulary defines canonical "Risk" (no alias); item stored as "Risk" | Item included in view result | Resolution logic only traverses alias tables; canonical match not handled → excluded | `test_canonical_type_included_falsifies_alias_only_resolution` |
| Concept Dependency — alias item included | Canonical "Risk", alias "risk"; item stored as "risk"; vocabulary loaded | Item included in view result | `page_types.contains_key("risk")` → not found → excluded | `test_alias_item_included_falsifies_string_comparison` |
| No type name is hardcoded for recognition | Vocabulary defines "Inspector"; item stored as "Inspector" | Item included in view result | Hardcoded type list consulted; "Inspector" absent → excluded | `test_custom_type_in_vocabulary_falsifies_hardcoded_type_list` |
| Display uses canonical, not stored representation | Canonical "Risk", alias "risk"; item stored as "risk" | Displayed type is "Risk" | `display(item.type)` → shows "risk" | `test_display_canonical_falsifies_display_stored_representation` |
| Representation Ban — casing fixture | Canonical "Risk" (uppercase R), alias "risk" (lowercase r); item stored as "risk" | Item recognized and included; displayed as "Risk" | String comparison `item.type == "Risk"` fails for stored "risk" → excluded | `test_representation_ban_falsifies_case_sensitive_comparison` |
| No additional exclusions when no schema | No project schema; item with "task" type (recognized by default vocabulary) | Item included; no SchemaTypeUnknown signal | No schema → empty vocabulary → all items excluded | `test_default_vocabulary_preserves_pre_r10_visibility` |
| Empty view from exclusions ≠ EmptyRecord | All items have unrecognized type → all excluded → empty result | No EmptyRecord failure; SchemaTypeUnknown produced for each excluded item | Empty result triggers EmptyRecord failure path | `test_all_items_excluded_is_not_empty_record_failure` |

## Preconditions

- All preconditions from the base project_state contract apply
- For the view command: vocabulary must be evaluable before any output is
  produced; the default vocabulary is embedded — SchemaNotFound cannot occur
  unless neither project nor default schema is available

## Postconditions

- After successful view: the returned list contains only items with recognized
  entity types; each item's displayed type is the canonical name; a
  SchemaTypeUnknown signal has been produced for each excluded item; the
  project record is unchanged
- On SchemaLoadFailed: no view output has been produced; the project record
  is unchanged

## Runtime Artifacts

| Artifact | Path | Lifecycle |
|---|---|---|
| None beyond events/runtime_events.jsonl | — | — |

### Cross-module signals relied upon

| Event | Source module | When relied upon |
|---|---|---|
| `SchemaTypeUnknown` | project_schema | Produced per excluded item when entity type resolves to no vocabulary concept; non-aborting |
| `SchemaNotFound` / `SchemaParseError` / `SchemaValidationFailed` | project_schema | When schema file is absent or structurally invalid; accompanies the view-command schema failure signal |

## Failure Classifications

| Failure Name | Trigger Condition | Observable Signal |
|---|---|---|
| SchemaLoadFailed | Project schema cannot be loaded (absent, unreadable, or structurally invalid) | View-command schema failure signal from project_state + cross-module project_schema events; no view output produced |

Note on SchemaTypeUnknown ownership: `project_schema` emits `SchemaTypeUnknown`
because it owns vocabulary resolution — the signal records the vocabulary fact
("this type resolves to no concept") rather than the view decision ("this item
is excluded"). The view command reads that signal and responds by excluding the
item; the vocabulary module does not decide what the view does with the
unrecognized type. This separation follows the vocabulary ownership rule and is
consistent with R7. The `emit_type_unknown` function is part of the
project_schema library API; consumers call it at vocabulary-resolution sites.

Note: The view-command schema failure signal from project_state (recording that
the view command failed) accompanies cross-module events from project_schema
(recording why schema loading failed). This is the same two-fact pattern
established in R9: one event records the root cause, the other the business
outcome.

---
status: APPROVED
feature_id: project_state_schema_integration
approved_by: human
approved_at: 2026-06-03
derived_from_intent: intents/project_state_schema_integration.md
amends_contract: contracts/project_state_contract.md
derived_event_schema: events/project_state_schema_integration_schema.md

# Behavioral Contract: priority_view_schema_driven_filters

DERIVED FROM: intents/priority_view_schema_driven_filters.md
AMENDS: contracts/priority_view_contract.md — replaces hardcoded type
and status validation with vocabulary authority; adds unrecognized-item
exclusion; adds SchemaInvalid failure path. Priority filter validation,
ordering logic, and conjunctive filter semantics from the existing
contract remain in force unchanged.

## Definitions

**Recognized type** — a type string that matches a canonical type name
or alias defined in the active vocabulary, per the vocabulary's
type-name matching rules. The vocabulary-loading contract defines those
rules; this contract treats them as authoritative.

**Canonical type identity** — the canonical name of a type as defined
in the vocabulary. Two strings have the same canonical type identity
if they both resolve to the same canonical name (one may be the canonical
name itself; the other may be an alias for it).

**Alias resolution** — filtering operates on canonical type identity.
A filter value that matches either a canonical type name or an alias in
the vocabulary matches all records associated with that canonical type,
regardless of whether each record stores the canonical name or an alias.

**Valid status (filter)** — any status value that appears in the union
of all per-type status sets defined in the active vocabulary. Status
filter validation is not scoped to the type filter in effect.

## Scenarios

### Happy Path 1: Custom vocabulary type filter succeeds

```gherkin
Given the active vocabulary is loaded successfully
And the vocabulary defines a recognized type C (with or without aliases)
And the project record contains items whose entity type resolves to the
  same canonical type identity as C
When the PM requests a priority view with --type C
Then the filter is accepted because C is recognized by the active vocabulary
And only items whose entity type resolves to C under vocabulary
  type-resolution rules are returned
And items whose entity type does not resolve to C are absent from the result
```

### Happy Path 2: Alias filter matching is bidirectional

```gherkin
Given the active vocabulary is loaded successfully
And the vocabulary defines type T with alias A
And the project record contains items stored as "T" and items stored as "A"
When the PM requests a priority view with --type A
Then the filter is accepted because A is recognized by the active vocabulary
And items stored as "A" are included in the result
And items stored as "T" are also included in the result, because "T" and "A"
  share the same canonical type identity
And no item is excluded solely because it stores the canonical name rather
  than the alias used in the filter, or vice versa
```

### Happy Path 3: Unrecognized items excluded; command completes

```gherkin
Given the active vocabulary is loaded successfully
And the project record contains one or more items whose entity type
  is not a recognized type (matches no canonical name or alias)
When the PM requests a priority view (with or without filters)
Then each unrecognized-type item is excluded from the result
And a SchemaTypeUnknown signal is produced for each excluded item
  (identifying the item and its unrecognized type value)
And the command completes successfully — exclusion is not a failure
And items with recognized entity types are returned in normal priority order
```

### Happy Path 4: All items excluded by unrecognized type; empty result

```gherkin
Given the active vocabulary is loaded successfully
And every item in the project record has an unrecognized entity type
When the PM requests a priority view
Then the result is empty
And SchemaTypeUnknown is produced for each excluded item
And no failure is signalled — EmptyRecord is not triggered
  (EmptyRecord requires the project record itself to be empty before any
  exclusion is applied)
```

### Happy Path 5: Status globally valid but inapplicable to filtered type

```gherkin
Given the active vocabulary is loaded successfully
And the vocabulary defines type T with status set S_T
And the vocabulary defines at least one other type U with status set S_U
And a status value V is in S_U but not in S_T
When the PM requests a priority view with --type T and --status V
Then the filter is accepted (V is present in the global vocabulary status
  union and is therefore a valid status filter value)
And the result may be empty (no items of type T have status V)
And no failure is signalled
```

Note: this scenario is intentional. Status filter validation uses the
global vocabulary status union, not a per-type subset. Filtering to
zero results is correct behavior, not an error.

### Failure Path 1: SchemaInvalid

```gherkin
Given the vocabulary file is present but cannot be loaded due to a parse
  or structural validation error
When the PM requests a priority view
Then the command fails before any priority view output is produced
And no item list is returned
And the project record is unchanged
```

### Failure Path 2: InvalidFilter — unrecognized type

```gherkin
Given the active vocabulary is loaded successfully
And the PM supplies a --type filter value that is not recognized by the
  active vocabulary (does not match any canonical name or alias)
When the PM requests a priority view
Then a failure result is returned identifying the invalid type filter
And no item list is returned
And the project record is unchanged
```

### Failure Path 3: InvalidFilter — status not in vocabulary

```gherkin
Given the active vocabulary is loaded successfully
And the PM supplies a --status filter value that does not appear in the
  union of all per-type status sets defined in the active vocabulary
When the PM requests a priority view
Then a failure result is returned identifying the invalid status filter
And no item list is returned
And the project record is unchanged
```

## Invariants

- Filter type validation reflects only the canonical types and aliases
  defined in the active vocabulary; the filter is accepted if and only if
  the supplied type value is recognized by the active vocabulary
- Filter status validation reflects only the status values present in
  the active vocabulary's global status set (union across all types);
  the filter is accepted if and only if the supplied status value appears
  in that union
- Priority filter validation remains hardcoded (high, medium, low) —
  not vocabulary-driven in this release
- Items with entity types not recognized by the active vocabulary are
  always absent from the result, regardless of filters applied
- A vocabulary load failure always prevents any priority view output
  from being produced — no partial result is returned
- When no project vocabulary is supplied, the embedded default vocabulary
  is used; priority view behavior is unchanged for projects using only
  the built-in entity types
- All invariants from the existing priority_view contract remain in force
  (ordering, conjunctive filters, read-only behavior)

## Preconditions

- All preconditions from the existing priority_view contract apply
- A vocabulary must be available before command execution proceeds; the
  default vocabulary is embedded — SchemaNotFound cannot occur

## Postconditions

- After success: the returned list contains only items with recognized
  entity types that satisfy all supplied filters, ordered by priority
  then status; zero or more items may be returned
- After success with exclusions: a SchemaTypeUnknown signal has been
  produced for each excluded item; the project record is unchanged
- On SchemaInvalid: no priority view output has been produced; the
  project record is unchanged
- On InvalidFilter: no item list has been returned; the project record
  is unchanged

## Runtime Artifacts

No new files are created or modified by this feature beyond the shared
event log. However, this contract relies on `SchemaTypeUnknown` being
produced by the project_schema module (source_module: "project_schema")
for each item whose type is not recognized during record processing. That
event is defined in the project_schema event schema and appears in the
shared event log; it is not emitted by priority_view itself.

## Failure Classifications

| Failure Name | Trigger Condition | Observable Signal |
|---|---|---|
| SchemaInvalid | Vocabulary file present but cannot be loaded | Vocabulary error from project_schema module; no priority view output produced; project record unchanged |
| InvalidFilter (type) | --type filter value not recognized by active vocabulary | Failure result identifying invalid type filter; no item list returned |
| InvalidFilter (status) | --status filter value absent from global vocabulary status union | Failure result identifying invalid status filter; no item list returned |

Note: SchemaInvalid maps to SchemaParseError or SchemaValidationFailed
events in the project_schema event schema, emitted by that module. The
observable signal for priority_view is the absence of any priority view
output. This is the same cross-module observable pattern used in R4, R5,
and R6.

Note: SchemaTypeUnknown is emitted by the project_schema module per
excluded item. It is not listed in Failure Classifications because it is
non-aborting — the command continues and returns whatever recognized-type
items remain.

---

status: DRAFT
feature_id: priority_view_schema_driven_filters
approved_by:
approved_at:
derived_from_intent: intents/priority_view_schema_driven_filters.md
amends_contract: contracts/priority_view_contract.md
derived_event_schema: events/priority_view_schema_driven_filters_schema.md

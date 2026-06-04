# Behavioral Contract: ontology_suggest_schema_driven_proposals

DERIVED FROM: intents/ontology_suggest_schema_driven_proposals.md
AMENDS: contracts/ontology_suggest_contract.md — replaces the hardcoded type
matrix with vocabulary authority for link proposal validation; adds
vocabulary-constrained status proposal filtering; adds unrecognized-item
exclusion from the analysis; adds SchemaLoadFailed and NoRecognizedItems
failure paths. All other scenarios from the base contract remain in force
unchanged.

---

## Definitions

**Active vocabulary** — the vocabulary used by the analysis operation to
determine recognized entity types, valid relation types and source/target type
pair constraints, and valid status values per entity type concept. When no
project schema is supplied, the embedded default vocabulary is active.

**Recognized entity type** — an entity type whose stored representation
resolves to a concept in the active vocabulary. Both canonical names and
aliases are valid stored representations; both resolve to the same concept.
An entity type that is present in the stored data but cannot be resolved to
any concept in the active vocabulary — regardless of whether the schema
loaded successfully — is treated as unrecognized for all downstream
evaluation. Vocabulary resolution is performed only against successfully
loaded vocabulary state; a partial or degraded schema state is equivalent
to SchemaLoadFailed for resolution purposes.

**Vocabulary-valid link proposal** — a proposed link whose relation type and
source/target entity type pair both resolve to recognized concepts and whose
combination is permitted by the active vocabulary.

**Vocabulary-valid status proposal** — a proposed status assignment whose
proposed value is present in the vocabulary-defined status set for the item's
entity type concept.

---

## Scenarios

### Happy Path 1: Successful analysis — recognized items analyzed

```gherkin
Given the active vocabulary is loaded successfully
And the project record contains one or more items with recognized entity types
When the PM triggers an AI analysis
Then a proposal set is produced
And every proposal in the set references only items with recognized entity types
And every link proposal in the set is vocabulary-valid
And every status proposal in the set is vocabulary-valid
And the proposal set is accessible by a stable review_id
And the project record is not modified
```

### Happy Path 2: No project schema — analysis behavior unchanged

```gherkin
Given no project schema file is supplied
And the project record and configuration are such that the PM could have
  triggered a successful analysis before R11
When the PM triggers an AI analysis
Then the analysis produces an outcome equivalent to the pre-R11 result for
  the same record
```

### Boundary Scenario 1: Unrecognized items do not block analysis of
recognized items

```gherkin
Given the active vocabulary is loaded successfully
And the project record contains at least one item with a recognized entity type
And the project record also contains one or more items with unrecognized
  entity types
When the PM triggers an AI analysis
Then a proposal set is produced (zero or more proposals)
And no proposal in the set references any item with an unrecognized entity type
And no failure is signalled
```

Note: Analysis proceeds on the recognized subset regardless of how many
unrecognized items are present alongside it.

### Boundary Scenario 2: All generated proposals filtered — still a success

```gherkin
Given the active vocabulary is loaded successfully
And the project record contains one or more items with recognized entity types
And every proposal that would have been generated fails vocabulary validation
  (invalid link types, invalid type pairs, or out-of-vocabulary status values)
When the PM triggers an AI analysis
Then a proposal set with zero proposals is produced
And no failure is signalled
```

Note: Zero proposals resulting from vocabulary filtering is a successful
analysis — it does not trigger NoRecognizedItems or any other failure
condition. The distinction from HP7 (base contract) is that the cause of
zero proposals is post-generation filtering, not the absence of enrichment
opportunities.

### Falsification Scenario: Vocabulary identity equivalence

```gherkin
Given the active vocabulary defines canonical entity type "Risk" with alias
  "risk"
And the project record contains:
  - item A whose entity type is stored as the canonical "Risk"
  - item B whose entity type is stored as the alias "risk"
When the PM triggers an AI analysis
Then both item A and item B are eligible for analysis
And proposals may be generated that reference either item A or item B
```

Falsifies (assumption 1): an implementation that checks eligibility only
against canonical type strings would exclude alias-stored items — "risk"
≠ "Risk" → item B excluded.

Falsifies (assumption 2): a case-sensitive string comparison would exclude
item B even if the vocabulary uses a different casing for the canonical name.

Falsifies (assumption 3): an implementation that resolves aliases on the
inclusion path but uses stored representations on the exclusion path would
produce inconsistent eligibility outcomes across items of the same concept.

### Falsification Scenario: Unrecognized item absent from proposals regardless
of generation content

```gherkin
Given the active vocabulary is loaded successfully
And the project record contains item X with an unrecognized entity type
  alongside one or more items with recognized entity types
When the PM triggers an AI analysis
Then no proposal in the resulting proposal set references item X
```

Falsifies: an implementation that relies solely on pre-filtering the analysis
input but does not validate proposals against recognized item identities
could surface a proposal referencing X if generation produces one for any
reason.

### Failure Path 1: SchemaLoadFailed

```gherkin
Given the project schema file cannot be loaded (absent, unreadable, or
  structurally invalid)
When the PM triggers an AI analysis
Then no proposals are produced
And the project record is not modified
```

### Failure Path 2: NoRecognizedItems

```gherkin
Given the active vocabulary is loaded successfully
And the project record contains one or more items
And no item in the project record has an entity type that resolves to a
  recognized concept
When the PM triggers an AI analysis
Then a failure result is returned indicating no recognized items exist
And no proposals are produced
And the project record is not modified
```

### Failure Path 3: EmptyProjectRecord (unchanged from base contract)

```gherkin
Given the project record contains no items
When the PM triggers an AI analysis
Then a failure result is returned indicating the record is empty
And no proposals are generated
And the project record is not modified
```

Note: EmptyProjectRecord and NoRecognizedItems are observably distinct.
EmptyProjectRecord fires when the record has zero items. NoRecognizedItems
fires when the record has items but none with a recognized entity type. The
two conditions cannot both apply simultaneously.

---

## Invariants

- **Concept Dependency Invariant:** Analysis outcomes — which items are
  eligible, which proposals are surfaced — are invariant under substitution
  of equivalent vocabulary representations. An item stored as alias and an
  item stored as the corresponding canonical name receive identical treatment.
- Proposal vocabulary validity is enforced as a post-generation constraint —
  the proposal set is validated against the active vocabulary after generation,
  and this enforcement must not be assumed from input filtering alone
- Vocabulary validation is applied per-proposal; an invalid proposal is
  discarded individually and does not invalidate the remaining proposal set
- A proposal set of zero items resulting from post-generation vocabulary
  filtering is a successful analysis outcome — it does not trigger
  NoRecognizedItems, EmptyProjectRecord, or any other failure condition
- Failure conditions are evaluated in the following authoritative order during
  analysis: (1) SchemaLoadFailed, (2) EmptyProjectRecord, (3)
  NoRecognizedItems. Evaluation is short-circuiting — exactly one failure
  condition is emitted per analysis attempt, and an AI call is made only when
  all three checks pass.
- No item whose entity type is unrecognized by the active vocabulary appears
  in any proposal in the proposal set
- Items with unrecognized entity types do not prevent analysis from proceeding
  for items with recognized entity types
- Every link proposal in the proposal set is vocabulary-valid — relation type
  and source/target entity type pair are both recognized and permitted by the
  active vocabulary
- Every status proposal in the proposal set carries a status value present in
  the vocabulary-defined status set for the item's entity type concept
- When no project schema is supplied, no items are excluded that were not
  already excluded before R11 — the absence of a schema does not cause
  additional exclusions
- All invariants from the base ontology_suggest contract that this amendment
  does not explicitly supersede remain in force

## Vocabulary Dependency

**Vocabulary owner:** project_schema module
**Concepts operated on:** entity type concept identity (for eligibility
determination); valid relation types and source/target type pair constraints
per vocabulary (established by R4); vocabulary-defined valid status values
per entity type concept (established by R5)
**Concept Dependency Invariant (governing):** Analysis outcomes are invariant
under substitution of equivalent vocabulary representations. Operations
receiving "risk" and "Risk" (equivalent concepts) must produce identical
eligibility and proposal-filtering outcomes.
**Representation Ban invariant (derived):** Because analysis outcomes depend
only on concept identity, vocabulary representations — aliases, canonical
strings, casing conventions, specific type names — must not appear as inputs
to domain decision logic for eligibility determination or proposal validation.
This ban applies to all stages prior to and including proposal validation:
preprocessing, item filtering, candidate selection, and final proposal
validation. It does not apply to display, where the canonical representation
associated with the resolved concept is used.

---

## Invariant Falsification Scenarios

| Invariant | Falsifying fixture | Observable when correct | Wrong implementation assumption | Test ID |
|---|---|---|---|---|
| No unrecognized item appears in any proposal | Vocabulary recognizes "Task"; record has item A (type "Task") and item B (type "Incident"); "Incident" not in vocabulary | No proposal references item B | All items fed to analysis regardless of type; proposals for B may be generated | `test_unrecognized_item_excluded_falsifies_input_only_filtering` |
| Unrecognized items don't block recognized items | Same fixture as above | Proposal set produced (may include proposals for A); no failure signalled | First unrecognized item encountered aborts analysis entirely | `test_unrecognized_items_dont_block_recognized_falsifies_abort_on_unknown` |
| Post-generation enforcement not assumed from input filtering | Vocabulary recognizes "Task"; record has item A (type "Task") only; generation produces a link proposal for A with a relation type not permitted by vocabulary | That link proposal absent from surfaced proposal set | Input filtering (removing unrecognized items) is treated as sufficient; proposal content not validated against vocabulary | `test_per_proposal_validation_not_batch_invalidating_falsifies_batch_rejection` |
| Validation is per-proposal, not batch-invalidating | Vocabulary recognizes "Task"; record has two "Task" items; generation produces one vocabulary-valid status proposal and one vocabulary-invalid link proposal | Valid status proposal present in surfaced set; invalid link proposal absent; no failure signalled | Invalid proposal causes entire proposal batch to be discarded | `test_per_proposal_validation_not_batch_invalidating_falsifies_batch_rejection` |
| Zero proposals after filtering = success | Vocabulary recognizes "Task" with no permitted link types and no status values; record has one "Task" item; generation produces link and status proposals | Proposal set produced with zero proposals; no failure event emitted | Zero proposals from filtering triggers NoRecognizedItems or a generic empty-result failure | `test_zero_proposals_is_success_not_failure_falsifies_empty_result_as_error` |
| Concept Dependency — vocabulary identity equivalence (canonical) | Vocabulary: canonical "Risk", alias "risk"; record has item A (type "Risk"), item B (type "risk") | Both A and B eligible; proposals may reference either | Case-sensitive string comparison; "risk" ≠ "Risk" → B excluded | `test_casing_canonical_and_alias_both_recognized_falsifies_case_sensitive_comparison` |
| Concept Dependency — vocabulary identity equivalence (alias) | Vocabulary: canonical "Risk", alias "hazard"; record has item X (type "hazard") | Item X eligible; proposals may reference X | Eligibility check against canonical strings only; "hazard" ≠ "Risk" → excluded | `test_alias_eligible_same_as_canonical_falsifies_canonical_only_check` |
| Every link proposal is vocabulary-valid | Vocabulary permits relation R only for "Task"→"Milestone"; record has "Task" and "Risk" items; generation produces R from "Task" to "Risk" | That proposal absent from surfaced proposal set | All generation-output proposals surfaced without post-filtering | (LLM integration test — requires live LLM) |
| Every status proposal is vocabulary-valid | Vocabulary defines status set {open, closed} for "Task"; generation proposes status "pending" for a "Task" item | "pending" proposal absent from surfaced proposal set | All generation-output proposals surfaced without status filtering | (LLM integration test — requires live LLM) |
| NoRecognizedItems ≠ EmptyProjectRecord | Record has 3 items, all with unrecognized entity types | NoRecognizedItems failure signalled; EmptyProjectRecord not signalled | Single empty-or-no-eligible path collapses both conditions into EmptyProjectRecord | `test_no_recognized_items_not_empty_record_falsifies_collapsed_condition` |
| Failure evaluation is short-circuiting | Schema file absent; record has 10 recognized items | Exactly one failure event emitted (SchemaLoadFailed); no additional failure events; no proposals produced | Multiple failure conditions accumulated and emitted in sequence | `test_schema_failure_short_circuits_exactly_one_failure_event_emitted` |
| Schema failure evaluated before record-content checks | Schema file absent; record has 10 recognized items | SchemaLoadFailed signalled; no proposals produced | Record checked before schema; analysis proceeds with fallback when schema absent | `test_schema_failure_before_empty_record_check_falsifies_ordering` |
| No schema — no additional exclusions | No project schema supplied; record has item of a type that was visible before R11 | Item eligible for analysis; no additional exclusion | Default vocabulary omits types that were previously included | `test_no_schema_no_additional_exclusions_falsifies_default_empty_vocabulary` |

---

## Preconditions

- All preconditions from the base ontology_suggest contract apply
- If a project schema file is present, it is evaluated before any proposals
  are produced
- For the confirm phase: preconditions are unchanged from the base contract;
  confirm-time type matrix and status validation delegate to item_links (R4)
  and item_status (R5)

## Postconditions

- After a successful analysis: a proposal set exists, identified by review_id;
  every proposal references only items with recognized entity types; every
  link proposal is vocabulary-valid; every status proposal is
  vocabulary-valid; the set may contain zero proposals
- On SchemaLoadFailed: no proposals produced; project record unchanged
- On NoRecognizedItems: no proposals produced; project record unchanged
- On EmptyProjectRecord: unchanged from base contract
- Confirm-phase postconditions are unchanged from the base contract

## Runtime Artifacts

| Artifact | Path | Lifecycle |
|---|---|---|
| None beyond events/runtime_events.jsonl | — | — |

### Cross-module signals relied upon

| Event(s) | Source module | When relied upon |
|---|---|---|
| Schema load failure signals — currently: `SchemaNotFound`, `SchemaParseError`, `SchemaValidationFailed`, `SchemaAliasCollisionDetected` | project_schema | Emitted when schema loading fails before analysis can proceed; analysis additionally emits OntologyReviewFailedSchemaInvalid to record the analysis business outcome |

Note: `ontology_suggest` does not distinguish among `project_schema` failure subtypes —
any schema load failure signal causes the analysis to emit
`OntologyReviewFailedSchemaInvalid` and terminate. If `project_schema` introduces
additional events that cause schema loading to fail before analysis can proceed,
`ontology_suggest` treats them equivalently.

---

## Failure Classifications

| Failure Name | Trigger Condition | Observable Signal |
|---|---|---|
| SchemaLoadFailed | Project schema cannot be loaded (absent, unreadable, or structurally invalid) | OntologyReviewRequested emitted, then OntologyReviewFailedSchemaInvalid emitted; no proposals produced; project record unchanged |
| NoRecognizedItems | Project record contains items but none have an entity type resolving to a recognized vocabulary concept | OntologyReviewRequested emitted, then OntologyReviewFailedNoRecognizedItems emitted; no proposals produced; project record unchanged |
| EmptyProjectRecord | Project record contains no items at analysis time | Unchanged from base contract: OntologyReviewFailedEmptyRecord emitted |
| LLMUnavailable | AI service unreachable or response unparseable | Unchanged from base contract: OntologyReviewFailedLLMUnavailable emitted |
| ReviewNotFound | PM references a review_id not in the project record | Unchanged from base contract: OntologyConfirmFailedReviewNotFound emitted |

---

Note: project_schema emits a schema load failure signal (SchemaNotFound,
SchemaParseError, SchemaValidationFailed, or SchemaAliasCollisionDetected)
when it fails to load the schema. ontology_suggest additionally emits
OntologyReviewFailedSchemaInvalid to record the analysis business outcome.
Both facts are recorded independently.

Note: confirm-time scenarios (HP2–HP8 from the base contract) are unchanged.
Vocabulary-based type matrix and status validation at confirm time are already
enforced by the item_links module (R4) and item_status module (R5). This
amendment does not re-specify confirm behavior.

---

status: APPROVED
feature_id: ontology_suggest_schema_driven_proposals
approved_by: human
approved_at: 2026-06-03
derived_from_intent: intents/ontology_suggest_schema_driven_proposals.md
amends_contract: contracts/ontology_suggest_contract.md
derived_event_schema: events/ontology_suggest_schema_driven_proposals_schema.md

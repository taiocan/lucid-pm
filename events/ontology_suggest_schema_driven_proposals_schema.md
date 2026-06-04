# Event Schema: ontology_suggest_schema_driven_proposals

DERIVED FROM:
- intents/ontology_suggest_schema_driven_proposals.md
- contracts/ontology_suggest_schema_driven_proposals_contract.md

AMENDS: events/ontology_suggest_schema.md — adds
OntologyReviewFailedSchemaInvalid and OntologyReviewFailedNoRecognizedItems
failure events; anchors OntologyReviewProposed zero-proposal semantics;
updates the event flow diagram with two new terminal failure branches. All
event definitions from the base schema remain in force unchanged.

---

## Required Base Fields (all events)

```json
{
  "event_id": "uuid-v4",
  "event_type": "EventName",
  "timestamp": 1710000000000,
  "correlation_id": "uuid-v4",
  "source_module": "ontology_suggest",
  "payload": {}
}
```

`correlation_id` is mandatory and must propagate through the entire execution
chain. The analyse phase and the confirm phase each carry their own
correlation_id (they are separate invocations).

---

## Schema Invariants

These are operational constraints on event emission — not advisory metadata
or documentation. Implementations MUST enforce them.

**Terminal event rule:** Terminal events are mutually exclusive end-states of
the analysis invocation. Emission of a terminal failure event MUST immediately
end analysis-phase execution; no further analysis-phase events may follow
within the same correlation_id after a terminal event is emitted.

**Idempotency rule:** Within a single correlation_id, a terminal failure event
MUST NOT be emitted more than once for the same analysis invocation. Retries
begin a new invocation with a new correlation_id.

**proposal_count integrity:** For every OntologyReviewProposed event,
proposal_count MUST equal the length of the proposals array. These fields are
not independent observables — they describe the same cardinality and MUST
remain strictly synchronized. A divergence (e.g., proposal_count=5,
proposals=[]) is a malformed event.

**filtering integrity:** For every OntologyReviewProposed event,
`generated_count == proposal_count + rejected_count` MUST hold. This strict
equality makes post-generation filtering directly observable: if
`generated_count > proposal_count`, vocabulary filtering discarded
`rejected_count` candidates. If all three are equal, filtering was a no-op
for this invocation. A violation is a malformed event.

---

## New Event Definitions

### OntologyReviewFailedSchemaInvalid

- category: FAILURE
- emitted when: the project schema cannot be loaded at analysis time (absent,
  unreadable, or structurally invalid)
- terminal: this event is terminal for the analysis invocation — no further
  analysis-phase events follow within the same correlation_id
- payload:
  - `failure_reason`: `string` — `"schema_invalid"`
  - `error_detail`: `string` — description of the load failure (file not
    found, parse error, etc.)

Contract trace: Failure Path 1 (SchemaLoadFailed); contract invariant:
"Failure evaluation is short-circuiting — exactly one failure condition is
emitted per analysis attempt."

Note: error_detail parallels the error_detail field in
OntologyReviewFailedLLMUnavailable and provides equivalent diagnostic
coverage for schema-level failures.

### OntologyReviewFailedNoRecognizedItems

- category: FAILURE
- emitted when: the project record contains one or more items but none have
  an entity type that resolves to a recognized concept in the active
  vocabulary
- terminal: this event is terminal for the analysis invocation — no further
  analysis-phase events follow within the same correlation_id
- payload:
  - `failure_reason`: `string` — `"no_recognized_items"`
  - `item_count`: `u32` — total number of items in the project record at
    analysis time, including items with unrecognized entity types

Contract trace: Failure Path 2 (NoRecognizedItems); contract invariant:
"EmptyProjectRecord fires when the record has zero items. NoRecognizedItems
fires when the record has items but none with a recognized entity type."

Note: item_count is always ≥ 1 in this event — if item_count were 0,
OntologyReviewFailedEmptyRecord would have fired first (short-circuit
ordering, contract failure evaluation step 2 precedes step 3).
item_count reflects the total record size, not the count of recognized
items — there are zero recognized items by definition when this event fires.

---

## Unchanged Events — Payload Anchor for R11

The following events are structurally unchanged from events/ontology_suggest_schema.md.
Their payload definitions are reproduced here where R11 contract scenarios
depend on specific payload states that must be machine-verifiable.

### OntologyReviewProposed — payload anchor

This schema extends the base payload (defined in events/ontology_suggest_schema.md)
with two observability fields:

```json
{
  "review_id": "uuid-v4",
  "generated_count": "u32",
  "proposal_count": "u32",
  "rejected_count": "u32",
  "proposals": "array<Proposal>"
}
```

Field definitions:
- `generated_count` — number of candidate proposals successfully parsed from the
  AI response **before** any domain filtering (vocabulary type-pair validation,
  status validation, unrecognized-item exclusion). Parsing failures (malformed
  JSON objects) are excluded from this count.
- `proposal_count` — number of candidates that passed domain filtering and are
  surfaced to the PM.
- `rejected_count` — number of candidates discarded by domain filtering
  (`generated_count - proposal_count`).
- `proposals` — the surfaced candidates (vocabulary-valid at emission time).

Invariants (see Schema Invariants):
- `generated_count == proposal_count + rejected_count` (strict equality)
- `proposal_count == proposals.length`

Observable states:
- **generated_count > proposal_count** — filtering discarded `rejected_count`
  candidates; post-generation enforcement is directly observable
- **generated_count == proposal_count** — filtering was a no-op (all generated
  proposals happened to be vocabulary-valid)
- **proposal_count = 0, proposals = []** — valid success (BS2); not a failure
  condition regardless of `generated_count` value

For the full Proposal object shape, see events/ontology_suggest_schema.md.

### Other unchanged events

| Event | Category | Notes |
|---|---|---|
| OntologyReviewRequested | OBSERVATIONAL | payload unchanged; item_count reflects total record count at trigger time, before schema or vocabulary checks |
| OntologyReviewFailedEmptyRecord | FAILURE | terminal; unchanged |
| OntologyReviewFailedLLMUnavailable | FAILURE | terminal; unchanged |
| OntologyConfirmRequested | OBSERVATIONAL | unchanged |
| OntologyReviewConfirmed | BEHAVIORAL | unchanged |
| OntologyConfirmFailedReviewNotFound | FAILURE | terminal for the confirm invocation; unchanged |

---

## Delegated Events (not owned by this schema)

Unchanged from the base schema:

| Event | source_module | Emitted when |
|---|---|---|
| `ItemLinked` | `item_links` | An accepted link proposal passes confirm-time validation |
| `ItemStatusUpdated` | `item_status` | An accepted status proposal is applied |
| `ItemPriorityUpdated` | `item_status` | An accepted priority proposal is applied |

---

## Event Flow

```text
── ANALYSE PHASE ──────────────────────────────────────────────────────────────

OntologyReviewRequested                ← PM triggers analysis

  Exactly one of the following terminal failure events is emitted (if applicable):

  ├─ (SchemaLoadFailed)
  │    OntologyReviewFailedSchemaInvalid          [terminal]

  ├─ (EmptyProjectRecord)
  │    OntologyReviewFailedEmptyRecord            [terminal]

  ├─ (NoRecognizedItems)
  │    OntologyReviewFailedNoRecognizedItems      [terminal]

  ├─ (LLMUnavailable)
  │    OntologyReviewFailedLLMUnavailable         [terminal]

  └─ (success)
       OntologyReviewProposed
         proposal_count: 0..N
         proposals: [] | [Proposal, ...]

── CONFIRM PHASE (separate invocation) ────────────────────────────────────────

OntologyConfirmRequested               ← PM submits accept/reject decisions

  ├─ (ReviewNotFound)
  │    OntologyConfirmFailedReviewNotFound        [terminal]

  └─ (review found — for each accepted proposal)
       ItemLinked            (source_module: item_links)
       ItemStatusUpdated     (source_module: item_status)
       ItemPriorityUpdated   (source_module: item_status)

OntologyReviewConfirmed                ← after all accepted proposals processed
```

See Schema Invariants for the operational rules governing terminal events,
idempotency, and proposal_count integrity. The four analysis-phase failure
branches are mutually exclusive; branch layout reflects the contract-specified
short-circuit ordering without prescribing it beyond what the contract defines.

---

## Coverage Check

### New scenarios (R11)

| Contract Scenario | Events | Status |
|---|---|---|
| HP1: Recognized items analyzed, vocabulary-valid proposals produced | OntologyReviewRequested → OntologyReviewProposed (proposal_count ≥ 1) | COVERED |
| HP2: No project schema — behavior unchanged | OntologyReviewRequested → OntologyReviewProposed | COVERED (default vocabulary active; no new events needed) |
| BS1: Unrecognized items do not block analysis of recognized items | OntologyReviewRequested → OntologyReviewProposed (proposal_count ≥ 0) | COVERED |
| BS2: All proposals filtered — zero proposals, success | OntologyReviewRequested → OntologyReviewProposed (proposal_count=0, proposals=[]) | COVERED (anchored explicitly above) |
| FS: Vocabulary identity equivalence | OntologyReviewRequested → OntologyReviewProposed | COVERED (behavioral constraint; no new event) |
| FS: Unrecognized item absent from proposals | OntologyReviewRequested → OntologyReviewProposed | COVERED (behavioral constraint; no new event) |
| FP1: SchemaLoadFailed | OntologyReviewRequested → OntologyReviewFailedSchemaInvalid | COVERED (new terminal event) |
| FP2: NoRecognizedItems | OntologyReviewRequested → OntologyReviewFailedNoRecognizedItems | COVERED (new terminal event) |
| FP3: EmptyProjectRecord | OntologyReviewRequested → OntologyReviewFailedEmptyRecord | COVERED (base terminal event; unchanged) |

### Base scenarios (coverage unchanged from ontology_suggest_schema.md)

| Contract Scenario | Events | Status |
|---|---|---|
| HP: Link proposal accepted | OntologyConfirmRequested → ItemLinked → OntologyReviewConfirmed | COVERED |
| HP: Status proposal accepted | OntologyConfirmRequested → ItemStatusUpdated → OntologyReviewConfirmed | COVERED |
| HP: Priority proposal accepted | OntologyConfirmRequested → ItemPriorityUpdated → OntologyReviewConfirmed | COVERED |
| HP: Proposals rejected | OntologyConfirmRequested → OntologyReviewConfirmed (accepted_count=0) | COVERED |
| HP: Partial acceptance | OntologyConfirmRequested → (delegated events) × N → OntologyReviewConfirmed | COVERED |
| HP: Confirm old review | OntologyConfirmRequested (prior review_id) → OntologyReviewConfirmed | COVERED |
| FP: LLMUnavailable | OntologyReviewRequested → OntologyReviewFailedLLMUnavailable | COVERED |
| FP: ReviewNotFound | OntologyConfirmRequested → OntologyConfirmFailedReviewNotFound | COVERED |

---

<!-- METADATA -->
status: APPROVED
feature_id: ontology_suggest_schema_driven_proposals
approved_by: human
approved_at: 2026-06-03
derived_from_intent: intents/ontology_suggest_schema_driven_proposals.md
derived_from_contract: contracts/ontology_suggest_schema_driven_proposals_contract.md
amends_schema: events/ontology_suggest_schema.md

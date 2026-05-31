# Behavioral Contract: ontology_suggest

<!--
DERIVED FROM: intents/ontology_suggest.md
-->

## Source Module Compatibility Decision

Accepted proposals produce behavioral events (ItemLinked, ItemStatusUpdated,
ItemPriorityUpdated) emitted with the source_module of the owning feature
("item_links" and "item_status" respectively). This makes confirmed proposals
indistinguishable to all downstream readers from manually-entered changes —
satisfying the intent guarantee — without requiring changes to any existing
reader. The OntologyReviewConfirmed event (source_module: "ontology_suggest")
provides the full audit trail of what was AI-proposed and which proposals were
accepted or rejected.

---

## Scenarios

### Happy Path 1: Proposals Generated

```gherkin
Given the project record contains one or more items
When the PM triggers an AI analysis of the project record
Then a set of enrichment proposals is produced
And each proposal has a unique proposal_id and a rationale string
And each proposal is one of: a link between two items, a status assignment, or a priority assignment
And proposals for links conform to the established type matrix (only valid source/target type pairs)
And the proposal set is accessible by a stable review_id
And the project record is not modified
```

### Happy Path 2: Link Proposal Accepted

```gherkin
Given an analysis has produced a proposal to add a link from item A to item B with link_type T
And that link does not already exist in the project record
And both items exist in the project record
When the PM confirms acceptance of that proposal
Then the link is recorded in the project record
And querying item A shows the link under its forward label
And querying item B shows the link under its inverse label
And a confirmation record is produced that includes this proposal_id in the accepted list
```

### Happy Path 3: Status Proposal Accepted

```gherkin
Given an analysis has produced a proposal to set item X's status to S
When the PM confirms acceptance of that proposal
Then item X's current status becomes S
And a confirmation record is produced that includes this proposal_id in the accepted list
```

### Happy Path 4: Priority Proposal Accepted

```gherkin
Given an analysis has produced a proposal to set item X's priority to P
When the PM confirms acceptance of that proposal
Then item X's current priority becomes P
And a confirmation record is produced that includes this proposal_id in the accepted list
```

### Happy Path 5: Proposals Rejected

```gherkin
Given an analysis has produced one or more proposals
When the PM confirms rejection of all proposals
Then the project record is unchanged
And a confirmation record is produced with all proposal_ids in the rejected list
```

### Happy Path 6: Partial Acceptance

```gherkin
Given an analysis has produced multiple proposals
When the PM confirms acceptance of a subset and rejection of the rest
Then only the accepted proposals are applied to the project record
And the rejected proposals produce no change to the project record
And the confirmation record distinguishes accepted from rejected proposal_ids
```

### Happy Path 7: No Proposals Generated

```gherkin
Given the project record contains one or more items
And the AI analysis finds no enrichment opportunities
When the PM triggers an analysis
Then a proposal set with zero proposals is produced
And no failure is signalled
And the project record is not modified
```

### Happy Path 8: Confirm a Previous Review

```gherkin
Given an analysis was performed earlier and produced proposals with review_id R
And a more recent analysis has also been performed
When the PM confirms proposals from review R by supplying review_id R
Then exactly the proposals from review R are considered
And proposals from other reviews are not affected
```

### Failure Path 1: EmptyProjectRecord

```gherkin
Given the project record contains no items
When the PM triggers an analysis
Then a failure result is returned indicating the record is empty
And no proposals are generated
And the project record is not modified
```

### Failure Path 2: LLMUnavailable

```gherkin
Given the project record contains one or more items
And the AI service is unreachable or returns a response that cannot be parsed into proposals
When the PM triggers an analysis
Then a failure result is returned indicating the AI service is unavailable
And no proposals are generated
And the project record is not modified
```

### Failure Path 3: ReviewNotFound

```gherkin
Given no analysis has been performed with review_id R
When the PM attempts to confirm proposals from review R
Then a failure result is returned indicating that review_id R does not exist
And the project record is not modified
```

---

## Invariants

- The project record is never modified by the analysis phase — proposals are
  read-only outputs; changes only occur in the confirm phase
- A proposal is applied only if the PM explicitly includes its proposal_id in
  the accepted list — unmentioned proposals are treated as neither accepted
  nor rejected and remain available for a future confirm invocation
- A link proposal that fails validation at confirm time (item no longer exists,
  link already exists, or type matrix violation) is skipped and recorded as
  skipped in the confirmation record; other accepted proposals in the same
  confirm invocation are still applied
- The analysis filters out any link proposals that violate the type matrix
  before surfacing them to the PM — invalid combinations never appear in the
  proposal set
- Proposals from a review remain available for confirmation until explicitly
  rejected; running a new analysis does not invalidate previous reviews
- Accepted proposals produce behavioral events identical in structure to those
  produced by the owning tools (item_links, item_status)

## Preconditions

- The project record exists and is readable
- For confirm: the referenced review_id must correspond to a prior completed
  analysis visible in the project record

## Postconditions

- After a successful analysis: a proposal set exists, identified by review_id,
  containing zero or more proposals each with a unique proposal_id and rationale
- After a successful confirm: for each accepted proposal, exactly one
  corresponding behavioral event has been appended to the project record;
  a confirmation record exists recording accepted and rejected proposal_ids

## Runtime Artifacts

| Artifact | Path | Lifecycle |
|---|---|---|
| Event log | events/runtime_events.jsonl | Append-only; OntologyReviewRequested, OntologyReviewProposed, OntologyConfirmRequested, OntologyReviewConfirmed, and delegated behavioral events appended per operation |

No files are created or modified outside the event log.

## Failure Classifications

| Failure Name | Trigger Condition | Observable Signal |
|---|---|---|
| EmptyProjectRecord | Project record contains no items at analysis time | `OntologyReviewFailedEmptyRecord` emitted |
| LLMUnavailable | AI service unreachable or response unparseable | `OntologyReviewFailedLLMUnavailable` emitted |
| ReviewNotFound | PM references a review_id not in the project record | `OntologyConfirmFailedReviewNotFound` emitted |

---

<!-- METADATA -->
status: APPROVED
feature_id: ontology_suggest
approved_by: human
approved_at: 2026-05-28
derived_from_intent: intents/ontology_suggest.md
derived_event_schema: events/ontology_suggest_schema.md

# Behavioral Contract: project_state

<!--
PURPOSE OF THIS FILE:
Defines observable truths derived from the approved intent.
Contracts describe OBSERVABLE behavior, not internal logic.
Every clause must be independently testable.
This file must be APPROVED before Stage 4 (implementation) begins.

DERIVED FROM: intents/project_state.md
-->

## Scenarios

### Happy Path 1: Incorporate Confirmed Extraction into Project Record

```gherkin
Given the project record exists (empty or containing prior items)
When the PM incorporates items from a confirmed extraction session
Then all confirmed items from that extraction are added to the project record
And each new item is associated with the session identifier of that extraction
And previously recorded items are unchanged
```

### Happy Path 2: View Project Record

```gherkin
Given the project record contains items from at least one incorporated extraction
When the PM requests the project record
Then all recorded items are presented
And each item is displayed with its originating session identifier
```

### Failure Path 1: EmptyRecord

```gherkin
Given no confirmed extractions have been incorporated into the project record
When the PM requests the project record
Then the PM is informed the record is empty
And no items are displayed
```

### Failure Path 2: SessionAlreadyIncorporated

```gherkin
Given the project record already contains items from extraction session S
When the PM attempts to incorporate session S again
Then the project record is unchanged
And the PM is informed that session S has already been incorporated
```

## Invariants

- Only items from PM-confirmed extractions are ever present in the project record
- Every item in the project record carries the session identifier of its originating extraction
- Incorporating a new extraction never alters or removes previously recorded items

## Preconditions

- A project record exists (created on first use if not already present)

## Postconditions

- After successful incorporation: all confirmed items from the extraction appear
  in the project record, each associated with their session identifier
- After viewing: no state change occurs — viewing is read-only

## Failure Classifications

| Failure Name | Trigger Condition | Observable Signal |
|---|---|---|
| EmptyRecord | PM queries record before any extraction has been incorporated | PM informed record is empty; no items returned |
| SessionAlreadyIncorporated | PM incorporates a session already present in the record | Record unchanged; PM informed session already incorporated |

---

<!-- METADATA -->
status: APPROVED
feature_id: project_state
approved_by:
approved_at:
derived_from_intent: intents/project_state.md
derived_event_schema: events/project_state_schema.md

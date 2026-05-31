# Behavioral Contract: priority_view

## Scenarios

### Happy Path 1: Unfiltered view

```gherkin
Given the project record contains items with varying priorities and statuses
When the PM requests a priority view with no filters
Then a list of all items in the record is returned
And items with an explicit priority appear before items with no priority set
And within the same priority level, items at an active status appear before
    items at an initial or pending status
And items at the same priority and status appear in a stable, consistent order
```

### Happy Path 2: Filtered view

```gherkin
Given the project record contains items of varying types, statuses, and priorities
When the PM requests a priority view with one or more filters applied
Then only items matching every specified filter are returned
And the returned items are ordered by priority then by status,
    identical to the unfiltered ordering rules
```

### Happy Path 3: Filtered view with no matching items

```gherkin
Given the project record contains items
When the PM requests a priority view with filters that match no items
Then an empty list is returned
And no failure is signalled
```

### Failure Path 1: EmptyRecord

```gherkin
Given the project record contains no items
When the PM requests a priority view
Then a failure result is returned indicating the record is empty
And no item list is returned
And the project record remains unchanged
```

### Failure Path 2: InvalidFilter

```gherkin
Given the project record contains items
When the PM requests a priority view supplying a filter value that is not a
    recognised item type, status value, or priority level
Then a failure result is returned identifying the invalid filter
And no item list is returned
And the project record remains unchanged
```

## Invariants

- Items with an explicit priority always rank before items with no priority set,
  regardless of any filter applied
- When priority is equal, active/in-progress status ranks before initial/pending status
- Filters are conjunctive: an item must satisfy every specified filter to appear
- The view never modifies any item's status, priority, or any other field

## Preconditions

- The project record exists
- The project record contains at least one item (required for non-failure execution)

## Postconditions

- A list of items is returned, ordered by priority then by status
- Every item in the returned list satisfies all specified filters
- No item in the project record has been modified

## Runtime Artifacts

| Artifact | Path | Lifecycle |
|---|---|---|
| (none beyond events/runtime_events.jsonl) | — | — |

## Failure Classifications

| Failure Name | Trigger Condition | Observable Signal |
|---|---|---|
| EmptyRecord | Project record contains no items | PriorityViewFailedEmptyRecord emitted |
| InvalidFilter | Filter value is not a recognised type, status, or priority level | PriorityViewFailedInvalidFilter emitted |

---
status: APPROVED
feature_id: priority_view
approved_by: human
approved_at: 2026-05-26
derived_from_intent: intents/priority_view.md
derived_event_schema: events/priority_view_schema.md

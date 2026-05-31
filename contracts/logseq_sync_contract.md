# Behavioral Contract: logseq_sync

<!--
DERIVED FROM: intents/logseq_sync.md
-->

## Scenarios

### Happy Path 1: Successful Sync With Changes

```gherkin
Given the project record contains one or more items
And the Logseq graph is accessible and contains pages for those items
And one or more pages reflect a status or priority different from the project record
When the PM triggers a sync
Then a SyncRequested event is emitted
And a status update is recorded for each item whose status changed in Logseq
And a priority update is recorded for each item whose priority changed in Logseq
And a SyncCompleted event is emitted reporting the total number of changes applied
And existing events in the project event log are unchanged
```

### Happy Path 2: Sync With No Changes (Idempotent)

```gherkin
Given the project record contains one or more items
And the Logseq graph is accessible and reflects the same status and priority
  as the project record for all items
When the PM triggers a sync
Then a SyncRequested event is emitted
And a SyncCompletedNoChanges event is emitted
And no status or priority update events are emitted
And existing events in the project event log are unchanged
```

### Failure Path 1: GraphNotAccessible

```gherkin
Given the Logseq graph directory is missing or cannot be read
When the PM triggers a sync
Then a SyncRequested event is emitted
And a SyncFailedGraphNotAccessible event is emitted
And no changes are made to the project record
And existing events in the project event log are unchanged
```

### Failure Path 2: ProjectRecordEmpty

```gherkin
Given the project record contains no items
When the PM triggers a sync
Then a SyncRequested event is emitted
And a SyncFailedEmptyRecord event is emitted
And no changes are made to the project record
```

### Failure Path 3: InvalidStatusForType (item-level, sync continues)

```gherkin
Given the project record contains one or more items
And the Logseq graph is accessible
And a Logseq page for a known item contains a status value
  not valid for that item's type
When the PM triggers a sync
Then a SyncRequested event is emitted
And an ItemSyncSkippedInvalidStatus event is emitted for the invalid item
And that item's status is not updated in the project record
And valid changes for all other items are still applied
And a SyncCompleted event is emitted reporting changes applied and items skipped
```

## Invariants

- Only items that already exist in the project record are ever synced —
  Logseq pages whose content contains no recognisable `item-id:` bullet
  are silently ignored
- Items in the project record with no corresponding Logseq page are silently
  skipped — their status and priority remain unchanged in the project record
- Only status and priority are read from Logseq; no other attribute is ever
  written to the project record by a sync
- Existing entries in the project event log are never modified or deleted
  by a sync
- A sync run where no differences are detected emits no update events

## Preconditions

- The project record is readable
- A Logseq graph directory has been designated
- The project record contains at least one item (otherwise EmptyRecord
  failure applies)

## Postconditions

- For each item whose status changed: a status update has been appended
  to the project event log; the item's effective status now matches Logseq
- For each item whose priority changed: a priority update has been appended
  to the project event log; the item's effective priority now matches Logseq
- Items skipped due to an invalid status are reported in the SyncCompleted
  payload; their project record state is unchanged
- Items in Logseq with no recognised `item-id:` bullet are not mentioned in any event
- Items in the project record with no corresponding Logseq page are not mentioned in any event

## Runtime Artifacts

| Artifact | Path | Lifecycle |
|---|---|---|
| None beyond events/runtime_events.jsonl | — | — |

## Failure Classifications

| Failure Name | Trigger Condition | Observable Signal |
|---|---|---|
| GraphNotAccessible | Logseq graph directory is missing or cannot be read | `SyncFailedGraphNotAccessible` event emitted; sync aborts |
| ProjectRecordEmpty | Project record contains no items at sync time | `SyncFailedEmptyRecord` event emitted; sync aborts |
| InvalidStatusForType | A Logseq page contains a status not valid for that item's type | `ItemSyncSkippedInvalidStatus` event emitted per item; sync continues |

---

<!-- METADATA -->
status: APPROVED
feature_id: logseq_sync
approved_by: human
approved_at: 2026-05-26
refined_at: 2026-05-29
refinement_log: intents/logseq_sync_refinements.md
derived_from_intent: intents/logseq_sync.md
derived_event_schema: events/logseq_sync_schema.md

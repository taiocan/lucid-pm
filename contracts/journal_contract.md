# Behavioral Contract: journal

<!--
DERIVED FROM: intents/journal.md
-->

## Scenarios

### Happy Path 1: Entry Created

```gherkin
Given the PM is working within a project
When the PM creates a new journal entry with a title
Then an entry file exists that is addressable by its creation date and title
And the entry appears in the project's journal listing
And the entry content is initially empty (ready for the PM to write)
And the project record is not modified
```

### Happy Path 2: Entry Listed

```gherkin
Given one or more journal entries have been created for the current project
When the PM lists journal entries
Then all entries for the current project are returned
And entries are ordered by creation date, most recent first
And each entry shows its filename, title, and creation date
```

### Happy Path 3: Entry Located

```gherkin
Given one or more journal entries exist for the current project
When the PM requests to open a specific entry by filename
Then the path to that entry file is returned
And no modification is made to the entry or the project record
```

### Happy Path 4: Empty Journal Listed

```gherkin
Given no journal entries have been created for the current project
When the PM lists journal entries
Then an empty list is returned
And no failure is signalled
```

### Failure Path 1: EntryNotFound

```gherkin
Given a filename that does not correspond to any journal entry in the current project
When the PM attempts to open that entry
Then a failure result is returned indicating the entry was not found
And no entry is created or modified
```

---

## Invariants

- A journal entry file is never deleted or overwritten by the system after
  creation — only the PM can modify content through their editor
- The listing order is always chronological by creation date, descending —
  no reordering occurs between invocations
- An entry created in one project's journal directory is never returned when
  listing entries for a different project

## Preconditions

- The project directory exists and is writable (for entry creation)
- For open/list: the journal directory may not yet exist (list returns empty;
  open returns EntryNotFound)

## Postconditions

- After a successful create: an entry file exists at a deterministic path
  derived from the creation date and title slug; a record of the creation
  exists in the event log
- After a successful list: a record of the listing exists in the event log
  containing all current entry metadata
- After a successful open: a record of the open request exists in the event log
  containing the returned path

## Runtime Artifacts

| Artifact | Path | Lifecycle |
|---|---|---|
| Journal entry files | `journal/YYYY-MM-DD-<slug>.<ext>` | Created on `new`; never deleted by system |
| Event log | `events/runtime_events.jsonl` | Append-only |

The `journal/` directory is created automatically on first entry creation if
it does not exist.

## Failure Classifications

| Failure Name | Trigger Condition | Observable Signal |
|---|---|---|
| EntryNotFound | PM requests to open a filename not present in the journal directory | `JournalOpenFailedEntryNotFound` emitted |

---

<!-- METADATA -->
status: APPROVED
feature_id: journal
approved_by: human
approved_at: 2026-05-28
derived_from_intent: intents/journal.md
derived_event_schema: events/journal_schema.md

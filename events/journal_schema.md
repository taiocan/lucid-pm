# Event Schema: journal

<!--
DERIVED FROM:
- intents/journal.md
- contracts/journal_contract.md
-->

## Naming Convention

See `.codeos/templates/conventions.md`.

## Required Base Fields (all events)

```json
{
  "event_id": "uuid-v4",
  "event_type": "EventName",
  "timestamp": 1710000000000,
  "correlation_id": "uuid-v4",
  "source_module": "journal",
  "payload": {}
}
```

---

## Event Definitions

### JournalEntryCreated

- category: BEHAVIORAL
- emitted when: the PM creates a new journal entry and the file is written
- payload:
  - `filename`: `string` — the entry filename (e.g. `2026-05-28-sprint-planning.md`)
  - `title`: `string` — the title supplied by the PM
  - `created_at`: `string` (ISO-8601 date) — the creation date (`YYYY-MM-DD`)

### JournalListRequested

- category: OBSERVATIONAL
- emitted when: the PM requests a listing of journal entries
- payload: *(empty object)*

### JournalListReturned

- category: BEHAVIORAL
- emitted when: the journal listing is produced (including empty)
- payload:
  - `entry_count`: `u32` — number of entries (may be zero)
  - `entries`: `array<JournalEntry>` — ordered by `created_at` descending

  **JournalEntry object shape:**
  ```json
  {
    "filename": "2026-05-28-sprint-planning.md",
    "title": "Sprint planning",
    "created_at": "2026-05-28"
  }
  ```

### JournalOpenRequested

- category: OBSERVATIONAL
- emitted when: the PM requests to open a specific entry by filename
- payload:
  - `filename`: `string` — the requested filename

### JournalEntryOpened

- category: BEHAVIORAL
- emitted when: the requested entry exists and its path is returned
- payload:
  - `filename`: `string` — the filename that was opened
  - `path`: `string` — the full filesystem path returned to the PM

### JournalOpenFailedEntryNotFound

- category: FAILURE
- emitted when: the requested filename does not exist in the journal directory
- payload:
  - `failure_reason`: `string` — `"entry_not_found"`
  - `filename`: `string` — the filename that was not found

---

## Event Flow

```text
── NEW ────────────────────────────────────────────────────────────────────────

  ↓ (PM creates entry)
JournalEntryCreated       ← file written, path available

── LIST ───────────────────────────────────────────────────────────────────────

JournalListRequested      ← PM requests listing

  ↓ (zero or more entries)
JournalListReturned

── OPEN ───────────────────────────────────────────────────────────────────────

JournalOpenRequested      ← PM requests a specific entry

  ↓ (entry exists)
JournalEntryOpened

  ↓ (entry not found)
JournalOpenFailedEntryNotFound
```

---

## Coverage Check

| Contract Scenario | Events | Status |
|---|---|---|
| HP1: Entry Created | JournalEntryCreated | COVERED |
| HP2: Entry Listed | JournalListRequested → JournalListReturned | COVERED |
| HP3: Entry Located | JournalOpenRequested → JournalEntryOpened | COVERED |
| HP4: Empty Journal Listed | JournalListRequested → JournalListReturned (entry_count=0) | COVERED |
| FP1: EntryNotFound | JournalOpenRequested → JournalOpenFailedEntryNotFound | COVERED |

---

<!-- METADATA -->
status: APPROVED
feature_id: journal
approved_by: human
approved_at: 2026-05-28
derived_from_intent: intents/journal.md
derived_from_contract: contracts/journal_contract.md

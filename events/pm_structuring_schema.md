# Event Schema: pm_structuring

<!--
PURPOSE OF THIS FILE:
Defines the complete event spine for this feature.
This is the most constraining artifact in the DBA loop.

Once approved, implementation may ONLY emit events listed here.
No additional events are permitted without:
1. Updating this schema
2. Re-approval of the schema
3. Re-run of affected stages

DERIVED FROM:
- intents/pm_structuring.md (actors, outcomes)
- contracts/pm_structuring_contract.md (state transitions, failure modes)
-->

## Naming Convention

See `.codeos/templates/conventions.md`.

## Required Base Fields (all events)

Every event must include these fields:

```json
{
  "event_id": "uuid-v4",
  "event_type": "EventName",
  "timestamp": 1710000000000,
  "correlation_id": "uuid-v4",
  "source_module": "pm_structuring",
  "payload": {}
}
```

`correlation_id` is mandatory and must propagate through the entire execution chain.

## Event Definitions

### TextSubmitted

- category: OBSERVATIONAL
- emitted when: PM submits unstructured text for extraction
- payload:
  - `source_text`: `string` — the original text exactly as submitted
  - `input_length`: `integer` — character count of submitted text

### ItemsExtracted

- category: BEHAVIORAL
- emitted when: structured items are successfully extracted and ready for PM review
- payload:
  - `items`: `array` — list of extracted items, each containing:
    - `item_id`: `string (uuid-v4)` — unique identifier for this item
    - `item_type`: `string` — one of: task, milestone, risk, issue, stakeholder
    - `description`: `string` — extracted content derived from source text
    - `uncertain`: `boolean` — true if derived from ambiguous or incomplete source content
    - `uncertainty_reason`: `string | null` — explanation of uncertainty when uncertain is true
    - `proposed_status`: `string | null` — AI-inferred initial status; must be a valid status for the item's type, or null if source text provides no basis
    - `proposed_priority`: `string | null` — AI-inferred initial priority; one of: high, medium, low, or null if source text provides no basis
  - `item_count`: `integer` — total number of extracted items
  - `uncertain_count`: `integer` — number of items flagged as uncertain

**Valid proposed_status values by item_type:**

| item_type | valid proposed_status values |
|---|---|
| task | todo, doing, done, waiting, cancelled |
| milestone | pending, achieved, missed |
| risk | open, mitigated, accepted, closed |
| issue | open, in_progress, resolved, closed |
| stakeholder | active, inactive |

### ExtractionConfirmed

- category: BEHAVIORAL
- emitted when: PM confirms the extracted items, accepting them
- payload:
  - `accepted_item_ids`: `array<string>` — IDs of all accepted items
  - `accepted_count`: `integer` — total items accepted

### ExtractionRejected

- category: BEHAVIORAL
- emitted when: PM declines to confirm extracted items; no items are accepted
- payload: (none beyond base fields)

### ExtractionFailedEmptyInput

- category: FAILURE
- emitted when: PM submits empty or blank text (contract failure: EmptyInput)
- payload:
  - `failure_reason`: `string` — always "empty_input"

### ExtractionFailedNoContent

- category: FAILURE
- emitted when: submitted text contains no identifiable PM elements (contract failure: NoExtractableContent)
- payload:
  - `failure_reason`: `string` — always "no_extractable_content"
  - `source_text_length`: `integer` — character count confirming input was non-empty

### ExtractionFailedApiRequest

- category: FAILURE
- emitted when: the extraction API is unreachable or returns an error (contract failure: ApiRequestFailed)
- payload:
  - `failure_reason`: `string` — always "api_request_failed"
  - `error_detail`: `string` — human-readable description of the API error

## Event Flow

```text
TextSubmitted                          ← PM submits text
  ↓
  ├─ (empty input)
  │    ExtractionFailedEmptyInput
  │
  ├─ (no PM elements found)
  │    ExtractionFailedNoContent
  │
  ├─ (API unreachable or error)
  │    ExtractionFailedApiRequest
  │
  └─ (items extracted)
       ItemsExtracted                  ← items presented to PM for review
         ↓
         ├─ (PM confirms)
         │    ExtractionConfirmed
         │
         └─ (PM rejects)
              ExtractionRejected
```

## Coverage Check

| Contract Failure     | Event Here                  | Status  |
|---|---|---|
| EmptyInput           | ExtractionFailedEmptyInput  | COVERED |
| NoExtractableContent | ExtractionFailedNoContent   | COVERED |
| PMRejectedExtraction | ExtractionRejected          | COVERED |
| ApiRequestFailed     | ExtractionFailedApiRequest  | COVERED |

---

## R2: Folder Ingestion — Schema Additions

### Change to `ItemsExtracted` (additive)

One new field added to the existing payload:

- `source_file`: `string | null` — `null` for stdin sessions; filename (not full path) for folder-mode runs

Existing events where `source_file` is absent are treated as `null` (backward-compatible).

### FolderScanRequested

- category: OBSERVATIONAL
- emitted when: PM invokes `--folder <path>`; one event per folder run
- payload:
  - `folder_path`: `string` — path as provided by PM
  - `auto_confirm`: `boolean` — true when `--yes` flag is present

### FolderScanCompleted

- category: BEHAVIORAL
- emitted when: folder scan run finishes (whether or not any files were processed)
- payload:
  - `folder_path`: `string` — path as provided by PM
  - `files_found`: `integer` — total `.txt`/`.md` files in folder
  - `files_skipped`: `integer` — files already processed in a prior run (or unreadable/empty)
  - `files_processed`: `integer` — files that went through the extraction pipeline this run

### ExtractionFailedFolderNotFound

- category: FAILURE
- emitted when: `--folder` path does not exist on disk (contract failure: FolderNotFound)
- payload:
  - `failure_reason`: `string` — always `"folder_not_found"`
  - `folder_path`: `string` — the path that was not found

### R2 Event Flow

```text
FolderScanRequested                     ← PM runs --folder <path>
  ↓
  ├─ (folder not found)
  │    ExtractionFailedFolderNotFound
  │
  └─ (folder found)
       [for each unprocessed .txt/.md file — own correlation_id]:
         TextSubmitted
           ↓  (standard per-file pipeline, unchanged)
         ItemsExtracted  [source_file = filename]
           ↓
           ├─ ExtractionConfirmed
           └─ ExtractionRejected
       FolderScanCompleted
```

`FolderScanRequested` and `FolderScanCompleted` share a folder-level `correlation_id`.
Each per-file pipeline uses its own file-level `correlation_id`.

### R2 Coverage Check

| Contract Failure   | Event                          | Status  |
|---|---|---|
| FolderNotFound     | ExtractionFailedFolderNotFound | COVERED |
| FolderEmpty        | FolderScanCompleted (files_found=0) | COVERED |

---

<!-- METADATA -->
status: APPROVED
feature_id: pm_structuring
approved_by:
approved_at:
derived_from_intent: intents/pm_structuring.md
derived_from_contract: contracts/pm_structuring_contract.md

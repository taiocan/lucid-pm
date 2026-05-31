# Behavioral Contract: pm_structuring

<!--
PURPOSE OF THIS FILE:
Defines observable truths derived from the approved intent.
Contracts describe OBSERVABLE behavior, not internal logic.
Every clause must be independently testable.
This file must be APPROVED before Stage 4 (implementation) begins.

DERIVED FROM: intents/pm_structuring.md
-->

## Scenarios

### Happy Path: Full Extraction with PM Confirmation

```gherkin
Given the PM has provided unstructured text containing identifiable project management elements
When the PM submits the text for extraction
Then a set of structured items is presented to the PM for review
And each item is classified as one of: task, milestone, risk, issue, or stakeholder
And the original source text is unchanged
When the PM confirms the extracted items
Then the items are accepted
```

### Happy Path Variant: Extraction with Uncertainty

```gherkin
Given the PM has provided unstructured text that is partially ambiguous or incomplete
When the PM submits the text for extraction
Then structured items are presented to the PM for review
And each item derived from ambiguous content is visibly marked as uncertain
And the original source text is unchanged
When the PM confirms the extracted items
Then the items are accepted with their uncertainty markers intact
```

### Happy Path Variant: Extraction with Proposed Status and Priority

```gherkin
Given the PM has provided unstructured text from which status or priority can be inferred
When the PM submits the text for extraction
Then each extracted item may carry a proposed_status and/or proposed_priority
And proposed values are constrained to the valid vocabulary for that item's type
And proposed_status or proposed_priority is null when the source text provides no basis for inference
When the PM confirms the extracted items
Then the proposed values are accepted alongside the items
When the PM declines to confirm the extracted items
Then no proposed values take effect
```

### Failure Path 1: EmptyInput

```gherkin
Given the PM has not provided any text
When the PM attempts to submit empty text for extraction
Then no extraction is performed
And the PM is informed that input text is required
And no items are created
```

### Failure Path 2: NoExtractableContent

```gherkin
Given the PM has provided non-empty text containing no identifiable project management elements
When the PM submits the text for extraction
Then no items are extracted
And the PM is informed that no project management elements were found
And the original source text is unchanged
```

### Failure Path 3: PMRejectedExtraction

```gherkin
Given extracted items have been presented to the PM for review
When the PM declines to confirm the extracted items
Then no items are accepted
And the original source text is unchanged
```

### Failure Path 4: ApiRequestFailed

```gherkin
Given the PM has submitted valid non-empty text
When the extraction service is unreachable or returns an error
Then no items are extracted
And a classified terminal failure event is emitted
And the original source text is unchanged
```

## Invariants

- Source text is never modified at any point in the process
- Extracted items contain only information present in the source text
- Proposed status and priority values are inferences from the source text — they are never invented
- Proposed values that fall outside the valid vocabulary for an item's type are not emitted
- Uncertainty in any extracted item is always visible to the PM before confirmation is possible
- Every accepted correlation chain MUST eventually resolve to a classified terminal state whenever execution remains recoverable

## Preconditions

- PM has unstructured text to submit
- Submitted text is non-empty

## Postconditions

- A set of structured items (tasks, milestones, risks, issues, and/or stakeholders) exists,
  each traceable to the source text
- Each accepted item retains its uncertainty marker if one was assigned
- Each accepted item carries proposed_status and proposed_priority where the source text
  provided a basis for inference; both fields are null otherwise
- Source text is preserved exactly as submitted

## Failure Classifications

| Failure Name | Trigger Condition | Observable Signal |
|---|---|---|
| EmptyInput | PM submits empty or blank text | PM informed input is required; no items created |
| NoExtractableContent | Text contains no identifiable PM elements | PM informed nothing was found; source preserved |
| PMRejectedExtraction | PM declines to confirm extracted items | Items discarded; source preserved |
| ApiRequestFailed | Extraction API is unreachable or returns an error | Classified terminal failure event emitted; source preserved |

## Infrastructure Failure Semantics

An orphaned correlation chain (a chain with no terminal event) is an operational
anomaly requiring infrastructure-level investigation. It is NOT expected behavior
for any recoverable failure mode.

Only unrecoverable runtime termination (e.g. SIGKILL, host loss, power failure)
may legitimately result in an orphaned chain. All other failures — including API
errors, network timeouts, and dependency unavailability — must resolve to a
classified terminal failure event before the process exits.

---

## R2: Folder Ingestion Refinement

*Additive clauses only. No existing scenario is modified.*

### Happy Path: Folder Ingestion — New Files

```gherkin
Given a folder contains one or more .txt or .md files
And none of those files have a matching source_file in any prior ItemsExtracted event
When the PM runs pm_structuring with --folder <path>
Then each file is processed through the standard extraction pipeline in turn
And each ItemsExtracted event carries source_file set to that file's filename
And the existing stdin extraction flow is unaffected
```

### Happy Path: Folder Ingestion — All Files Already Processed

```gherkin
Given a folder contains one or more .txt or .md files
And every file in the folder has a matching source_file in a prior ItemsExtracted event
When the PM runs pm_structuring with --folder <path>
Then no extraction is performed
And the PM is informed that there are no new files to process
```

### Happy Path: Folder Ingestion — Partial Skip

```gherkin
Given a folder contains multiple .txt or .md files
And some files have a matching source_file in a prior ItemsExtracted event
And some files do not
When the PM runs pm_structuring with --folder <path>
Then only the unprocessed files are extracted
And already-processed files are silently skipped
```

### Happy Path: Folder Ingestion — Non-interactive

```gherkin
Given a folder contains one or more unprocessed .txt or .md files
When the PM runs pm_structuring with --folder <path> --yes
Then each file is extracted and confirmed without PM review
And each file produces the standard ItemsExtracted + ExtractionConfirmed + ItemsIncorporated chain
```

### Failure Path: FolderNotFound

```gherkin
Given the PM specifies a --folder path that does not exist
When pm_structuring is invoked with that path
Then no extraction is performed
And the PM is informed that the folder was not found
And a classified terminal failure event is emitted
```

### Failure Path: FolderEmpty

```gherkin
Given the PM specifies a --folder path that exists but contains no .txt or .md files
When pm_structuring is invoked with that path
Then no extraction is performed
And the PM is informed that no eligible files were found
```

### New Invariants

- A file whose filename appears in any prior `ItemsExtracted.payload.source_file` is **never re-processed**
- `ItemsExtracted.payload.source_file` is **null** for stdin sessions and a **non-empty string** (filename) for folder sessions
- The per-file extraction pipeline in folder mode is **identical** to the stdin pipeline — same LLM call, same event chain, same failure modes

### New Failure Classifications

| Failure Name | Trigger Condition | Observable Signal |
|---|---|---|
| FolderNotFound | `--folder` path does not exist on disk | PM informed; terminal failure event emitted; no extraction |
| FolderEmpty | `--folder` path exists but has no `.txt`/`.md` files | PM informed; no extraction |

---

<!-- METADATA -->
status: APPROVED
feature_id: pm_structuring
approved_by:
approved_at:
derived_from_intent: intents/pm_structuring.md
derived_event_schema: events/pm_structuring_schema.md

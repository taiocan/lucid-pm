# Behavioral Contract: report_export

## Scenarios

### Happy Path 1: Report to stdout (no graph destination)

```gherkin
Given the project record contains at least one item
When the PM requests a report of a recognised type without specifying a graph path
Then the report content is written to stdout as formatted markdown text
And the project record remains unchanged
And no files are created in the current directory
```

### Happy Path 2: Report to Logseq graph

```gherkin
Given the project record contains at least one item
And a directory exists at the specified graph path
When the PM requests a report of a recognised type with a graph path
Then a markdown file is written into the graph directory
And the project record remains unchanged
And no other files are created or modified
```

### Happy Path 3: Weekly report with no items in the last 7 days

```gherkin
Given the project record contains items
And no items were added to the record in the last 7 days
When the PM requests a weekly report
Then the report is generated with zero recent items noted
And no failure is signalled
```

### Failure Path 1: EmptyRecord

```gherkin
Given the project record contains no items
When the PM requests any report type
Then a failure result is returned indicating the record is empty
And no report content is written
And no files are created
```

### Failure Path 2: InvalidReportType

```gherkin
Given the project record contains items
When the PM requests a report with a type value that is not one of:
    weekly, risk-register, stakeholders, full
Then a failure result is returned identifying the invalid type value
And no report content is written
```

### Failure Path 3: OutputNotFound

```gherkin
Given the project record contains items
And the PM specifies a graph path that does not exist
When the PM requests a report
Then a failure result is returned indicating the graph path was not found
And no report content is written
```

## Invariants

- Report generation never modifies any item's status, priority, or any
  other field in the project record
- Report content reflects project record state at the moment of generation;
  subsequent changes to the record do not alter an already-generated report
- Exactly one report type is produced per invocation
- "Recent activity" in weekly reports means items incorporated in the
  7 days prior to the moment of generation
- Effective status and priority include proposed values from extraction
  as a fallback when no explicit value has been set (consistent with
  item_status behaviour)

## Preconditions

- The project record exists and contains at least one item
  (required for non-failure execution)
- When a graph path is specified, it must exist as a directory

## Postconditions

- On success without graph: report content has been written to stdout
- On success with graph: exactly one markdown file has been written
  to the graph directory; the file did not exist before or has been
  overwritten (idempotent)
- No item in the project record has been modified

## Runtime Artifacts

| Artifact | Path | Lifecycle |
|---|---|---|
| Report markdown file | `<graph-path>/<report-name>.md` | Created or overwritten on each successful run with `--graph`; no cleanup |
| (stdout output) | — | Written to stdout only; no file created when `--graph` is absent |

## Failure Classifications

| Failure Name | Trigger Condition | Observable Signal |
|---|---|---|
| EmptyRecord | Project record contains no items | ReportFailedEmptyRecord emitted |
| InvalidReportType | --type value is not in {weekly, risk-register, stakeholders, full} | ReportFailedInvalidType emitted |
| OutputNotFound | Specified --graph directory does not exist | ReportFailedOutputNotFound emitted |

---
status: APPROVED
feature_id: report_export
approved_by: human
approved_at: 2026-05-27
derived_from_intent: intents/report_export.md
derived_event_schema: events/report_export_schema.md

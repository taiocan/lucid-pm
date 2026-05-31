# Intent: report_export

`report_export` exists to let the PM produce a point-in-time structured
summary of the project record in a shareable, human-readable form.

Specifically:
- PM can generate a summary of all project items organised by type
- PM can generate a report scoped to risks and their current states
- PM can generate a report scoped to stakeholders and their associated items
- PM can generate a weekly status summary of open items and recent activity
- PM can direct report output to a Logseq graph or receive it as a
  standalone markdown file

## Stable Guarantees

- Reports reflect the state of the project record at the exact moment
  of generation
- Report generation does not modify any item's status, priority, or
  any other field in the project record
- Each report type produces output independent of any other report type

## Scope Boundary

This feature does NOT:
- Modify any item in the project record
- Schedule or automatically repeat report generation
- Send or publish reports to external systems beyond the output destination
  supplied at invocation
- Serve as a real-time or live view of the project record

---
status: APPROVED
feature_id: report_export
approved_by: human
approved_at: 2026-05-27
derived_contracts: contracts/report_export_contract.md

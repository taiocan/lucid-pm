# F6: `report_export`

**Tier**: 3 — Analytics
**Depends on**: F1, F2
**Event spine impact**: New schema (new feature)
**Status**: COMPLETE

---

**Intent sketch**
The PM can generate formatted project reports (weekly status summary, risk register, stakeholder list, open items by type) as Logseq pages or plain markdown files. Reports are point-in-time snapshots derived from the event log.

**Report types**
- Weekly status: open tasks + risks, milestone progress, items added this week
- Risk register: all risks with current status and priority
- Stakeholder map: all stakeholders with associated items
- Full project summary: all item types, counts, session history

**CLI**: `report_export --type weekly|risk-register|stakeholders|full [--graph /path/to/logseq]`

**Event spine (new)**: `ReportRequested` → `ReportGenerated` | `ReportFailedEmptyRecord`

# F5: `priority_view`

**Tier**: 3 — Analytics
**Depends on**: F1
**Event spine impact**: New schema (new feature)
**Status**: COMPLETE

---

**Intent sketch**
The PM can view all open items sorted by priority and status in a single actionable list — replacing `project_state view` for day-to-day work. Filters by item type, status, and priority level.

**CLI**: `priority_view [--type task|risk|issue|milestone] [--status open|doing] [--priority high]`

**Logseq alignment**: output format mirrors Logseq block properties so the view is consistent with what the PM sees in Logseq.

**Event spine (new)**: `PriorityViewRequested` → `PriorityViewReturned` | `PriorityViewFailedEmptyRecord`

# R16: `logseq_export` Stage 9 — Suppress Unassigned Owner in Task Blocks

**Tier**: Refine  
**Depends on**: F12, F14 (logseq_plugin)  
**Event spine impact**: None  
**Status**: BACKLOG

**Trigger type**: HUMAN_APPROVED_EVOLUTION

---

**The problem**

When a task record exists with no owner set (i.e., `owner_id == "TBD"`), the current export renders the task block line as:

```
- TODO Task description [[TBD]]
```

`[[TBD]]` is a Logseq wiki-link that points to a non-existent page. It clutters the block, creates orphaned backlinks, and misleads the PM into thinking ownership has been resolved. In practice, most tasks start without an owner. The unassigned state should be visually absent — not a broken reference.

---

**What needs to change**

- A domain predicate `is_assigned(owner_id)` is introduced to encapsulate the unassigned sentinel ("TBD"). Rendering calls this predicate rather than comparing the string directly.
- When `is_assigned` returns false, the task block line omits the owner wiki-link:  
  `- TODO Task description`
- When `is_assigned` returns true, behavior is unchanged:  
  `- TODO Task description [[owner-slug]]`
- The contract scenario "Task Block with TBD Owner" is replaced by "Task Block with Unassigned Owner" with the corrected expected output.

**What does NOT change**

- Task block structure (`:PROPERTIES:` drawer, `:task-id:`, `SCHEDULED:`, `DEADLINE:`)
- Named owner rendering
- Event spine: no new events

---

**DBA classification**

| Artifact | Change type |
|---|---|
| `contracts/logseq_export_contract.md` | Replace "TBD Owner" scenario with "Unassigned Owner"; update invariant line 151–152 |
| `modules/logseq_export/src/main.rs` | Add `is_assigned()` helper; update task block emission (~lines 623–629) |
| Behavioral tests | Replace TBD-owner test case with unassigned-omits-link case; add named-owner test |

Stages re-run: Stage 2 → Stage 4 → Stage 5.

---

**Open design questions for Stage 2**

1. Should the owner omission apply only when `owner_id == "TBD"`, or should `None` (absent) also be treated as unassigned? The current model always stores "TBD" as the default, but a future schema change might allow None.
2. Should `is_assigned()` live in `logseq_export` or in a shared domain module (e.g., `lucid_core`) so `logseq_sync` can also use it when writing back owner changes?

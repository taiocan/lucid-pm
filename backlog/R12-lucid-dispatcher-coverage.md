# R12: `lucid` dispatcher — full feature coverage

**Tier**: 8 — Developer Experience
**Refines**: `bin/lucid`
**Event spine impact**: None (dispatcher only — no new events)
**Status**: BACKLOG

---

**Context**

`bin/lucid` is an existing bash dispatcher that routes `lucid <command>` to individual
feature binaries. It already covers extract, state, status, link, export, sync, project,
priority, report, suggest, journal, and schema.

The gap is that newly-added binaries are not automatically reflected in the dispatcher —
`install.sh` and `bin/lucid` must be updated together manually, and they have drifted:

| Binary | In install.sh | In lucid dispatcher |
|---|---|---|
| task_model (F12) | ✅ | ❌ missing |

---

**Intent sketch**

The PM can invoke any installed LucidPM feature through `lucid <command>`. The dispatcher
and `lucid help` are always in sync with the installed binary set. No feature is reachable
only by calling its binary directly.

---

**Scope**

1. Add missing `task` dispatch case and help text to `bin/lucid`
2. Establish a convention (comment or CI check) so install.sh and bin/lucid stay in sync
   as future features are added

---

**Acceptance criteria**

- `lucid task add --description "..." --parent <id>` works
- `lucid help` lists `task` with a usage example
- Every binary in `install.sh MODULES` has a corresponding dispatch case in `bin/lucid`

# F13: `lucid` — Unified CLI surface covers all features

**Tier**: 8 — Developer Experience
**Depends on**: all existing features
**Event spine impact**: None (dispatcher only — no new events)
**Status**: BACKLOG

---

**Problem statement**

The `lucid` dispatcher in `bin/lucid` does not expose every installed binary. Currently
missing: `task_model` (installed as F12). As new features are added, the dispatcher must
be updated alongside them — but today that coupling is manual and error-prone.

The user should be able to do `lucid task add ...` rather than `task_model add ...`, and
`lucid help` should reflect the full feature set.

---

**Intent sketch**

The PM can perform any LucidPM operation through a single `lucid <command>` entry point.
No installed feature binary is left unreachable from the dispatcher. The `lucid help`
output is the authoritative reference for the full feature surface.

---

**Scope**

1. **Immediate gap** — add `task` to the dispatcher:
   ```bash
   task)      exec "$LUCID_BIN_DIR/task_model" "$@" ;;
   ```
   and document it in `lucid help` output with usage examples.

2. **Systematic gap** — prevent future drift: install.sh and lucid should be kept in sync.
   Options:
   - A lint step (CI check that every entry in MODULES has a case in lucid)
   - A generated dispatcher (install.sh writes the case statement from MODULES at
     install time)
   - Convention documentation in CLAUDE.md requiring both files to be updated together

3. **Out of scope**: changing how individual binaries work, adding new subcommands to
   existing features, or building a Rust-native dispatcher.

---

**Acceptance criteria**

- `lucid task add --description "..." --parent <id>` works
- `lucid help` lists the task command with a usage example
- Every binary listed in `install.sh MODULES` has a corresponding `lucid` dispatch case
- No installed binary is reachable only by calling it directly

---

**Implementation notes**

The dispatcher is a bash `case` statement in `bin/lucid`. The fix for the immediate gap
is two lines (case entry + help text). The systematic gap requires a decision on the
enforcement strategy — the simplest is a comment in both files stating the coupling
requirement, backed by a CI check.

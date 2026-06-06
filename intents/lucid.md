# Intent: lucid

`lucid` exists to let the PM access the full LucidPM feature set through a single named
entry point, without needing to know individual feature module names.

## Definitions

**Command** — the first argument to `lucid` that identifies which feature module to
route to (e.g., `extract`, `state`, `status`).

**Available command** — a command that `lucid` accepts and can route to a feature module.

Specifically:
- PM can invoke any available command through `lucid`
- PM can discover every available command and a usage example for each
- PM receives an error that names the unrecognized command and references `lucid help`
  when invoking a command `lucid` does not recognize

## Stable Guarantees

- Every available command reaches its intended feature module — no accepted command
  silently fails
- Every available command appears in the help output; every command in the help output
  is available — no command is discoverable but unreachable, and no command is
  reachable but undiscoverable
- Invoking a command through `lucid` produces the same stdout, stderr, and exit code
  as invoking the feature module directly with the same arguments
- An unrecognized command always produces a non-zero exit code and an error message
  that names the unrecognized command and references `lucid help`

## Scope Boundary

This feature does NOT:
- Implement any feature behavior
- Delegate help output to individual feature modules — the dispatcher owns all help text
- Verify that all installed feature modules have corresponding commands — reachability
  is guaranteed for commands that exist in the dispatcher, not for completeness of
  coverage across the installed set
- Manage the installation of feature modules
- Add entries to the runtime event log

---

<!-- METADATA -->
status: APPROVED
feature_id: lucid
approved_by: Primoz Gorjup
approved_at: 2026-06-05
derived_contracts: contracts/lucid_contract.md

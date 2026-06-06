# Behavioral Contract: lucid

<!--
DERIVED FROM: intents/lucid.md (revised 2026-06-05)
-->

## Definitions

**Command** — the first argument to `lucid` that identifies which feature module to
route to (e.g., `extract`, `state`, `status`). Imported from intent.

**Available command** — a command that `lucid` accepts and can route to a feature
module; equivalently, a command that has an entry in the command routing table.
Imported from intent.

**Command routing table** — the authoritative mapping maintained by the dispatcher
that associates each available command with its feature module target. Both dispatch
behavior and help output are derived from this mapping. It is the single source of
truth for what commands exist and what they route to.

---

## Scenarios

### Happy Path: Known Command Dispatched

```gherkin
Given the routing table contains an entry mapping command X to feature module M
And feature module M is installed and executable
When PM invokes `lucid X [args]`
Then feature module M executes with args unchanged
And lucid's stdout carries M's stdout unchanged
And lucid's stderr carries M's stderr unchanged
And lucid exits with M's exit code
```

### Happy Path: Help Output Displayed

```gherkin
Given the routing table contains N entries
When PM invokes `lucid help`
Then help output is produced on stdout listing all N commands from the routing table
And each command entry includes at least one usage example
And no command appears in the help output that does not have a routing table entry
And lucid exits with exit code 0
```

### Boundary Scenario: No Arguments Provided

```gherkin
Given lucid is installed
When PM invokes `lucid` with no arguments
Then the output is identical to invoking `lucid help`
And lucid exits with exit code 0
```

Note: No arguments is treated as equivalent to `lucid help`, not as an error.
This is an explicit design decision — future maintainers must not treat no-args as UnknownCommand.

### Failure Path: UnknownCommand

```gherkin
Given lucid is installed
When PM invokes `lucid foo` where foo has no entry in the routing table
Then lucid exits with a non-zero exit code
And stderr contains the string "foo" (the unrecognized command name)
And stderr contains a reference to `lucid help`
And no feature module is invoked
```

### Falsification Scenario: Routing Table Entry Absent from Help

```gherkin
Given the routing table contains an entry for command X
When PM invokes `lucid help`
Then X appears in the help output with a usage example
Falsifies: help text is maintained as an independently-edited block separate from
           the routing table — a new routing table entry added without updating
           the help block leaves X reachable but absent from discovery
```

### Falsification Scenario: Help Entry Without Routing Table Entry

```gherkin
Given command Y appears in the lucid help output
When PM invokes `lucid Y`
Then lucid routes to a feature module — UnknownCommand is not produced
Falsifies: help text lists commands beyond those in the routing table —
           Y appears discoverable but invocation fails with UnknownCommand
```

---

## Invariants

- For each entry (X → M) in the routing table, invoking `lucid X` reaches feature module M
- The routing table is the single source of truth for both dispatch and help: every routing table entry appears in the help output; every command in the help output has a routing table entry
- Invoking `lucid X [args]` produces the same stdout, stderr, and exit code as invoking the routing table's target module M directly with the same args
- An unrecognized command (no routing table entry) always produces a non-zero exit code, an error message naming the unrecognized command, and a reference to `lucid help`

---

## Invariant Falsification Scenarios

| Invariant | Falsifying fixture | Observable when correct | Wrong implementation assumption | Test ID |
|---|---|---|---|---|
| Routing table entry (X → M) → X reaches M | Invoke `lucid X`; compare output to invoking M directly | Output and exit code match direct invocation of M | Routing table maps X to a different module M' → output differs from M's expected output | LUC-IF-01 |
| Routing table entry → appears in help | Routing table entry for X exists; invoke `lucid help` | X appears in help output with usage example | Help block maintained independently of routing table → new entry added to routing table without updating help leaves X reachable but unlisted | LUC-IF-02 |
| Help entry → routing table entry | Y listed in help output; invoke `lucid Y` | Y routes to a feature module; UnknownCommand not produced | Help text lists commands beyond the routing table → Y visible but invocation fails with UnknownCommand | LUC-IF-03 |
| Same stdout, stderr, exit code as direct invocation | Module M produces output O and exit code E on args A when invoked directly; invoke `lucid X A` where X → M | stdout = O, stderr matches, exit code = E | Dispatcher buffers, modifies, or suppresses module output; or normalizes exit codes → observable output or exit code differs from direct invocation | LUC-IF-04 |
| Unrecognized command error content | Invoke `lucid unknown-xyz`; inspect exit code and stderr | Non-zero exit; stderr contains "unknown-xyz" and "lucid help" | Error uses generic text without the command name, or exits 0, or writes to stdout instead of stderr | LUC-IF-05 |

---

## Preconditions

- `lucid` is installed in a directory alongside all feature module executables referenced in the routing table
- All feature modules in the routing table are installed and executable
- The PM's shell can locate and execute `lucid`

**Violated precondition note:** If a feature module referenced in the routing table cannot
be executed (missing binary, permission denied), the precondition for dispatch is violated.
Contract behavior for that command is undefined. This is an installation concern outside
the scope of this feature — see scope boundary in `intents/lucid.md`.

---

## Postconditions

After successful dispatch:
- The feature module has executed with the provided arguments unchanged
- `lucid` has exited with the feature module's exit code
- No artifacts have been created by `lucid` itself

After `lucid help` or no arguments:
- Help listing on stdout covering all routing table entries with usage examples
- `lucid` has exited with code 0

After UnknownCommand:
- Stderr contains the unrecognized command name and a reference to `lucid help`
- `lucid` has exited with a non-zero exit code
- No feature module has been invoked

---

## Runtime Artifacts

| Artifact | Path | Lifecycle |
|---|---|---|
| (none — `lucid` creates no files; feature modules manage their own artifacts) | — | — |

### Cross-module signals relied upon

| Event | Source module | When relied upon |
|---|---|---|
| (none) | — | — |

Note: `lucid` dispatches to feature modules but does not read or depend on any module's
event stream. Feature modules emit their own events independently of `lucid`.

---

## Failure Classifications

| Failure Name | Trigger Condition | Observable Signal |
|---|---|---|
| UnknownCommand | PM invokes `lucid <cmd>` where `<cmd>` has no routing table entry | Non-zero exit code; stderr names the unrecognized command and references `lucid help` |

Note on event schema: `lucid` emits no events of its own. Stage 3 will produce an event
schema that formally documents this and records the single failure event `UnknownCommand`
if the process requires event representation of failures, or explicitly declares the
schema empty if not.

---

<!-- METADATA -->
status: APPROVED
feature_id: lucid
approved_by: Primoz Gorjup
approved_at: 2026-06-05
derived_from_intent: intents/lucid.md
derived_event_schema: events/lucid_schema.md

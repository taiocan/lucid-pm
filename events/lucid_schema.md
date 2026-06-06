# Event Schema: lucid

<!--
DERIVED FROM:
- intents/lucid.md
- contracts/lucid_contract.md

This is an intentionally empty event schema.
See Design Notes for full reasoning.
-->

## Naming Convention

See `docs/conventions.md` (source: `.codeos/templates/conventions.md`).

## Required Base Fields (all events)

```json
{
  "event_id": "uuid-v4",
  "event_type": "EventName",
  "timestamp": 1710000000000,
  "correlation_id": "uuid-v4",
  "source_module": "module_name",
  "payload": {}
}
```

`correlation_id` is mandatory and must propagate through the entire execution chain.

## Design Notes

`lucid` is a pure dispatcher. Its approved intent explicitly excludes event emission:

> "This feature does NOT add entries to the runtime event log."

This exclusion is not an oversight — it is the correct design for a dispatcher:

**Happy path:** `lucid` execs a feature module and exits with that module's exit code.
The feature module runs in place of `lucid` and emits its own events independently.
Any events in `runtime_events.jsonl` from a dispatched command belong to the feature
module, not to `lucid`. `lucid` has no observable state of its own to record.

**Failure path (UnknownCommand):** The contract specifies the observable signal for
`UnknownCommand` as process semantics — non-zero exit code and a stderr message naming
the unrecognized command and referencing `lucid help`. The process exit code and stderr
output are the complete observable signal for UnknownCommand. No additional event record
is warranted: the observable is fully captured by process semantics, consumed directly
by the invoking shell or calling process.

**DBA completeness rule tension:** The standard DBA rule requires every named contract
failure to have a FAILURE event. That rule assumes features use the event log as their
observability mechanism. `lucid` is explicitly scoped out of the event log. Adding a
`CommandDispatchFailed` event solely to satisfy a process rule would violate the
intent's scope boundary and add an observable the contract does not require.

**Conclusion:** Zero events is the correct and complete event spine for `lucid`.
The Coverage Check below records this decision explicitly.

## Event Definitions

*(none — this feature emits no events)*

## Event Flow

```text
PM invokes `lucid <command> [args]`
  │
  ├─ (recognized command)
  │    Feature module executes.
  │    lucid exits with feature module's exit code.
  │    → No events emitted by lucid.
  │      Feature module emits its own events independently.
  │
  └─ (UnknownCommand)
       lucid writes error to stderr.
       lucid exits with non-zero exit code.
       → No events emitted by lucid.
         Observable signal: process exit code + stderr content.
```

## Cross-module events relied upon

| Event | Source module | Contract clause |
|---|---|---|
| (none) | — | — |

Note: when `lucid` dispatches to a feature module, that module emits its own events.
Those events are not cross-module dependencies of `lucid` — they are independent
emissions by the dispatched module and are outside `lucid`'s event spine.

## Coverage Check

| Contract Failure | Event Here | Observable Signal | Status |
|---|---|---|---|
| UnknownCommand | (none — intentional) | Non-zero exit code; stderr names the command and references `lucid help` | COVERED BY PROCESS SEMANTICS |

**DBA completeness rule override:** The standard rule "every named failure has exactly
one FAILURE event" is overridden here by the intent's explicit scope boundary
("does NOT add entries to the runtime event log"). The `UnknownCommand` observable
signal is fully specified in the contract and is testable without an event record.
No contract clause is left uncovered.

---

<!-- METADATA -->
status: APPROVED
feature_id: lucid
approved_by: Primoz Gorjup
approved_at: 2026-06-05
derived_from_intent: intents/lucid.md
derived_from_contract: contracts/lucid_contract.md

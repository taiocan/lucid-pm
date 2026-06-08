# Event Schema: demo

<!--
DERIVED FROM:
- intents/demo.md
- contracts/demo_contract.md

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

`demo` is a static content artifact — a directory of files and a walkthrough document.
It has no executable module and introduces no new runtime commands.

**No executable module:** Unlike command-driven features (pm_structuring, item_status,
lucid, etc.), `demo` has no binary that runs, no source_module identity, and no
execution chain. There is no agent that could emit events even in principle.

**Events in demo/events/runtime_events.jsonl are not demo events:** When the PM runs
LucidPM commands against the demo content (lucid extract, lucid state, etc.), those
commands emit events under their own source_module identities (pm_structuring,
project_state, etc.). These are events from those features, not from `demo`. The
pre-populated record committed to the demo directory is static content, not live
emission.

**Invariant violations are content-inspection findings, not runtime signals:** The
two invariant violations defined in the contract (WalkthroughFeatureGap,
GraphRecordMismatch) are detected by inspecting the demo content at review and test
time — by parsing WALKTHROUGH.md and diffing export output — not by observing an
event stream during execution.

**Comparison to lucid (null spine for a different reason):** The `lucid` dispatcher
also has a null event spine, but for a different reason: `lucid` runs at runtime
but its intent explicitly excludes event emission. `demo` never runs at all —
the null spine here is structural, not intentional scope exclusion.

**Conclusion:** Zero events is the only correct event spine for `demo`. There is
no event category (OBSERVATIONAL, BEHAVIORAL, FAILURE, EXTERNAL) that applies to
a static content artifact. The Coverage Check below records this explicitly.

## Event Definitions

*(none — this feature has no executable module and emits no events)*

## Event Flow

```text
demo is a static content artifact.

There is no execution chain to diagram.

Observable outcomes (PM opens Logseq graph, follows walkthrough, etc.)
are produced by the PM's direct interaction with the files and by other
LucidPM feature modules invoked during the walkthrough — not by any
demo-owned process.
```

## Cross-module events relied upon

| Event | Source module | When relied upon |
|---|---|---|
| (none) | — | — |

Note: the demo's pre-populated runtime_events.jsonl contains events from many feature
modules (pm_structuring, project_state, item_status, etc.). These are static content
committed to the repository. They are not cross-module signal dependencies — demo
does not depend on any module's live event stream.

## Coverage Check

| Contract Invariant Violation | Event Here | Observable Signal | Status |
|---|---|---|---|
| WalkthroughFeatureGap | (none — intentional) | Absence of `lucid <command>` in WALKTHROUGH.md; detectable by content inspection | COVERED BY CONTENT INSPECTION |
| GraphRecordMismatch | (none — intentional) | Diff between `lucid export` output and committed graph pages; detectable by running the command and comparing | COVERED BY CONTENT INSPECTION |

**Structural null spine:** Unlike `lucid`, where the null spine overrides the standard
"every named failure has a FAILURE event" rule, `demo` does not override any rule.
The rule does not apply because there is no execution context in which events could be
emitted. The Coverage Check documents the detection mechanism for each invariant
violation so that Stage 5 test authors have a clear target.

---

<!-- METADATA -->
status: APPROVED
feature_id: demo
approved_by: Primoz Gorjup
approved_at: 2026-06-05
derived_from_intent: intents/demo.md
derived_from_contract: contracts/demo_contract.md

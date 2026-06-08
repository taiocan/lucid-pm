# Behavioral Contract: demo

<!--
DERIVED FROM: intents/demo.md

Note: demo is a delivered artifact (files + documentation), not a runtime command.
Standard contract conventions are adapted where noted:
- Failure classifications become invariant violation classifications (see below)
- Runtime artifacts section describes required deliverables by capability, not path
- Exact directory structure and filenames are Stage 4 implementation decisions
-->

## Definitions

**Demo project** — the demo directory in the repository root. Contains all
materials needed to follow the walkthrough without any additional setup.

**Self-contained** — every file needed to run the walkthrough is present in the
demo directory; no project setup is required beyond having LucidPM installed.

**Major LucidPM features** — the following features, each of which must be
represented by at least one step in WALKTHROUGH.md:

| Feature | lucid command | Optional? |
|---|---|---|
| Text extraction | `lucid extract` | No |
| Project record view | `lucid state` | No |
| Item status and priority | `lucid status` | No |
| Typed item links | `lucid link` | No |
| Logseq export | `lucid export` | No |
| Logseq sync | `lucid sync` | No |
| Priority view | `lucid priority` | No |
| Report export | `lucid report` | No |
| Vocabulary schema | `lucid schema` | No |
| Task management | `lucid task` | No |
| Journal | `lucid journal` | No |
| AI enrichment proposals | `lucid suggest` | Yes — requires API key |

**Graph-record consistency** — the pre-exported Logseq pages in the demo Logseq
graph are equivalent to the output of running `lucid export` against the demo
project record; no page is present in the graph that is not in the record, and no
record item is absent from the graph.

---

## Scenarios

### Happy Path: PM Opens Pre-Populated Logseq Graph

```gherkin
Given the demo directory is present and LucidPM is installed
When the PM opens the demo Logseq graph in Logseq Desktop
Then pages exist for all items in the demo project record
And item links are rendered as Logseq page references
And status and priority are visible on each item page
And the PM can navigate the project structure without running any commands
```

### Happy Path: PM Completes From-Scratch Walkthrough

```gherkin
Given the PM starts from the raw input files in the demo directory
When the PM follows the walkthrough from the first step to the last
Then each command produces the output described at that step in the walkthrough
And after the final step the PM's project record is equivalent to the
  demo's pre-populated record
And after running lucid export the PM's Logseq graph is equivalent to the
  demo's pre-exported graph
```

### Happy Path: All Major Features Represented

```gherkin
Given the walkthrough document
When the PM reads through it
Then every non-optional major LucidPM feature has at least one step in the walkthrough
And each optional feature (lucid suggest) is present with a clear label
  indicating it requires an external service
And the PM can complete the core workflow without an internet connection
  or an AI API key
```

### Boundary: Demo Runnable Without API Key

```gherkin
Given the PM has no AI API key
When the PM follows all steps not labelled optional
Then every step completes successfully using only the installed LucidPM binaries
And the PM reaches a fully populated Logseq graph without needing an API key
```

### Falsification Scenario: Graph-Record Consistency

```gherkin
Given the demo's pre-populated project record
When the PM runs lucid export against the demo project record
Then the exported pages are equivalent to the pages already present in the
  demo Logseq graph
Falsifies: the pre-exported Logseq graph was authored separately and drifted from
           the project record — the PM would find that re-running export produces
           different pages than those in the demo
```

### Falsification Scenario: Walkthrough Step Completeness

```gherkin
Given the walkthrough document
When the PM follows every step in order on a clean installation
Then the PM reaches a project state that includes output from every
  non-optional major feature (extraction, state view, status, links, export,
  sync, priority, report, schema, tasks, journal)
Falsifies: a feature is installed and functional but absent from the walkthrough
           — the PM completes the demo without ever learning that feature exists
```

---

## Invariants

- The demo is self-contained: all files needed to follow the walkthrough are
  present in the demo directory; no file outside it is required
- Every non-optional major LucidPM feature is represented by at least one walkthrough
  step; optional features are labelled as such
- The pre-exported Logseq graph is consistent with the demo project record:
  running `lucid export` against the record produces equivalent pages
- The walkthrough follows the from-scratch path: it begins with raw notes and ends
  with a fully populated Logseq graph
- The Logseq graph is the primary output surface: each major workflow phase produces
  a visible, navigable change in Logseq
- No existing LucidPM command behavior is changed or overridden by the demo

---

## Invariant Falsification Scenarios

| Invariant | Falsifying fixture | Observable when correct | Wrong implementation assumption | Test ID |
|---|---|---|---|---|
| Demo is self-contained | Follow walkthrough from a clean clone with only LucidPM installed | All steps complete, no missing-file errors | A walkthrough step references a resource outside the demo directory | DEMO-IF-01 |
| Every non-optional feature represented | Parse walkthrough for `lucid <command>` invocations; compare to major features list | All 11 non-optional commands appear at least once | A feature was omitted when writing the walkthrough | DEMO-IF-02 |
| Graph-record consistency | Run `lucid export` against demo record; diff against committed graph pages | No diff — pages are equivalent | Pages were manually authored or regenerated from a different record revision | DEMO-IF-03 |
| From-scratch path complete | Check that extraction is the first data-producing command in the walkthrough | `lucid extract` (or equivalent) appears before any state/status/export steps | Walkthrough begins from a pre-built record, skipping the extraction step | DEMO-IF-04 |
| Logseq is primary output surface | After each major phase, check that a Logseq step follows | Each phase ends with an updated Logseq view | Walkthrough shows terminal output only with no Logseq step after major phases | DEMO-IF-05 |

---

## Preconditions

- LucidPM is installed and all feature binaries are on PATH
- The PM has Logseq Desktop installed (required to open the graph)
- The PM has access to the repository (demo directory is present)

## Postconditions

After completing the walkthrough:
- The PM's project directory contains a populated project record equivalent to
  the demo's pre-populated record
- The PM's Logseq graph contains pages equivalent to the demo's pre-exported graph
- The PM has invoked every non-optional major feature at least once

## Required Deliverables

<!--
This table describes what the demo directory must provide, defined by what each
deliverable enables the PM to do. Exact paths and filenames are Stage 4
implementation decisions — the contract binds outcomes, not file structure.
-->

| Deliverable | What it enables |
|---|---|
| Raw extraction inputs | PM can run `lucid extract` against realistic sample notes covering diverse PM item types |
| Pre-populated project record | PM can run state, status, link, priority, report, and task commands immediately, without extracting first |
| Vocabulary schema | PM can run `lucid schema` and see typed entities and their allowed statuses |
| Pre-exported Logseq graph | PM can open Logseq immediately and see a fully populated navigable project |
| Step-by-step walkthrough | PM can follow the from-scratch workflow from raw notes to populated Logseq graph |

### Cross-module signals relied upon

| Event | Source module | When relied upon |
|---|---|---|
| (none) | — | — |

Note: the pre-populated project record contains events from many feature modules.
These are static content committed to the repository, not live cross-module signals.

---

## Invariant Violation Classifications

<!--
demo is a delivered artifact — invariant violations are detectable at review and test
time by inspecting the demo content, not at runtime via event emission. This replaces
the standard Failure Classifications section used by command-driven features.
Each violation name corresponds to a test in tests/behavioral/ (DEMO-IF).
-->

| Violation Name | Condition | Detectable By |
|---|---|---|
| WalkthroughFeatureGap | A non-optional major feature has no corresponding step in WALKTHROUGH.md | Parsing the walkthrough for `lucid <command>` invocations |
| GraphRecordMismatch | Pre-exported Logseq pages are inconsistent with what `lucid export` produces from the demo record | Running `lucid export` and diffing against committed pages |

---

<!-- METADATA -->
status: APPROVED
feature_id: demo
approved_by: Primoz Gorjup
approved_at: 2026-06-05
derived_from_intent: intents/demo.md
derived_event_schema: events/demo_schema.md

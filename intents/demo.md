# Intent: demo

The demo project exists to let the PM learn the full LucidPM workflow by working
through a realistic example, without needing to create their own project content first.

## Definitions

**Demo project** — a self-contained directory in the repository that includes all
materials needed to follow the walkthrough: raw input notes, a pre-populated project
record, a project vocabulary schema, and a paired Logseq graph with pre-exported pages.

**Self-contained** — everything required to run the demo is present in the demo
directory; no project setup is needed beyond having LucidPM installed.

Specifically:
- PM can follow the full LucidPM workflow from raw notes to a navigable Logseq graph
  by working through the demo without setting up a project of their own
- PM can open the Logseq graph immediately, before running any commands, and see what
  a fully populated LucidPM project looks like
- PM can observe every major LucidPM feature in the demo materials or walkthrough

## Stable Guarantees

- The demo is self-contained: a PM with LucidPM installed can begin immediately
  without any additional project setup
- Every major LucidPM feature is represented; features that require external services
  are clearly identified as optional in the walkthrough
- The walkthrough follows the from-scratch path only — it starts from raw notes and
  builds to a fully populated Logseq graph
- The Logseq graph is the primary output surface of the walkthrough: each major step
  produces a visible change in Logseq
- No existing LucidPM command behavior is changed

## Scope Boundary

This feature does NOT:
- Modify `lucid help` or any existing command behavior
- Provide a walkthrough for the "joining an existing project" scenario
- Add new LucidPM commands or features
- Require an internet connection or AI API key for the core workflow

---

<!-- METADATA -->
status: APPROVED
feature_id: demo
approved_by: Primoz Gorjup
approved_at: 2026-06-05
derived_contracts: contracts/demo_contract.md

# Intent: R13_logseq_plugin_extract

The `logseq-plugin extract` refinement exists so that the PM can trigger extraction
of a Logseq journal page into their active LucidPM project without leaving Logseq Desktop.

## Definitions

**Journal page** — a Logseq daily journal page.

**Active project** — as defined in the parent feature (logseq_plugin): the LucidPM
project determined by explicit configuration or graph path inference.

## Outcomes

- PM can trigger extraction of the current journal page into the active project from
  within Logseq Desktop without switching to a terminal
- PM can see the output produced by extraction as immediate feedback after the command completes
- PM is informed, upon successful extraction, that extracted items are now in the project
  record and that Export is required before they become visible as Logseq pages
- PM is informed when no items were found on the current journal page
- PM is informed when the current page is not a journal page and extraction does not proceed

## Stable Guarantees

- The command operates on the currently open journal page
- Extracting a journal page via the plugin produces the same result as invoking
  `lucid extract` on that page's vault file from the active project directory —
  there is no behavioral divergence between the plugin and the CLI
- The command does not execute extraction against a non-journal page
- Re-extracting a previously extracted journal page delegates to `lucid extract`
  without modification — deduplication behavior is inherited from the CLI and is
  not altered or suppressed by the plugin

## Scope Boundary

This feature does NOT:
- Automatically trigger Export after extraction — Export remains a separate explicit command
- Expose extraction for non-journal Logseq pages
- Write to or modify any Logseq page or block
- Introduce new extraction behavior — all extraction is delegated entirely to `lucid extract`
- Change any `lucid` CLI behavior
- Function in Logseq web or mobile — Desktop only

---

<!-- METADATA -->
status: APPROVED
feature_id: R13_logseq_plugin_extract
approved_by: human
approved_at: 2026-06-13
derived_contracts: contracts/R13_logseq_plugin_extract_contract.md

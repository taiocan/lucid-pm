# Intent: logseq_plugin

The `logseq-plugin` exists so that the PM can act on their LucidPM project without
leaving Logseq Desktop.

## Definitions

**Active project** — the LucidPM project the plugin operates against when a command
is invoked. Determined either by inference from the current Logseq graph path or by
the PM's explicit configuration; explicit configuration takes precedence.

**Plugin command** — a registered action in Logseq Desktop that invokes a LucidPM
operation against the active project.

**Supported operation** — a LucidPM command explicitly exposed by the plugin.
In v1: sync, export, suggest.

---

## Outcomes

- PM can invoke supported LucidPM operations from Logseq Desktop without switching
  to a terminal
- PM can determine whether a requested operation succeeded or failed and receive
  enough information to distinguish normal completion from an execution error
- PM can operate on the intended LucidPM project without manually navigating to its
  directory
- PM can configure which LucidPM project the plugin targets when the Logseq graph
  path is not the project directory

## Stable Guarantees

- A plugin command produces the same result as invoking the equivalent `lucid`
  subcommand from the active project directory — there is no behavioral divergence
  between the plugin and the CLI
- Before execution, the plugin deterministically identifies a single active project
  using either the PM's configured override or the current graph path; if no unique
  project can be determined, the command does not execute and the PM is informed
- Command failure is visible to the PM — a failed command does not produce the same
  visible output as a successful one
- The plugin always reflects the same project state that LucidPM itself uses — it
  is a trigger layer with no independent state

## Scope Boundary

This feature does NOT:
- Expose `extract` — input handling for extraction is deferred
- Function in Logseq web or mobile — Desktop only
- Write command output into Logseq pages or blocks
- Add new LucidPM behaviors — all operations are delegated entirely to `lucid`
- Change any `lucid` CLI behavior

---

<!-- METADATA -->
status: APPROVED
feature_id: logseq_plugin
approved_by: human
approved_at: 2026-06-09
derived_contracts: contracts/logseq_plugin_contract.md

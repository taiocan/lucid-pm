# Intent: logseq_plugin

The `logseq-plugin` exists so that the PM can act on their LucidPM project without
leaving Logseq Desktop.

## Definitions

**Active project** — the LucidPM project the plugin operates against when a command
is invoked. Determined either by inference from the current Logseq graph path or by
the PM's explicit configuration; explicit configuration takes precedence.

**Plugin command** — a registered action in Logseq Desktop that invokes a LucidPM
operation against the active project.

---

Specifically:
- PM can invoke sync, export, and suggest against the active project from inside
  Logseq Desktop
- PM can see the outcome of each command (success or failure) without leaving Logseq

## Stable Guarantees

- A plugin command produces the same result as invoking the equivalent `lucid`
  subcommand from the active project directory — there is no behavioral divergence
  between the plugin and the CLI
- The active project is deterministic before a command is invoked: the graph path
  is used to identify the project unless the PM has set an explicit override, in
  which case the override is used
- Command failure is visible to the PM — a failed command does not produce the same
  visible output as a successful one
- The plugin introduces no data model of its own — all project state is managed
  exclusively by `lucid`; the plugin is a trigger layer only

## Scope Boundary

This feature does NOT:
- Expose `extract` — input handling for extraction is deferred
- Function in Logseq web or mobile — Desktop only
- Write command output into Logseq pages or blocks
- Add new LucidPM behaviors — all operations are delegated entirely to `lucid`
- Change any `lucid` CLI behavior

---

<!-- METADATA -->
status: DRAFT
feature_id: logseq_plugin
approved_by:
approved_at:
derived_contracts: contracts/logseq_plugin_contract.md

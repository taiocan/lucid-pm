# Schema: logseq_plugin

<!--
DERIVED FROM: intents/logseq_plugin.md, contracts/logseq_plugin_contract.md

NOTE ON SCHEMA TYPE
This feature is a Logseq Desktop plugin (JavaScript). It does not emit events to
events/runtime_events.jsonl — that log is written by delegated `lucid` commands,
governed by their own schemas. This document functions as an observable interface
schema rather than an event schema: it defines the commands the plugin registers,
the feedback states it presents to the PM, and the configuration it persists.
-->

---

## Registered Commands

The plugin registers Sync, Export, Suggest, and Extract commands with Logseq
Desktop at load time.

| Function | Required | Notes |
|---|---|---|
| Sync | Yes | Invokes `lucid sync` against the active project |
| Export | Yes | Invokes `lucid export` against the active project |
| Suggest | Yes | Invokes `lucid suggest` against the active project |
| Extract | Yes | Invokes `lucid extract --yes` against the currently open journal page (R13) |

---

## Feedback States

The plugin presents feedback to the PM in one of three mutually exclusive states
after any command invocation. A PM observing the interface can determine whether
the invocation succeeded, failed, or could not proceed — without consulting logs
or external tools.

### SuccessIndication

Shown when the delegated `lucid` command exits with code 0.

| Field | Type | Required | Description |
|---|---|---|---|
| content | string | Yes | Feedback content shown to the PM for this invocation |

Constraint: `content` must provide enough information for the PM to distinguish
successful completion from an execution error.

Constraint (Export): for Export commands, `content` also includes the next step
required before exported pages become visible in Logseq. This constraint is
never omitted regardless of the content of the delegated command's output.

### FailureIndication

Shown when the delegated `lucid` command exits with a non-zero exit code.

| Field | Type | Required | Description |
|---|---|---|---|
| content | string | Yes | Feedback content shown to the PM for this invocation |

Constraint: FailureIndication must be presented in a way that a PM can distinguish
it from SuccessIndication without consulting logs or external tools.

### ErrorMessage

Shown when the plugin cannot invoke `lucid` due to a pre-invocation failure.

| Field | Type | Required | Description |
|---|---|---|---|
| failure_type | string | Yes | One of: `ActiveProjectNotResolved`, `LucidNotAvailable` |
| message | string | Yes | Human-readable description of why the command did not run |

Constraint: ErrorMessage is only shown when `lucid` was not invoked. The PM must
be able to determine from the message which failure type occurred.

---

## Configuration Schema

The plugin persists configuration across sessions.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| explicit_project_path | string \| null | No | null | Explicit LucidPM project directory path set by the PM. When non-null, takes precedence over graph path inference. |

`explicit_project_path` is the only configuration field governed by this feature
contract. Changes to it take effect on the next command invocation without
requiring a Logseq restart or modification of any LucidPM project files.

---

## Project Resolution Logic (Observable Contract)

Project resolution is deterministic. Before any command invocation, the plugin
resolves the active project using the following precedence:

1. If `explicit_project_path` is non-null, use it.
2. Otherwise, infer from the current Logseq graph directory.

If resolution does not produce a determinable active project, the plugin presents
an `ErrorMessage` with `failure_type: ActiveProjectNotResolved` and does not
invoke `lucid`.

---

## Delegated Command Behavior

The plugin produces no domain events of its own. All domain events
(SyncCompleted, ExportCompleted, SuggestionProposalGenerated, etc.) are produced
by the delegated `lucid` commands and are governed by their own schemas:

| Plugin command | Delegated to | Schema reference |
|---|---|---|
| Sync | `lucid sync` | events/logseq_sync_schema.md |
| Export | `lucid export` | events/logseq_export_schema.md |
| Suggest | `lucid suggest` | events/ontology_suggest_schema.md |
| Extract | `lucid extract --yes` | events/R13_logseq_plugin_extract_schema.md |

---

## Schema Invariants

- Sync, Export, Suggest, and Extract commands are registered at plugin load time
- The three feedback states (SuccessIndication, FailureIndication, ErrorMessage)
  are mutually exclusive per invocation
- A PM observing the interface can determine whether a command succeeded, failed,
  or could not proceed — without consulting logs or external tools
- ErrorMessage is only shown when `lucid` was not invoked
- `explicit_project_path` is the only configuration field governed by this
  feature contract
- Invoking Sync, Export, or Suggest through the plugin produces the same
  LucidPM operation as invoking the equivalent `lucid` subcommand against the
  resolved active project

---

<!-- METADATA -->
status: APPROVED
feature_id: logseq_plugin
approved_by: human
approved_at: 2026-06-09
refined_at: 2026-06-14 (R14 Stage 9: Extract added to Registered Commands; pre-existing R13 schema drift corrected)
derived_from_contract: contracts/logseq_plugin_contract.md

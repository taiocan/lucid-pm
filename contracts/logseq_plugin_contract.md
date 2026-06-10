# Behavioral Contract: logseq_plugin

<!--
DERIVED FROM: intents/logseq_plugin.md
-->

## Scenarios

### Happy Path 1: Successful Sync

```gherkin
Given the PM is in Logseq Desktop
And the active project is resolved
And `lucid` is available on the system PATH
When the PM invokes the Sync command via the plugin
Then the plugin executes `lucid sync` against the active project
And the PM sees a success indication
And the success indication provides enough information to distinguish successful
  completion from an execution error
And no terminal or external application is opened
```

### Happy Path 2: Successful Export

```gherkin
Given the active project is resolved
And `lucid` is available
When the PM invokes the Export command via the plugin
Then the plugin executes `lucid export` against the active project
And the PM sees a success indication
And the success indication provides enough information to distinguish successful
  completion from an execution error
And no terminal or external application is opened
```

### Happy Path 3: Successful Suggest

```gherkin
Given the active project is resolved
And `lucid` is available
When the PM invokes the Suggest command via the plugin
Then the plugin executes `lucid suggest` against the active project
And the PM sees the suggestion proposals produced by `lucid suggest`
And the PM can distinguish successful completion from an execution error
And no terminal or external application is opened
```

### Happy Path 4: Project Resolved via Graph Path Inference

```gherkin
Given the PM has not configured an explicit project path
And the current Logseq graph directory corresponds to a LucidPM project
When the PM invokes any plugin command
Then the plugin uses the Logseq graph directory as the active project path
```

### Happy Path 5: Explicit Config Overrides Inference

```gherkin
Given the PM has configured an explicit project path P
And the current Logseq graph directory also corresponds to a LucidPM project
When the PM invokes any plugin command
Then the plugin uses path P as the active project directory
And the Logseq graph directory is not used
```

### Happy Path 6: Explicit Project Path Configuration

```gherkin
Given the PM sets an explicit project path P in the plugin configuration
Then all subsequent plugin commands target project P
And the configuration persists across Logseq Desktop sessions
And no LucidPM project files are modified by the act of configuration
```

### Failure Path 1: ActiveProjectNotResolved

```gherkin
Given the active project cannot be determined
  (no explicit configuration and the graph directory does not correspond
   to a LucidPM project)
When the PM invokes any plugin command
Then the plugin does not execute `lucid`
And the PM sees an error message indicating the project could not be determined
And the error message is visible without leaving Logseq Desktop
```

### Failure Path 2: LucidNotAvailable

```gherkin
Given the active project is resolved
And `lucid` is not found on the system PATH at invocation time
When the PM invokes any plugin command
Then the plugin does not execute any LucidPM operation
And the PM sees an error message indicating `lucid` is not available
And the error message is visible without leaving Logseq Desktop
```

### Failure Path 3: CommandFailed

```gherkin
Given the active project is resolved
And `lucid` is available
When the PM invokes a plugin command
And the underlying `lucid` command exits with a non-zero code
Then the PM sees a failure indication containing the error output from `lucid`
And the failure indication is visually distinct from a success indication
And the failure indication is visible without leaving Logseq Desktop
```

## Invariants

- The plugin never modifies any project data itself — all state changes come
  exclusively from delegated `lucid` commands
- The plugin maintains no independent representation of LucidPM project state;
  all project state is owned exclusively by the LucidPM record
- A succeeded command and a failed command never produce the same visible output
- The plugin exposes Sync, Export, and Suggest commands; it does not expose `extract`
- When an explicit project path is configured, it always takes precedence over
  the inferred graph path — there is no scenario in which inference overrides
  an explicit setting
- Project resolution produces a single active project or fails visibly — the plugin
  never silently proceeds without a resolved project
- Executing a plugin command causes the same LucidPM operation to occur as invoking
  the equivalent `lucid` subcommand against the active project

## Preconditions

- Logseq Desktop is the runtime (web and mobile are out of scope)

## Postconditions

- The PM has received visual feedback indicating success or failure, visible
  without leaving Logseq Desktop
- The feedback contains enough information to distinguish successful completion
  from an execution error
- If the command succeeded, project state reflects the operation as defined by
  the delegated command's own contract

## Runtime Artifacts

The plugin itself produces no runtime artifacts — it writes no files and emits
no domain events to the project event log. All artifacts are produced by the
delegated `lucid` commands and are defined by their own contracts.

| Artifact | Storage | Lifecycle |
|---|---|---|
| Explicit project path setting | Plugin configuration | Persists across sessions; not stored in LucidPM project files |

## Failure Classifications

| Failure Name | Trigger Condition | Observable Signal |
|---|---|---|
| ActiveProjectNotResolved | Active project cannot be determined | Error message shown in Logseq; no `lucid` invocation |
| LucidNotAvailable | `lucid` binary not found on PATH at invocation time | Error message shown in Logseq; no operation performed |
| CommandFailed | Delegated `lucid` command exits non-zero | Failure indication shown in Logseq with error output; visually distinct from success |

---

<!-- METADATA -->
status: APPROVED
feature_id: logseq_plugin
approved_by: human
approved_at: 2026-06-09
derived_from_intent: intents/logseq_plugin.md
derived_event_schema: events/logseq_plugin_schema.md

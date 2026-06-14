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
And the PM is informed of the next step required before exported pages become
  visible in Logseq
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

### Operational Path 1: EndpointUnavailable

```gherkin
Given the plugin is running in a sandboxed environment (no child_process available)
And the companion server endpoint is not reachable
  (server not started, port occupied by another process, or connection refused for any reason)
When the PM invokes any plugin command
Then the plugin shows a CompanionServerUnavailable error message
And the error message includes the server port number and instructions to start the server
And the plugin does not crash or hang
```

Note: from the plugin's perspective all connection-level failures are observable as a
single class — the network error does not distinguish between "server down", "port occupied",
or "wrong service". The contract models what is observable.

### Operational Path 2: EndpointTimeout

```gherkin
Given a TCP connection to the companion server endpoint is established
  and an HTTP request is sent
And the server does not return a response within 60 seconds
When the PM invokes any plugin command
Then the plugin shows a CompanionServerTimeout error message
And the error message includes the server port number
And the plugin does not hang beyond the 60-second bound
```

Note: "reachable" is defined as TCP connection established and HTTP request sent.
Failures before TCP connection are `CompanionServerUnavailable`; failures after response
received are `MalformedServerResponse`. The three classes are non-overlapping.

### Operational Path 3: MalformedResponse

```gherkin
Given the companion server is running and reachable
And the server returns a response that is not valid JSON or is missing required fields
When the PM invokes any plugin command
Then the plugin shows a MalformedServerResponse indication
And the message includes the phrase "invalid server response"
And the plugin does not crash
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
- When an explicit project path is configured, it always takes precedence over
  the inferred graph path — there is no scenario in which inference overrides
  an explicit setting
- Project resolution produces a single active project or fails visibly — the plugin
  never silently proceeds without a resolved project
- Executing a plugin command causes the same LucidPM operation to occur as invoking
  the equivalent `lucid` subcommand against the active project
- A successful Export always informs the PM of the next step required before
  exported pages appear in Logseq — this hint is never omitted, regardless of
  the content of the `lucid export` output

## Preconditions

- Logseq Desktop is the runtime (web and mobile are out of scope)

## Postconditions

- The PM has received visual feedback indicating success or failure, visible
  without leaving Logseq Desktop
- The feedback contains enough information to distinguish successful completion
  from an execution error
- If the command succeeded, project state reflects the operation as defined by
  the delegated command's own contract
- If Export succeeded, the PM is informed of the next step required before
  exported pages appear in Logseq

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
| ActiveProjectNotResolved | Active project cannot be determined | Error message in Logseq; no `lucid` invocation |
| LucidNotAvailable | `lucid` binary not found on PATH (direct child_process path) | Error message in Logseq; no operation performed |
| CompanionServerUnavailable | Companion server endpoint not reachable for any reason (connection refused, wrong port, server not started) | Error message with port number and start instructions; no crash or hang |
| CompanionServerTimeout | TCP established and HTTP request sent, no response within 60 seconds | Error message with port number; plugin returns within 60s bound |
| MalformedServerResponse | Companion server returns invalid JSON or response missing required fields | Failure indication containing "invalid server response"; no crash |
| CommandFailed | Delegated `lucid` command exits non-zero | Failure indication with error output from `lucid`; visually distinct from success |

---

<!-- METADATA -->
status: APPROVED
feature_id: logseq_plugin
approved_by: human
approved_at: 2026-06-09
amended_at: 2026-06-13 (R14: Export next-step guidance in success indication)
amendment: HP2 Export: added next-step postcondition; new invariant: Export always informs PM of next step; Postconditions: Export next-step added
derived_from_intent: intents/logseq_plugin.md
derived_event_schema: events/logseq_plugin_schema.md

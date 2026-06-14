# Behavioral Contract: R13_logseq_plugin_extract

<!--
DERIVED FROM: intents/R13_logseq_plugin_extract.md
-->

## Runtime Context

**Execution environment:** Logseq Desktop (Electron renderer sandbox)
**External boundaries:** Logseq page API (current page detection), Logseq graph API
  (vault path), companion HTTP server, `lucid extract` CLI
**Environment assumptions:** Logseq Desktop is running with a page open; companion
  server is running and reachable; `lucid` is available on PATH
**Environment-sensitive behavior:** journal page determination depends on the Logseq runtime
**Observation mode:** `external-observation`
**Observation artifact:** plugin/ACCEPTANCE.md (Verification Ladder + manual checklist)
**Minimum runtime evidence:** 4 — must be observed in real Logseq Desktop with a real
  journal page and companion server running

## Scenarios

### Happy Path 1: Successful Extraction

```gherkin
Given the PM is in Logseq Desktop
And a journal page is currently open
And the active project is resolved
And `lucid` is available
When the PM invokes the Extract command via the plugin
Then extraction is performed against the currently open journal page
And the PM sees the extraction result
And the success indication informs the PM that extracted items are now in the
  project record
And the success indication informs the PM how extracted items become visible
  in Logseq
And no terminal or external application is opened
```

### Happy Path 2: No Items Found

```gherkin
Given a journal page is currently open
And the active project is resolved
And `lucid` is available
And invoking `lucid extract` on the page would produce no extracted items
When the PM invokes the Extract command
Then the PM sees a message indicating no items were extracted from this page
```

### Happy Path 3: Re-extraction of Previously Extracted Page (Unchanged)

```gherkin
Given the currently open journal page has been successfully extracted before
And no new content has been added since the previous extraction
When the PM invokes the Extract command again
Then extraction completes without error
And the result is consistent with running `lucid extract --yes` with the file content on stdin from the CLI
And the PM sees the output produced by extraction
```

### Happy Path 4: Re-extraction After New Content Added

```gherkin
Given the currently open journal page has been successfully extracted before
And new content has been appended to the page since the previous extraction
When the PM invokes the Extract command again
Then extraction completes without error
And the result is consistent with running `lucid extract --yes` with the file content on stdin from the CLI
And the PM sees the output produced by extraction
```

Note: the plugin delegates extraction via stdin (`lucid extract --yes < file.md`), not via a
file-path argument. In stdin invocation mode, `lucid extract` performs no file-path
deduplication. Re-invoking Extract on a previously extracted page will process the content
again; the resulting project record state is consistent with running `lucid extract --yes` on
stdin twice. Deduplication requires folder-mode invocation and is outside the scope of this
feature.

### Failure Path 1: NotAJournalPage

```gherkin
Given the currently open page in Logseq Desktop is not a journal page
When the PM invokes the Extract command
Then no extraction is performed
And the PM sees an error message indicating the current page is not a journal page
And the error message is visible without leaving Logseq Desktop
```

### Failure Path 2: NoCurrentPage

```gherkin
Given Logseq Desktop is open
And no page is currently open in the editor
When the PM invokes the Extract command
Then no extraction is performed
And the PM sees an error message indicating no page is currently open
And the error message is visible without leaving Logseq Desktop
```

### Failure Path 3: ActiveProjectNotResolved

```gherkin
Given a journal page is currently open
And the active project cannot be determined
When the PM invokes the Extract command
Then the plugin does not invoke `lucid extract`
And the PM sees an error message indicating the project could not be determined
And the error message is visible without leaving Logseq Desktop
```

### Failure Path 4: LucidNotAvailable

```gherkin
Given a journal page is currently open
And the active project is resolved
And `lucid` is not found on the system PATH at invocation time
When the PM invokes the Extract command
Then the plugin does not execute any LucidPM operation
And the PM sees an error message indicating `lucid` is not available
And the error message is visible without leaving Logseq Desktop
```

### Failure Path 5: CommandFailed

```gherkin
Given a journal page is currently open
And the active project is resolved
And `lucid` is available
When the PM invokes the Extract command
And `lucid extract` exits with a non-zero code
Then the PM sees a failure indication containing the error output from `lucid`
And the failure indication is visually distinct from a success indication
And the failure indication is visible without leaving Logseq Desktop
```

Note: companion server failure modes (CompanionServerUnavailable, CompanionServerTimeout,
MalformedServerResponse) apply to this command as specified in the parent feature contract
(contracts/logseq_plugin_contract.md — "any plugin command" clauses). They are not
re-specified here.

### Boundary Scenario: Empty Journal Page

```gherkin
Given the currently open page is a journal page
And the page contains no text content beyond Logseq system properties
When the PM invokes the Extract command
Then the PM sees a message indicating no items were extracted
```

Note: an empty journal page is an extreme case of HP2. This scenario exists to prevent
treating an empty file as an error rather than a "no items" result.

### Falsification Scenario: Extract Always Targets Currently Open Page

```gherkin
Given the PM previously invoked Extract on journal page A
And the PM has since navigated to and opened journal page B
When the PM invokes Extract again
Then extraction operates on journal page B
And journal page A is not re-extracted
Falsifies: implementation resolves the page once (at registration or first invocation)
  and reuses the cached path, rather than resolving the currently open page at each
  invocation
```

## Invariants

- The command always operates on the currently open journal page — it does not cache,
  default to, or infer a different page between invocations
- Extracting a journal page via the plugin produces the same result as running
  `lucid extract --yes` with the page's vault file content piped to stdin from the
  active project directory — there is no behavioral divergence between the plugin
  and the CLI stdin invocation mode
- The command does not invoke `lucid extract` unless the currently open page is a
  journal page
- Successful extraction always informs the PM how extracted items become visible in Logseq
- The plugin never modifies any project data itself — all extraction is performed
  exclusively by the delegated `lucid extract` command

## Vocabulary Dependency

None — this feature delegates to `lucid extract` without inspecting or transforming
project content.

## Invariant Falsification Scenarios

| Invariant | Falsifying fixture | Observable when correct | Wrong implementation assumption | Test ID |
|---|---|---|---|---|
| Command operates on currently open page | PM extracts journal A, navigates to journal B, invokes Extract again | Journal B is extracted; journal A record is unaffected by second invocation | Plugin caches the resolved page path at first invocation and reuses it | R13-INV-1 (behavioral) |
| CLI stdin equivalence | Extract same journal file via plugin and via `lucid extract --yes < file.md` from terminal; compare project record state | Identical project record state | Plugin transforms, truncates, or re-encodes page content before passing to companion server | R13-INV-2 (serialization) |
| Non-journal page guard | PM opens a regular (non-journal) Logseq page; invokes Extract | NotAJournalPage error shown; no `lucid extract` invoked | Plugin checks only whether page name resembles a date rather than whether Logseq identifies the page as a journal page | R13-INV-3 (behavioral) |
| Success notification informs about visibility | Successful extraction | Notification always contains guidance on how items become visible in Logseq | Notification text is constructed conditionally and omits visibility guidance when output is long or in specific output formats | R13-INV-4 (behavioral) |
| Plugin never modifies project data itself | Plugin writes directly to project record before delegating to `lucid extract`; `lucid extract` then produces the correct final state | Project record state before and after plugin invocation is identical to what `lucid extract` alone would produce — no additional writes present | Treating the plugin as a coordinator that may pre-process or pre-write project state before calling `lucid extract`, rather than a pure delegation layer | MANUAL-PENDING (L6) |

## Preconditions

- Logseq Desktop is the runtime (web and mobile are out of scope)
- The companion server is running (or failure is observable per parent contract)

## Postconditions

- The PM has received visual feedback (success or failure) visible without leaving
  Logseq Desktop
- On success: the extraction operation completed successfully; PM has been informed
  how to surface items in Logseq
- On no items found: PM has been informed; project record is unchanged
- On failure: PM has received a specific error message

## Runtime Artifacts

The Extract command produces no plugin-level artifacts — all artifacts are created
by the delegated `lucid extract` and are governed by its own contract.

| Artifact | Path | Lifecycle |
|---|---|---|
| (none beyond those produced by `lucid extract`) | — | — |

### Cross-module signals relied upon

| Event | Source module | When relied upon |
|---|---|---|
| (none — `lucid extract` output is consumed as opaque text via companion server HTTP response) | — | — |

## Failure Classifications

| Failure Name | Trigger Condition | Observable Signal |
|---|---|---|
| NotAJournalPage | Currently open page is not a journal page | Error message in Logseq; no `lucid extract` invoked |
| NoCurrentPage | No page is currently open in the Logseq editor | Error message in Logseq; no `lucid extract` invoked |
| ActiveProjectNotResolved | Active project cannot be determined | Error message in Logseq; no `lucid` invocation |
| LucidNotAvailable | `lucid` binary not found on PATH | Error message in Logseq; no operation performed |
| CommandFailed | `lucid extract` exits non-zero | Failure indication with error output; visually distinct from success |

Note: CompanionServerUnavailable, CompanionServerTimeout, and MalformedServerResponse
apply to this command as contracted in the parent feature and are not re-specified here.

---

**Parent contract amendment required:** `contracts/logseq_plugin_contract.md` contains
the invariant "The plugin does not expose `extract`." This must be removed when R13
is implemented (Stage 4).

---

<!-- METADATA -->
status: APPROVED
feature_id: R13_logseq_plugin_extract
approved_by: human
approved_at: 2026-06-13
amended_at: 2026-06-13
amendment: clarified invocation mode as stdin (lucid extract --yes < file.md), not file-argument; added no-deduplication note to HP3/HP4; updated CLI equivalence invariant and falsification row accordingly
derived_from_intent: intents/R13_logseq_plugin_extract.md
derived_event_schema: events/R13_logseq_plugin_extract_schema.md

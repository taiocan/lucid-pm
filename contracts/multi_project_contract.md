# Behavioral Contract: multi_project

<!--
DERIVED FROM: intents/multi_project.md
-->

## Scenarios

### Happy Path 1: Successful Project Init

```gherkin
Given a project name that does not already exist in the registry
And the directory path supplied via --dir is writable
When the PM runs `multi_project init "<name>" --dir <path>`
Then a new project directory is created at the specified path
And the project is registered in the registry under the given name
And a ProjectInitialized event is emitted containing the project name and directory path
And running other module binaries from that directory works correctly
```

### Happy Path 2: List Projects — Registry Non-empty

```gherkin
Given the registry contains one or more registered projects
When the PM runs `multi_project list`
Then a ProjectListReturned event is emitted
And each registered project's name and directory path is displayed to the PM
```

### Happy Path 3: List Projects — Registry Empty

```gherkin
Given the registry contains no registered projects
When the PM runs `multi_project list`
Then a ProjectListReturned event is emitted with zero projects
And the PM receives a message indicating no projects are registered
```

### Happy Path 4: Open a Registered Project

```gherkin
Given the registry contains a project with the requested name
When the PM runs `multi_project open "<name>"`
Then the project's directory path is printed to stdout
And a ProjectPathReturned event is emitted containing the project name and path
```

### Failure Path 1: ProjectNameAlreadyExists

```gherkin
Given the registry already contains a project with the same name
When the PM runs `multi_project init "<name>" --dir <path>`
Then a ProjectInitFailedDuplicate event is emitted
And the existing project's registration and data are unchanged
And no new directory is created
```

### Failure Path 2: DirectoryNotAccessible

```gherkin
Given the directory path supplied via --dir cannot be created or written to
When the PM runs `multi_project init "<name>" --dir <path>`
Then a ProjectInitFailedDirectoryNotAccessible event is emitted
And the registry is not modified
And the project registry is not modified
```

### Failure Path 3: ProjectNotFound

```gherkin
Given the registry does not contain a project with the requested name
When the PM runs `multi_project open "<name>"`
Then a ProjectOpenFailedNotFound event is emitted
And no directory changes occur
```

## Invariants

- Each project's event log, items, and history are never visible to or
  affected by any other project
- A project name is unique in the registry at all times
- Registering a new project never modifies any existing project's directory
  or data
- No existing module binary (pm_structuring, project_state, item_status,
  logseq_export, logseq_sync) changes its behaviour as a result of this feature

## Preconditions

- The PM has access to a writable filesystem location for the registry
- For `open` and `list`: the registry file exists (created on first `init`)

## Postconditions

- After a successful `init`: the project directory exists and is registered;
  the PM can immediately run other module binaries from that directory
- After a successful `list`: the PM has seen the current state of the registry
- After a successful `open`: the PM has the project's directory path and can
  navigate to it

## Runtime Artifacts

| Artifact | Path | Lifecycle |
|---|---|---|
| Project registry | `~/.lucidpm/projects.json` | Created on first `init`; updated on each `init` |
| Project directory | Path supplied via `--dir` at `init` time | Created on `init`; never deleted by this module |
| events/runtime_events.jsonl | Inside each project directory | Append-only; one per project |
| Registry event log | `<registry_dir>/events.jsonl` | Created on first invocation; append-only |

## Failure Classifications

| Failure Name | Trigger Condition | Observable Signal |
|---|---|---|
| ProjectNameAlreadyExists | Registry already contains a project with the same name | `ProjectInitFailedDuplicate` event emitted; init aborts |
| DirectoryNotAccessible | `--dir` path cannot be created or written to | `ProjectInitFailedDirectoryNotAccessible` event emitted; init aborts |
| ProjectNotFound | Registry does not contain the requested project name | `ProjectOpenFailedNotFound` event emitted |

---

<!-- METADATA -->
status: APPROVED
feature_id: multi_project
approved_by: human
approved_at: 2026-05-26
derived_from_intent: intents/multi_project.md
derived_event_schema: events/multi_project_schema.md

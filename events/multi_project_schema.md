# Event Schema: multi_project

<!--
DERIVED FROM:
- intents/multi_project.md
- contracts/multi_project_contract.md
-->

## Naming Convention

See `docs/conventions.md`.

## Required Base Fields (all events)

```json
{
  "event_id": "uuid-v4",
  "event_type": "EventName",
  "timestamp": 1710000000000,
  "correlation_id": "uuid-v4",
  "source_module": "multi_project",
  "payload": {}
}
```

`correlation_id` is mandatory and must propagate through the entire
execution chain for each command invocation.

## Registry Event Log Note

`multi_project` operates across projects and has no single project directory
as its working context. Its events are written to `<registry_dir>/events.jsonl`
where `<registry_dir>` defaults to `~/.lucidpm/` and may be overridden via
`--registry <path>` for testing. Per-project `events/runtime_events.jsonl`
files are unaffected by `multi_project` events.

## Event Definitions

### ProjectInitRequested

- category: OBSERVATIONAL
- emitted when: the PM triggers `multi_project init`
- payload:
  - `project_name`: `string` — the name supplied by the PM
  - `project_dir`: `string` — the directory path supplied via `--dir`

### ProjectInitialized

- category: BEHAVIORAL
- emitted when: the project directory was created and the project was
  successfully registered under the given name
- payload:
  - `project_name`: `string` — the registered project name
  - `project_dir`: `string` — the absolute path to the created directory

### ProjectInitFailedDuplicate

- category: FAILURE
- emitted when: the registry already contains a project with the same name;
  no directory is created and the registry is unchanged
- payload:
  - `failure_reason`: `string` — `"project_name_already_exists"`
  - `project_name`: `string` — the duplicate name that was requested

### ProjectInitFailedDirectoryNotAccessible

- category: FAILURE
- emitted when: the directory path supplied via `--dir` cannot be created
  or written to; the registry is not modified
- payload:
  - `failure_reason`: `string` — `"directory_not_accessible"`
  - `project_name`: `string` — the project name that was requested
  - `project_dir`: `string` — the path that could not be created

### ProjectListRequested

- category: OBSERVATIONAL
- emitted when: the PM triggers `multi_project list`
- payload: `{}` (no parameters)

### ProjectListReturned

- category: BEHAVIORAL
- emitted when: the registry was read and the project list was returned;
  emitted for both non-empty and empty registries
- payload:
  - `project_count`: `u32` — number of registered projects (0 if none)
  - `projects`: `array` — list of `{ "name": string, "dir": string }` objects;
    empty array if no projects are registered

### ProjectOpenRequested

- category: OBSERVATIONAL
- emitted when: the PM triggers `multi_project open`
- payload:
  - `project_name`: `string` — the name requested

### ProjectPathReturned

- category: BEHAVIORAL
- emitted when: the requested project was found in the registry and its
  directory path was returned
- payload:
  - `project_name`: `string` — the project name
  - `project_dir`: `string` — the directory path printed to stdout

### ProjectOpenFailedNotFound

- category: FAILURE
- emitted when: the registry does not contain a project with the requested name
- payload:
  - `failure_reason`: `string` — `"project_not_found"`
  - `project_name`: `string` — the name that was not found

## Event Flow

```text
init command:
  ProjectInitRequested
    ↓ (name already in registry)
  ProjectInitFailedDuplicate              ← init aborts

    ↓ (--dir path not writable)
  ProjectInitFailedDirectoryNotAccessible ← init aborts

    ↓ (success)
  ProjectInitialized

list command:
  ProjectListRequested
    ↓ (always succeeds — empty registry is not a failure)
  ProjectListReturned

open command:
  ProjectOpenRequested
    ↓ (name not in registry)
  ProjectOpenFailedNotFound

    ↓ (name found)
  ProjectPathReturned
```

## Coverage Check

| Contract Scenario | Event(s) | Status |
|---|---|---|
| HP-1: Successful init | ProjectInitRequested + ProjectInitialized | COVERED |
| HP-2: List non-empty | ProjectListRequested + ProjectListReturned (count > 0) | COVERED |
| HP-3: List empty | ProjectListRequested + ProjectListReturned (count = 0) | COVERED |
| HP-4: Open registered project | ProjectOpenRequested + ProjectPathReturned | COVERED |
| FP-1: ProjectNameAlreadyExists | ProjectInitFailedDuplicate | COVERED |
| FP-2: DirectoryNotAccessible | ProjectInitFailedDirectoryNotAccessible | COVERED |
| FP-3: ProjectNotFound | ProjectOpenFailedNotFound | COVERED |

---

<!-- METADATA -->
status: APPROVED
feature_id: multi_project
approved_by: human
approved_at: 2026-05-26
derived_from_intent: intents/multi_project.md
derived_from_contract: contracts/multi_project_contract.md

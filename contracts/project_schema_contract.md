# Behavioral Contract: project_schema

DERIVED FROM: intents/project_schema.md

## Scenarios

### Happy Path 1: Valid project vocabulary loaded — applied to command output

```gherkin
Given a valid project-level vocabulary definition is configured
And the vocabulary defines entity types, relation types, and label mappings
When the PM runs any LucidPM command that produces project output
Then the output uses the entity type names and labels defined in the vocabulary
And entity types not defined in the vocabulary do not appear in the output
```

### Happy Path 2: Project vocabulary takes precedence over shared default

```gherkin
Given a shared default vocabulary is accessible
And a project-level vocabulary definition is configured that redefines one or more terms
When the PM runs any LucidPM command
Then the project-level definitions are used where they differ from the default
And vocabulary terms not overridden retain their default definitions
And the command completes successfully
```

### Happy Path 3: No project vocabulary — shared default used

```gherkin
Given no project-level vocabulary definition is configured
And a shared default vocabulary is accessible
When the PM runs any LucidPM command
Then the command uses the shared default vocabulary
And the command completes successfully
```

### Happy Path 4: Renamed entity type — existing project data accessible

```gherkin
Given a project-level vocabulary defines entity type Y with entity type X listed as an alias
And the project record contains items recorded under type X
When the PM runs a command that reads the project record
Then items recorded as type X are accessible as type Y in the output
And no items are missing or inaccessible due to the rename
```

### Happy Path 5: Task marker mapping applied in status-based queries

```gherkin
Given task data is available to the command
And a project-level vocabulary defines mappings from task markers to project statuses
When the PM runs a command that filters or compares items by status
Then task-type items are evaluated using their mapped status, not their literal marker
And page-type items are matched by their stored status value
```

### Failure Path 1: SchemaNotFound

```gherkin
Given no project-level vocabulary definition is configured
And no shared default vocabulary is accessible
When the PM runs any LucidPM command
Then the command reports an error and does not complete successfully
And no changes are made to project state
```

### Failure Path 2: SchemaParseError

```gherkin
Given a project-level vocabulary definition is configured
And the definition contains a syntax error or is missing a required structural field
When the PM runs any LucidPM command
Then the command reports an error identifying the parse failure
And no changes are made to project state
```

### Failure Path 3: SchemaValidationError

```gherkin
Given a project-level vocabulary definition parses without syntax errors
And the definition violates a structural rule
  (e.g., a uses: entry references an undefined property,
   a renderer mapping references an undefined relation)
When the PM runs any LucidPM command
Then the command reports an error identifying the violated rule
And no changes are made to project state
```

### Failure Path 4: AliasCollision

```gherkin
Given a project-level vocabulary defines entity type A
And a second entity type B declares A as an alias
When the vocabulary definition is loaded
Then vocabulary validation fails with an error identifying the collision
And no changes are made to project state
```

### Non-aborting condition: SchemaTypeUnknownWarning

```gherkin
Given the vocabulary definition loads successfully
And the project record contains items whose type does not match any defined type or alias
When the PM runs a command that reads the project record
Then those items are excluded from the output with a warning
And the command completes successfully
And all other items are unaffected
```

---

## Invariants

- The vocabulary definition is read at the start of each command — no vocabulary state persists between command executions
- Project-level vocabulary definitions take precedence over shared default definitions for all overlapping terms
- A command that encounters a vocabulary error (parse or validation) makes no changes to project state
- Items whose type matches a vocabulary alias are treated as the aliased canonical type in all outputs
- Task marker-to-status mappings apply uniformly across all commands that compare or filter items by status

## Preconditions

- The command is invoked in the context of a valid LucidPM project
- Vocabulary accessibility is evaluated during command execution, not as an external precondition

## Postconditions

- The command recognizes all entity types defined in the active vocabulary
- The command recognizes all relation types defined in the active vocabulary
- All label mappings defined in the active vocabulary are applied in command output
- Task marker-to-status mappings are applied when the command filters or compares items by status

## Runtime Artifacts

| Artifact | Description | Lifecycle |
|---|---|---|
| Project vocabulary definition | Project-scoped vocabulary configuration | Read at command startup; never written by this feature |
| Shared default vocabulary | Fallback vocabulary for all projects | Read when no project definition exists, or for merge; never written by this feature |

No new artifacts are created or modified by this feature at runtime.

## Failure Classifications

| Failure Name | Trigger Condition | Observable Signal |
|---|---|---|
| SchemaNotFound | No project vocabulary and no shared default accessible | Command reports error and does not complete successfully; project state unchanged |
| SchemaParseError | Vocabulary definition has syntax error or missing required field | Command reports error identifying parse failure; project state unchanged |
| SchemaValidationError | Vocabulary parses but violates a structural rule (undefined property ref, undefined relation ref, etc.) | Command reports error identifying the violated rule; project state unchanged |
| AliasCollision | An alias value matches another type's canonical name or another alias | Command reports error identifying the collision; project state unchanged |
| SchemaTypeUnknownWarning | Item type in event log has no match in vocabulary (no alias match) | Warning reported; item excluded from output; command completes successfully |

---

status: APPROVED
feature_id: project_schema
approved_by: human
approved_at: 2026-05-31
derived_from_intent: intents/project_schema.md
derived_event_schema: events/project_schema_schema.md

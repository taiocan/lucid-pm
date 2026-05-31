# F4: `multi_project`

**Tier**: 2 — Interaction
**Depends on**: none
**Event spine impact**: New schema (new feature)
**Status**: COMPLETE

---

**Intent sketch**
The PM can manage multiple distinct projects, each isolated in its own directory with its own event log. A lightweight project registry makes it ergonomic to create, list, and switch between projects without manual directory management.

**Architecture**
- Each project lives in its own directory (e.g., `~/lucidpm/smart-home/`, `~/lucidpm/crm/`)
- Each project directory has its own `events/runtime_events.jsonl` — complete isolation
- Registry file at `~/.lucidpm/projects.json` maps project names to directories
- All existing binaries (pm_structuring, project_state, logseq_export, etc.) run from the project directory as before — no changes to their CLIs

**CLI**
```
multi_project init "SmartHome Hub v2"   # creates directory, registers it
multi_project list                       # lists all registered projects
multi_project open "SmartHome Hub v2"   # prints directory path (for cd or subshell)
```

**Runtime artifact** (must be declared in contract)
- `~/.lucidpm/projects.json` — project registry (created on first `init`)
- Project directories under user-specified base path

**Integration with existing features**
- Zero changes to pm_structuring, project_state, item_status, or logseq_export schemas
- Existing binaries continue to work from their project directory unchanged

**Event spine (new)**
```
ProjectInitRequested        ← OBSERVATIONAL
  ↓
  ├─ (project name already exists)
  │    ProjectInitFailedDuplicate
  │
  └─ (new project)
       ProjectInitialized

ProjectListRequested        ← OBSERVATIONAL
  ↓
  ProjectListReturned
```

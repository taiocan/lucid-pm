# LucidPM

Command-line project management assistant. Paste meeting notes or project text — LucidPM extracts structured items (tasks, milestones, risks, issues, stakeholders), tracks their status, and exports everything to Logseq as navigable pages.

All state lives in an append-only event log (`events/runtime_events.jsonl`). No database.

---

## Install

Requires Rust (stable) and a [Gemini API key](https://aistudio.google.com) for extraction and AI suggestions.

```bash
git clone https://github.com/taiocan/lucid-pm.git
cd lucid-pm
export GEMINI_API_KEY_PMCLI=your_key_here
./install.sh
```

Installs `lucid` and all feature binaries to `~/.local/bin/`. Add that to your `PATH` if needed.

---

## Quick start

```bash
# Create a project
lucid project init "My Project" --dir ~/projects/my-project
cd ~/projects/my-project

# Extract items from meeting notes
lucid extract

# View the project record
lucid state view

# Set status on an item
lucid status set-status <item_id> doing

# Export to Logseq
lucid export --output-dir ~/logseq/pages

# Sync status changes made in Logseq back
lucid sync --graph ~/logseq

# Validate your project vocabulary schema
lucid schema validate
```

---

## Commands

| Command | What it does |
|---|---|
| `lucid extract` | Extract PM items from stdin or `--folder <path>` of notes |
| `lucid state view` | Show all items in the project record |
| `lucid status set-status <id> <status>` | Update item lifecycle status |
| `lucid status set-priority <id> <high\|medium\|low>` | Set item priority |
| `lucid link add <src> <type> <target>` | Create typed link between items |
| `lucid export --output-dir <path>` | Export project record to Logseq pages |
| `lucid sync --graph <path>` | Sync Logseq edits back into the event log |
| `lucid project init <name> --dir <path>` | Create and register a new project |
| `lucid project list` | List all registered projects |
| `lucid priority` | Priority-ranked view of open items |
| `lucid report --type <type>` | Generate report (full, weekly, risk-register, stakeholders) |
| `lucid suggest propose` | AI-generated link, status, and priority proposals |
| `lucid suggest confirm <id> --accept-all` | Apply accepted proposals as events |
| `lucid journal new --title <title>` | Create a dated journal entry |
| `lucid schema validate` | Validate `project-schema.yaml` |
| `lucid schema show` | Print the resolved vocabulary schema |

---

## Project vocabulary

Each project can define its own entity types, relation types, and Logseq rendering labels via `project-schema.yaml` in the project directory. A default schema covering `WorkPackage`, `Milestone`, `Risk`, `Issue`, `Stakeholder`, and `Task` is installed to `~/.lucidpm/default-schema.yaml`.

```yaml
# project-schema.yaml — override or extend the default
schemaVersion: 1
pageTypes:
  Workstream:          # rename WorkPackage for this project
    uses: [status, priority, deadline]
    aliases:
      - WorkPackage    # keeps old event log entries visible
statuses:
  active:
  waiting:
  done:
  cancelled:
```

---

## Architecture

- Each feature is a standalone Rust binary. `lucid` dispatches to them.
- State is event-sourced: `events/runtime_events.jsonl` is append-only and is the only source of truth.
- Multiple projects are isolated by directory. Registry at `~/.lucidpm/projects.json`.
- Built using [Codeos DBA methodology](https://github.com/taiocan/lucid-pm/tree/master/.codeos) — every feature has an approved intent, behavioral contract, and event schema before any code is written.

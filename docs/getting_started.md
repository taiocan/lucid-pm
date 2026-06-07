# LucidPM — Getting Started

LucidPM is a command-line project management assistant. It extracts structured project data from your notes and meeting minutes, tracks items and their status, and keeps a complete audit trail of everything as an append-only event log.

---

## Prerequisites

- **Rust** — [rustup.rs](https://rustup.rs) (stable toolchain)
- **Gemini API key** — required for AI features (`extract`, `suggest`)
  ```bash
  export GEMINI_API_KEY_PMCLI=your_key_here
  ```
  Add this to your `~/.bashrc` or `~/.zshrc` to persist it.

---

## Installation

```bash
git clone <repo-url> LucidPM
cd LucidPM
./install.sh
```

This builds all 13 feature binaries in release mode and installs them alongside the `lucid` dispatcher into `~/.local/bin/`. If that directory is not in your `PATH`, the script tells you how to add it.

To install elsewhere:
```bash
INSTALL_DIR=/usr/local/bin ./install.sh
```

Verify the install:
```bash
lucid help
```

---

## Project setup

LucidPM is project-centric — run its commands from inside a project directory. Each project keeps its own record.

```bash
mkdir my-project
cd my-project
mkdir events          # required: event log lives here
mkdir journal         # optional: for notes and meeting minutes
```

That's it. No init command needed.

---

## Daily workflow

### 1. Extract items from notes

Paste meeting notes or emails and LucidPM extracts tasks, milestones, risks, issues, and stakeholders:

```bash
lucid extract
```

Or process a whole folder of notes at once (skips files already processed):

```bash
lucid extract --folder journal/ --yes
```

### 2. Incorporate and view the project record

After extracting, incorporate the confirmed session into the project record:

```bash
lucid state incorporate <correlation_id>   # correlation_id from ExtractionConfirmed event
lucid state view                           # show all items
```

### 3. Update item status or priority

```bash
lucid status set-status <item_id> doing
lucid status set-status <item_id> done
lucid status set-priority <item_id> high
lucid status get <item_id>                 # query current status and priority
```

### 4. Write journal entries

```bash
lucid journal new --title "standup 2026-05-29"   # creates journal/<date>-standup-....md
lucid journal list                                # list all entries, most recent first
lucid journal open <filename>                     # print the path to open in your editor
```

### 5. Link related items

Valid link types: `blocks`, `affects`, `assigned_to`, `mitigated_by`, `escalates_to`, `related_to`

```bash
lucid link add <source_id> blocks <target_id>
lucid link add <source_id> assigned_to <target_id>
lucid link remove <source_id> <link_type> <target_id>
lucid link list
```

### 6. AI enrichment

Get AI-suggested links, statuses, and priorities across the full record:

```bash
lucid suggest propose
lucid suggest confirm <review_id> --accept-all
lucid suggest confirm <review_id> --accept p-001,p-003
```

### 7. Export to Logseq

```bash
lucid export --output-dir logseq/pages    # writes one .md page per item
```

Sync changes made in Logseq back into the record:

```bash
lucid sync --graph logseq
```

### 8. Reports

```bash
lucid report --type full            # full project report
lucid report --type weekly          # weekly summary
lucid report --type risk-register   # risks only
lucid report --type stakeholders    # stakeholders only
```

### 9. Priority view

```bash
lucid priority                      # all items ranked by priority
lucid priority --type task          # filter by item type
lucid priority --status open        # filter by status
lucid priority --priority high      # filter by priority
```

---

## Multiple projects

```bash
lucid project init client-alpha --dir ~/projects/client-alpha   # create and register
lucid project list                                               # list all projects
lucid project open client-alpha                                  # print project path
```

Each project has its own isolated `events/` directory and record. Navigate to the project directory before running any commands.

---

## How it works

Every command appends events to `events/runtime_events.jsonl` in your project directory. This file is the single source of truth — it is never modified, only appended to. The project record is always reconstructed from this log, so nothing is ever lost.

---

## Quick reference

| Goal | Command |
|---|---|
| Extract from stdin | `lucid extract` |
| Extract from folder | `lucid extract --folder journal/ --yes` |
| Incorporate session | `lucid state incorporate <correlation_id>` |
| View all items | `lucid state view` |
| Set item status | `lucid status set-status <id> <status>` |
| Set item priority | `lucid status set-priority <id> <high\|medium\|low>` |
| New journal entry | `lucid journal new --title "..."` |
| List journal entries | `lucid journal list` |
| Link items | `lucid link add <src> <link_type> <tgt>` |
| AI suggestions | `lucid suggest propose` |
| Confirm suggestions | `lucid suggest confirm <review_id> --accept-all` |
| Export to Logseq | `lucid export --output-dir logseq/pages` |
| Sync from Logseq | `lucid sync --graph logseq` |
| Generate report | `lucid report --type full` |
| Priority view | `lucid priority` |
| New project | `lucid project init <name> --dir <path>` |
| List projects | `lucid project list` |
| Help | `lucid help` |

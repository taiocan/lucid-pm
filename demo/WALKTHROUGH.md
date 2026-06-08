# LucidPM Demo Walkthrough
## Nucleus Platform v2 — Launch Readiness

This walkthrough shows the full LucidPM workflow from scratch using a fictional
software project. You will extract items from meeting notes, build a project record,
set statuses and priorities, create links, manage tasks, and see everything in Logseq.

**Time to complete:** 20–30 minutes  
**Prerequisites:** LucidPM installed (`lucid help` should show all commands)  
**Logseq:** Install Logseq Desktop to follow the Logseq steps

---

## Before you start

The `demo/` directory contains two things:
- **`notes/`** — raw meeting notes you will extract from (your starting point)
- **Pre-populated end state** — `events/`, `logseq/`, `journal/` show the finished result

You have two options:
- **Follow from scratch** — work through this guide step by step (recommended)
- **Explore the end state first** — open `logseq/` in Logseq Desktop, then come back here

For the from-scratch path, run all commands from inside the `demo/` directory:

```bash
cd /path/to/LucidPM/demo
```

---

## Phase 1 — Schema

Before extracting anything, tell LucidPM what entity types your project uses.

```bash
lucid schema validate
```

This checks `project-schema.yaml`. You should see it validates cleanly.

```bash
lucid schema show
```

This prints the resolved vocabulary — the entity types, their allowed statuses,
and the link relations defined for this project.

**What you see:** `Milestone`, `Risk`, `Issue`, `Task`, `Stakeholder` with their
allowed statuses and the `blocks`, `affects`, `assignedTo`, `relatedTo` link types.

**Why this matters:** The schema is the vocabulary that governs extraction,
status management, Logseq rendering, and AI proposals throughout the project.

---

## Phase 2 — Extraction

### Extract kickoff notes

```bash
lucid extract < notes/01-kickoff-meeting.md
```

LucidPM reads the notes, identifies PM items, and shows you a proposed extraction.
You will see milestones and stakeholders extracted from the kickoff notes.

Review the proposed items and confirm (`y`) to accept them. LucidPM prints a
session ID — copy it for the next step.

### Incorporate into the project record

```bash
lucid state incorporate <session_id>
```

Replace `<session_id>` with the value from the previous step. This commits the
extracted items into the permanent project record.

### Extract the risk and issue review

```bash
lucid extract < notes/02-risk-and-issue-review.md
```

Confirm the extraction (`y`) — you should see two risks and two issues identified.
Incorporate the second session too:

```bash
lucid state incorporate <second_session_id>
```

### View the project record

```bash
lucid state view
```

**What you see:** 8 items across 2 sessions — 2 milestones, 2 risks, 2 issues,
2 stakeholders.

---

## Phase 3 — Status and Priority

Now give items their operational state. Use the item IDs shown by `lucid state view`.

### Set milestone statuses

```bash
lucid status set-status <milestone_id_1> pending
lucid status set-status <milestone_id_2> pending
```

### Set risk statuses and priorities

```bash
lucid status set-status <risk_id_1> open
lucid status set-priority <risk_id_1> high

lucid status set-status <risk_id_2> open
lucid status set-priority <risk_id_2> high
```

### Set issue statuses and priorities

```bash
lucid status set-status <issue_id_1> in_progress
lucid status set-priority <issue_id_1> high

lucid status set-status <issue_id_2> open
lucid status set-priority <issue_id_2> medium
```

### Set stakeholder statuses

```bash
lucid status set-status <stakeholder_id_1> active
lucid status set-status <stakeholder_id_2> active
```

### See ranked view

```bash
lucid priority
```

**What you see:** All items ranked by priority. Filter by type or status:

```bash
lucid priority --type risk
lucid priority --status open
```

---

## Phase 4 — Links

Link related items to show how risks and issues affect your milestones.

```bash
# Auth risk affects the beta launch milestone
lucid link add <risk_authify_id> affects <milestone_launch_id>

# Database risk blocks the API migration milestone
lucid link add <risk_db_id> blocks <milestone_api_id>

# iOS Safari issue affects the beta launch
lucid link add <issue_ios_id> affects <milestone_launch_id>
```

View all links:

```bash
lucid link list
```

View links for a specific item:

```bash
lucid link list --item <milestone_launch_id>
```

**What you see:** The launch milestone is affected by two items — the auth risk
and the iOS Safari issue. This is the dependency map for your project.

---

## Phase 5 — Logseq Export

Now export the project record to Logseq so you can navigate it visually.

```bash
lucid export --output-dir my-logseq-graph
```

**Open in Logseq Desktop:**
1. Open Logseq Desktop
2. Choose "Add a graph" → select the `my-logseq-graph/` directory
3. Browse the pages

**What you see in Logseq:**
- Each item is a page with `type::`, `status::`, `priority::` properties
- The launch milestone page shows **Affected By** links to the auth risk and iOS issue
- The API migration milestone shows **Blocked By** the database risk
- Navigate between pages by clicking `[[page-links]]`

**Compare with the pre-built graph:** The `logseq/` directory in this demo contains
the pre-exported version. Your freshly exported graph should produce identical pages.

---

## Phase 6 — Tasks

Add a task to the auth risk item. Tasks are nested under a parent record item.

```bash
lucid task add \
  --description "Add auth retry logic with exponential backoff" \
  --parent <risk_authify_id> \
  --marker TODO
```

View the updated state:

```bash
lucid state view
```

Re-export to see the task rendered under its parent in Logseq:

```bash
lucid export --output-dir my-logseq-graph
```

**What you see in Logseq:** The auth risk page now has a task block nested under
it, with a `TODO` marker. Check it off in Logseq, then sync back (Phase 8).

---

## Phase 7 — Reports

Generate structured reports from the project record.

```bash
# Full project report
lucid report --type full

# Risk register only
lucid report --type risk-register

# Weekly status snapshot
lucid report --type weekly
```

Each report uses the vocabulary and statuses from your schema to group and label items.

---

## Phase 8 — Logseq Sync

Make a change in Logseq and sync it back into the project record.

1. Open your `my-logseq-graph/` in Logseq Desktop
2. Find the `platform-v2-public-beta-launch` page
3. Change `status:: pending` to `status:: in_progress`
4. Save (Logseq autosaves)

Now sync the change back:

```bash
lucid sync --graph my-logseq-graph
```

Verify the change was applied:

```bash
lucid status get <milestone_launch_id>
```

**What you see:** The milestone status is now `in_progress`, updated from Logseq
without touching the CLI directly. This is the core Logseq ↔ LucidPM sync loop.

---

## Phase 9 — Journal

Capture meeting notes as a dated journal entry.

```bash
lucid journal new --title "sprint-planning-2026-06-01"
```

This creates a markdown file in `journal/`. Open it, add your notes, and save.
List all journal entries:

```bash
lucid journal list
```

The `journal/` directory in this demo contains an example entry from the kickoff
standup. Open `2026-06-05-kickoff-standup-2026-05-29.md` to see the format.

---

## Phase 10 — AI Enrichment (optional — requires API key)

> **Requires:** `ANTHROPIC_API_KEY` environment variable set.
> Skip this phase if you do not have an API key — all other phases work without one.

Run the AI proposal engine:

```bash
lucid suggest propose
```

LucidPM analyses your project record and proposes enrichments — suggested links,
status updates, and priority adjustments based on the content of your items.

Review the proposal and accept selectively:

```bash
lucid suggest confirm <review_id> --accept-all
# or
lucid suggest confirm <review_id> --accept p-001,p-002
```

---

## Summary

You have now used the full LucidPM workflow:

| Phase | Feature | Command |
|---|---|---|
| 1 | Vocabulary schema | `lucid schema` |
| 2 | Extraction + incorporation | `lucid extract`, `lucid state` |
| 3 | Status and priority | `lucid status`, `lucid priority` |
| 4 | Typed links | `lucid link` |
| 5 | Logseq export | `lucid export` |
| 6 | Task management | `lucid task` |
| 7 | Reports | `lucid report` |
| 8 | Logseq sync | `lucid sync` |
| 9 | Journal | `lucid journal` |
| 10 | AI enrichment (optional) | `lucid suggest` |

The Logseq graph is your primary navigation surface for the project record. The
terminal is your operational surface for updates. They stay in sync via
`lucid export` and `lucid sync`.

To apply this to your own project:
1. Create a new directory and run `lucid schema validate` with your own schema
2. Run `lucid extract` on your meeting notes
3. Follow this same workflow

---

## What's in this demo directory

```
demo/
├── WALKTHROUGH.md              ← this guide
├── project-schema.yaml         ← vocabulary for this project
├── notes/
│   ├── 01-kickoff-meeting.md   ← raw notes for Phase 2 extraction
│   └── 02-risk-and-issue-review.md
├── events/
│   └── runtime_events.jsonl    ← pre-populated project record (end state)
├── logseq/                     ← pre-exported Logseq graph (end state)
│   ├── logseq/config.edn
│   └── pages/                  ← 9 item pages with links and properties
└── journal/
    └── 2026-06-05-kickoff-standup-2026-05-29.md
```

The `events/` and `logseq/` directories show where you will end up after completing
the walkthrough. They are committed so you can open the graph in Logseq immediately
without running any commands first.

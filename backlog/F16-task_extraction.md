# Feature Brief: F16 — `task_extraction`

<!--
Type: F-type (new feature)
-->

**Type**: F-type  
**Tier**: 7 — Task Layer  
**Depends on**: pm_structuring (R6), task_model (F12), item_links (F7), project_schema (F11)  
**Event spine impact**: Additive — new events for task record creation during extraction; schema defines task and WP vocabulary  
**Status**: BRIEF-DRAFT

---

## Problem / Need

`lucid extract` produces items of type "task" as standalone project record items with no association to a parent work package. In practice, tasks arise in the context of work packages — a block of text that describes a WP will name the tasks that constitute it. After extraction, a PM must manually run `lucid task add --parent <wp_id>` for each task to create a proper task record. This step is invisible to the extraction flow and easy to miss.

The result: work package pages in Logseq show no nested task blocks, even when the source text described tasks clearly. The extraction pipeline discards the structural relationship between tasks and their WP.

**Trigger type**: HUMAN_APPROVED_EVOLUTION

---

## Primary Actor

The PM who runs `lucid extract` on notes or meeting minutes and expects the resulting Logseq graph to reflect the task structure described in the source text without a separate manual step.

---

## Core Outcome (informal)

After this feature exists: when `lucid extract` processes text that describes tasks in the context of a named work package, the extracted output includes task records (conforming to task_model) with their `parent_item_id` set to the relevant WP. After export, those tasks appear as nested blocks under the WP page in Logseq — no separate `lucid task add` step needed for tasks that were unambiguously described in the source text.

---

## Design Tensions and Open Questions

1. **How is "unambiguous WP attribution" defined?** The extraction AI must decide whether a task belongs to a specific WP. Two signal types are sufficient for V1:
   - *Structural hierarchy*: a bold or heading-level item one indent level above a list of bullet-point tasks (e.g. `**Razvojne zmogljivosti**` followed by indented task bullets) — the heading is the WP, the bullets are its tasks. Date markers inline with task bullets (`– junT2`, `- jul`) are extracted as deadline hints on the task record.
   - *Explicit naming*: a task bullet that directly names an existing WP in the project record by slug or alias.
   Flat text with no hierarchy and no WP reference is the ambiguous case — tasks extracted from flat text become unassigned task records (no parent, no WP).

2. **Task records vs. generic items.** Currently, items of type "task" are generic project record items. Task records (from `task_model`) are a distinct entity with `parent_item_id`, `owner_id`, `current_marker`, `scheduled_date`, `deadline`. Should extraction produce full task records (creating TaskAdded events) or generic task items that are later promoted? V1 answer: full task records, using the task_model event path.

3. **WP resolution: what if the named WP doesn't exist yet?** V1 scope: task is extracted as unassigned (no parent). V2 backlog: create the WP if none matches. Automatic WP creation is explicitly out of V1 — it risks inventing project architecture not intended by the PM.

4. **Schema conformance.** WP type, task block type, and WP-task relation type must be read from the project schema (`pageTypes`, `blockTypes`, `relations`) — not hardcoded. The extraction AI prompt must include schema vocabulary, consistent with how R6 works for entity types generally.

5. **WP-task relationship in the data model.** The parent relationship is expressed via `parent_item_id` on the task record (existing task_model field). An item_link of the appropriate schema relation type (e.g., `assignedTo` from WP to task, or a new `contains` relation) is also created to make the relationship queryable via item_links. Which relation type to use is to be settled at Stage 1.

6. **Stage 0 verification required.** Before writing the Stage 1 intent: verify that a task record created with `parent_item_id` pointing to a WP — when that record was not created via `lucid task add` but via an extraction event — correctly flows through `cmd_export()` to appear as a nested block on the WP page. If this path is broken, Stage 4 will need to fix it.

---

## Suspected Dependencies

- **pm_structuring** (R6): extraction already reads schema `pageTypes` for entity vocabulary; F16 extends the AI prompt to include task-WP relational context and `blockTypes`/`relations` vocabulary.
- **task_model** (F12): defines the TaskAdded event and task record structure; extraction must emit TaskAdded (or an equivalent internal event) to create task records.
- **item_links** (F7): the WP-task relationship is stored as an item_link; F16 must emit LinkAdded events alongside TaskAdded events.
- **project_schema** (F11): type slugs, relation type names, and marker defaults are all read from schema at runtime.

---

## Rough Scope Notes

**In scope (V1):**
- Extraction creates task records (not generic items) when tasks are unambiguously attributed to an existing WP in the source text
- Task records have `parent_item_id` set to the resolved WP item
- A schema-declared item_link is created between WP and each task record
- Unambiguous = structural hierarchy (bold/heading WP above indented task bullets) OR explicit WP name reference in the project record
- Inline date markers on task bullets (`– junT2`, `- jul`) extracted as deadline hints on the task record
- Tasks without resolvable WP → unassigned task records (discoverable via Dashboard "Open Tasks" query)
- No automatic WP creation

**Out of scope (V1) — V2 direction:**
- **WP assignment proposals via ontology_suggest**: extend `ontology_suggest` to propose WP assignments for unassigned task records — "Task X appears most related to WP Y based on semantic similarity." The PM reviews and accepts or redirects. This keeps the AI-proposes / human-approves pattern established by the existing suggestion workflow; it does not perform autonomous assignment. Depends on: ontology_suggest (F9), item_links (F7).
- Automatic WP creation when no existing WP matches (autonomous project restructuring — deferred indefinitely)
- "Extract and Surface" command (Extract + Export in one step)
- Owner inference from task context (who is assigned to the task)

---

## Readiness Check

- [x] The problem statement explains WHY, not HOW
- [x] The primary actor is a human role, not "the system"
- [x] The core outcome is stated from the actor's perspective
- [x] At least one open question is listed
- [x] Suspected dependencies are named (even if marked uncertain)
- [x] No actor+outcome DBA form appears anywhere in this brief
- [x] No stable guarantees or DBA scope boundaries appear in this brief
- [x] The feature can be described without mentioning implementation technology
- [x] (F-type) Stage 0 verification requirement is noted

**Brief status**: READY FOR STAGE 1 (pending Stage 0 verification)

---

<!-- METADATA -->
brief_created: 2026-06-13
brief_last_updated: 2026-06-13 (V2 direction added: ontology_suggest WP assignment proposals)
stage1_started:

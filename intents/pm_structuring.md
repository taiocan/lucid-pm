# Intent: pm_structuring

<!--
PURPOSE OF THIS FILE:
Defines why this feature exists and what meaningful outcomes it enables.
This is NOT a requirements document, feature list, or architecture plan.
Intent must remain stable even if implementation changes significantly.

RULES:
- State outcomes, not mechanisms
- Use "Actor can [outcome]" form for every statement
- No implementation details (no APIs, databases, frameworks, file formats)
- No feature decomposition or workflow steps
- No observability mechanics (no events, logs, metrics)
- Guarantees must be enforceable and testable
- Fits on one screen — if it expands into architecture, it is no longer intent
-->

LucidPM exists to let a Project Manager derive structured project information
from unstructured written text.

Specifically:
- PM can extract project tasks, milestones, risks, issues, and stakeholders
  from unstructured text (e.g., meeting notes, emails)
- PM can review and confirm extracted items before they are accepted
- PM can see uncertainty clearly identified when source text is ambiguous
  or incomplete
- PM can see an AI-suggested initial status and priority for each extracted
  item, inferred from the content of the source text

## Stable Guarantees

- Source text is preserved exactly as provided
- Extracted items contain only information present in the source text
- Proposed status and priority values are inferences from the source text,
  not inventions — they are presented as suggestions and only take effect
  if the PM confirms the extraction
- Uncertainty in any extracted item is always visible to the PM before confirmation

## Scope Boundary

This feature does NOT:
- invent status or priority values that have no basis in the source text
- apply proposed status or priority to the project record without PM confirmation
- control how extracted information is stored, rendered, or displayed in any tool

---

<!-- METADATA — fill in when status changes -->
status: APPROVED
feature_id: pm_structuring
approved_by:
approved_at:
derived_contracts: contracts/pm_structuring_contract.md

---

## R2: Folder Ingestion Refinement

*Stage 9 refinement. Extends the extraction pipeline to process a directory of files
rather than a single stdin input, with automatic deduplication against prior runs.*

### Why This Exists

The `journal/` folder (F10) accumulates free-form notes and meeting minutes over time.
Without R2, processing those files requires manually feeding each one to `pm_structuring`
with no protection against processing the same file twice. R2 removes that friction.

### Outcomes

- PM can point `pm_structuring` at a folder and extract items from all files not yet processed
- PM can re-run the same folder command safely — already-ingested files are silently skipped
- PM can see which source file each extracted item came from
- PM can process a folder non-interactively (auto-confirm) for batch workflows

### Stable Guarantees

- A file is processed **at most once**: the event log is the source of truth; no external state file required
- The extraction pipeline is **unchanged** per file: same LLM call, same review step, same confirmation chain
- Source provenance is **preserved in the event record**: `ItemsExtracted` carries a `source_file` field for folder-mode runs (null for interactive stdin sessions)
- Files already processed are **skipped silently**, not re-extracted

### Scope Boundary

This refinement does NOT:
- change the interactive stdin extraction flow in any way
- create, manage, or delete files in the folder
- parse file content differently from how stdin text is parsed today
- enforce any structure on the files it reads
- process files outside `.txt` and `.md` extensions

---

<!-- R2 METADATA -->
status: APPROVED
refinement_id: R2
depends_on: F10 (journal)

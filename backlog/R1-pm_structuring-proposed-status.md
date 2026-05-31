# R1: `pm_structuring` Stage 9 — Proposed Status and Priority

**Tier**: Refine
**Depends on**: F1
**Event spine impact**: Additive (two nullable fields to ItemsExtracted)
**Status**: COMPLETE

---

**What this is**
A Stage 9 Refine pass across two existing features. Not a new feature — no new CLI, no new module. Two nullable fields added to one existing event. One fallback path added to two existing functions.

**Intent**
When the LLM extracts items from meeting notes, it also infers a plausible initial status and priority from the same text. These are presented as suggestions — subject to PM confirmation via the existing ExtractionConfirmed gate. The PM never has to set status/priority from scratch; the LLM does a first pass.

**Examples**
- "urgent bug causing customer data loss" → proposed_priority: high, proposed_status: open (issue)
- "agreed to close the vendor dependency risk" → proposed_status: closed (risk), proposed_priority: null
- "schedule release for EOQ" → proposed_status: pending (milestone), proposed_priority: medium

**Schema change (pm_structuring)**
`ItemsExtracted` items gain two nullable fields:
```json
{
  "proposed_status": "doing",   // null if LLM cannot confidently infer
  "proposed_priority": "high"   // null if LLM cannot confidently infer
}
```
No new events. No changes to any other event.

**Behavior change (item_status)**
`current_status()` and `current_priority()` gain a read-only fallback: if no explicit `ItemStatusUpdated`/`ItemPriorityUpdated` exists for an item, return the proposed value from `ItemsExtracted`. Explicit updates always take precedence. `cmd_get()` marks proposed-but-not-overridden values as `(proposed)`.

**DBA approach**
Stage 9 Refine on both features in order:
1. `pm_structuring` → Stage 2 → 3 → 4 → 5 → 6 → 7 → 8 (schema first)
2. `item_status` → Stage 2 → 3 → 4 → 5 → 6 → 7 → 8

**Module boundary preserved**: `pm_structuring` never emits `item_status` events — it adds fields to its own `ItemsExtracted`. `item_status` reads those as a read-only fallback.

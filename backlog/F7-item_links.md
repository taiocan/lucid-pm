# F7: `item_links`

**Tier**: 4 — Relationships
**Depends on**: project_state, F2
**Event spine impact**: New schema (new feature)
**Status**: COMPLETE

---

**Intent sketch**
The PM can define typed directional links between any two items in the project record. Links are stored as events and rendered by `logseq_export` as real `[[page]]` backlinks on each item's Logseq page, enabling Logseq's graph view to show the project dependency map visually.

**Link types (relationship ontology)**

| Link Type | Source Item Types | Target Item Types | Semantics |
|---|---|---|---|
| `blocks` | task, issue | task, milestone | source must resolve before target can progress |
| `affects` | risk, issue | task, milestone, stakeholder | source introduces uncertainty or impact on target |
| `assigned_to` | task, issue | stakeholder | source is the responsibility of target |
| `mitigated_by` | risk | task | source risk is addressed by target task |
| `escalates_to` | risk, issue | stakeholder | source is escalated to target stakeholder |
| `related_to` | any | any | general association, symmetric |

Links are validated against this matrix at write time — a nonsensical link (e.g., task `affects` stakeholder) is rejected.

**Inverse rendering in Logseq**
When A links to B, both pages show the relationship. B's page renders the inverse label automatically:

| Link Type | Forward Label (on A) | Inverse Label (on B) |
|---|---|---|
| `blocks` | Blocks | Blocked By |
| `affects` | Affects | Affected By |
| `assigned_to` | Assigned To | Owns |
| `mitigated_by` | Mitigated By | Mitigates |
| `escalates_to` | Escalated To | Escalations |
| `related_to` | Related To | Related To |

Each item page in Logseq gets type-specific relationship sections, not a generic `## Links` block:
```markdown
## Blocked By
- [[Task - Fix critical data loss bug]]

## Assigned To
- [[Engineering lead]]
```

**CLI**
```
item_links add <source_id> <link_type> <target_id>    # create a link
item_links remove <source_id> <link_type> <target_id> # remove a link
item_links list [<item_id>]                            # list all links, or links for one item
```

**Runtime artifact**
No files created. Links live in `events/runtime_events.jsonl` as `ItemLinked`/`ItemUnlinked` events. `logseq_export` resolves them to page names at export time.

**Integration with existing features**
- Reads `project_state` events to validate that both item_ids exist in the record
- `logseq_export` must be re-run after adding links to update Logseq pages
- No changes to pm_structuring, project_state, or item_status schemas

**Event spine (new)**
```
LinkRequested               ← OBSERVATIONAL: PM creates or removes a link
  ↓
  ├─ (source or target item_id not in record)
  │    LinkFailedItemNotFound
  │
  ├─ (link_type not valid for the item type combination)
  │    LinkFailedInvalidType
  │
  ├─ (link already exists — for add; link does not exist — for remove)
  │    LinkFailedConflict
  │
  └─ (valid)
       ItemLinked | ItemUnlinked
```

**Logseq rendering note**: `logseq_export` must be extended (Stage 9 refinement of F2) to read
`ItemLinked`/`ItemUnlinked` events and write type-specific relationship sections on each item page,
including inverse labels. This is out of F7's scope — F7 owns storage and validation only.
See **F8: `logseq_export_links`**.

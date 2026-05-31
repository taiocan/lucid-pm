# F1: `item_status`

**Tier**: 1 — Foundation
**Depends on**: project_state
**Event spine impact**: New schema (new feature)
**Status**: COMPLETE

---

**Intent sketch**
The PM can update the lifecycle status of any item in the project record and view the current status of all items. Status is the bridge between static extraction facts and a living project — without it, the record cannot be monitored or acted upon.

**Status vocabularies by item type**

| Item Type | Valid Statuses |
|---|---|
| task | `todo` → `doing` → `done` \| `cancelled` \| `waiting` |
| milestone | `pending` → `achieved` \| `missed` |
| risk | `open` → `mitigated` \| `accepted` \| `closed` |
| issue | `open` → `in_progress` → `resolved` \| `closed` |
| stakeholder | `active` \| `inactive` |

**Logseq alignment**
- Task statuses map to Logseq native markers: `TODO`, `DOING`, `DONE`, `WAITING`, `CANCELLED`
- Other types use Logseq block properties: `status:: open`, `priority:: high`
- Priority property: `high`, `medium`, `low` (optional, set independently of status)

**Integration with existing features**
- Reads `item_id` values from `project_state`'s `ItemsIncorporated` and `RecordReturned` events
- No changes to `pm_structuring` or `project_state` schemas
- CLI: `item_status set <item_id> <status>`, `item_status set <item_id> --priority <level>`

**Event spine (new)**
```
StatusUpdateRequested       ← OBSERVATIONAL: PM initiates a status change
  ↓
  ├─ (item not found in record)
  │    StatusUpdateFailedItemNotFound
  │
  └─ (valid item and status)
       ItemStatusUpdated
```

**Failure modes**
- `ItemNotFound` — item_id does not exist in project record
- `InvalidStatusTransition` — status value not valid for the item's type

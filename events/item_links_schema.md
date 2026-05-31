# Event Schema: item_links

<!--
DERIVED FROM:
- intents/item_links.md (actors, outcomes)
- contracts/item_links_contract.md (state transitions, failure modes)
-->

## Naming Convention

See `docs/conventions.md` (source: `.codeos/templates/conventions.md`).

## Required Base Fields (all events)

```json
{
  "event_id": "uuid-v4",
  "event_type": "EventName",
  "timestamp": 1710000000000,
  "correlation_id": "uuid-v4",
  "source_module": "item_links",
  "payload": {}
}
```

`correlation_id` is mandatory and must propagate through the entire execution chain.

## Valid Link Types

`blocks` | `affects` | `assigned_to` | `mitigated_by` | `escalates_to` | `related_to`

## Event Definitions

### LinkAddRequested

- category: OBSERVATIONAL
- emitted when: PM initiates adding a link
- payload:
  - `source_id`: `string` — item_id of the source item
  - `link_type`: `string` — the relationship type requested
  - `target_id`: `string` — item_id of the target item

### LinkRemoveRequested

- category: OBSERVATIONAL
- emitted when: PM initiates removing a link
- payload:
  - `source_id`: `string` — item_id of the source item
  - `link_type`: `string` — the relationship type to remove
  - `target_id`: `string` — item_id of the target item

### LinkListRequested

- category: OBSERVATIONAL
- emitted when: PM requests a listing of links
- payload:
  - `item_id`: `string | null` — the item to scope the listing to;
    null when listing all links in the record

### ItemLinked

- category: BEHAVIORAL
- emitted when: a link was successfully recorded
- payload:
  - `source_id`: `string` — item_id of the source item
  - `source_type`: `string` — item type of the source item
  - `link_type`: `string` — the relationship type recorded
  - `target_id`: `string` — item_id of the target item
  - `target_type`: `string` — item type of the target item

### ItemUnlinked

- category: BEHAVIORAL
- emitted when: a link was successfully removed
- payload:
  - `source_id`: `string` — item_id of the source item
  - `link_type`: `string` — the relationship type removed
  - `target_id`: `string` — item_id of the target item

### LinkListReturned

- category: BEHAVIORAL
- emitted when: a link listing was produced (including empty results)
- payload:
  - `item_id`: `string | null` — the item filter applied; null for all-links listing
  - `link_count`: `integer` — total number of link entries returned
  - `links`: `array` — the link entries; each entry contains:
    - `source_id`: `string`
    - `source_type`: `string`
    - `link_type`: `string`
    - `target_id`: `string`
    - `target_type`: `string`
    - `direction`: `string` — `"outgoing"` when source_id is the queried item or
      for all-links listing; `"incoming"` when target_id is the queried item
    - `display_label`: `string` — the forward label for outgoing, inverse label
      for incoming (e.g. "Blocked By" when direction is incoming for a `blocks` link)

### LinkFailedItemNotFound

- category: FAILURE
- emitted when: source_id or target_id is not present in the project record
  (contract failure: ItemNotFound)
- payload:
  - `failure_reason`: `string` — always `"item_not_found"`
  - `operation`: `string` — `"add"` or `"remove"`
  - `missing_item_id`: `string` — the item_id that could not be found

### LinkFailedInvalidLinkType

- category: FAILURE
- emitted when: the link_type is unknown, or is not permitted for the
  source item type and target item type combination
  (contract failure: InvalidLinkType)
- payload:
  - `failure_reason`: `string` — always `"invalid_link_type"`
  - `link_type`: `string` — the value that was supplied
  - `source_type`: `string` — item type of the source item
  - `target_type`: `string` — item type of the target item

### LinkFailedDuplicateLink

- category: FAILURE
- emitted when: an identical (source_id, link_type, target_id) triple already exists
  (contract failure: DuplicateLink)
- payload:
  - `failure_reason`: `string` — always `"duplicate_link"`
  - `source_id`: `string`
  - `link_type`: `string`
  - `target_id`: `string`

### LinkFailedLinkNotFound

- category: FAILURE
- emitted when: the link to be removed does not exist in the record
  (contract failure: LinkNotFound)
- payload:
  - `failure_reason`: `string` — always `"link_not_found"`
  - `source_id`: `string`
  - `link_type`: `string`
  - `target_id`: `string`

## Event Flow

```text
── add ──────────────────────────────────────────────────────────────
LinkAddRequested                    ← PM initiates adding a link
  ↓
  ├─ (source not in record)
  │    LinkFailedItemNotFound  [operation="add", missing_item_id=source_id]
  │
  ├─ (target not in record)
  │    LinkFailedItemNotFound  [operation="add", missing_item_id=target_id]
  │
  ├─ (link_type unknown or not valid for type pair)
  │    LinkFailedInvalidLinkType
  │
  ├─ (identical link already exists)
  │    LinkFailedDuplicateLink
  │
  └─ (both items exist, type valid, not duplicate)
       ItemLinked

── remove ───────────────────────────────────────────────────────────
LinkRemoveRequested                 ← PM initiates removing a link
  ↓
  ├─ (source not in record)
  │    LinkFailedItemNotFound  [operation="remove", missing_item_id=source_id]
  │
  ├─ (target not in record)
  │    LinkFailedItemNotFound  [operation="remove", missing_item_id=target_id]
  │
  ├─ (link does not exist)
  │    LinkFailedLinkNotFound
  │
  └─ (link exists)
       ItemUnlinked

── list ─────────────────────────────────────────────────────────────
LinkListRequested                   ← PM requests a listing
  ↓
  LinkListReturned                  ← always emitted; link_count=0 if none found
```

## Validation Order (add operation)

1. ItemNotFound (source) → LinkFailedItemNotFound
2. ItemNotFound (target) → LinkFailedItemNotFound
3. InvalidLinkType → LinkFailedInvalidLinkType
4. DuplicateLink → LinkFailedDuplicateLink
5. → ItemLinked

## Coverage Check

| Contract Scenario | Event(s) | Status |
|---|---|---|
| HP1: Record a link | LinkAddRequested → ItemLinked | COVERED |
| HP2: Remove a link | LinkRemoveRequested → ItemUnlinked | COVERED |
| HP3: List all links | LinkListRequested → LinkListReturned | COVERED |
| HP4: List links for specific item (outgoing + incoming) | LinkListRequested → LinkListReturned | COVERED |
| HP5: List links for item with no links | LinkListRequested → LinkListReturned (link_count=0) | COVERED |
| Failure: ItemNotFound | LinkFailedItemNotFound | COVERED |
| Failure: InvalidLinkType | LinkFailedInvalidLinkType | COVERED |
| Failure: DuplicateLink | LinkFailedDuplicateLink | COVERED |
| Failure: LinkNotFound | LinkFailedLinkNotFound | COVERED |

| Contract Failure | Event Here | Status |
|---|---|---|
| ItemNotFound    | LinkFailedItemNotFound    | COVERED |
| InvalidLinkType | LinkFailedInvalidLinkType | COVERED |
| DuplicateLink   | LinkFailedDuplicateLink   | COVERED |
| LinkNotFound    | LinkFailedLinkNotFound    | COVERED |

---
status: APPROVED
feature_id: item_links
approved_by: human
approved_at: 2026-05-27
derived_from_intent: intents/item_links.md
derived_from_contract: contracts/item_links_contract.md

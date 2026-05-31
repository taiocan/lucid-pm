# Behavioral Contract: item_links

## Valid Relationship Types and Type Matrix

The following relationship types are valid. Any combination not listed here is rejected.

| Link Type      | Valid Source Types        | Valid Target Types                  |
|---|---|---|
| blocks         | task, issue               | task, milestone                     |
| affects        | risk, issue               | task, milestone, stakeholder        |
| assigned_to    | task, issue               | stakeholder                         |
| mitigated_by   | risk                      | task                                |
| escalates_to   | risk, issue               | stakeholder                         |
| related_to     | any                       | any                                 |

## Inverse Display Labels

| Link Type    | Forward Label (on source) | Inverse Label (on target) |
|---|---|---|
| blocks       | Blocks                    | Blocked By                |
| affects      | Affects                   | Affected By               |
| assigned_to  | Assigned To               | Owns                      |
| mitigated_by | Mitigated By              | Mitigates                 |
| escalates_to | Escalated To              | Escalations               |
| related_to   | Related To                | Related To                |

## Scenarios

### Happy Path 1: Record a link

```gherkin
Given both items identified by source_id and target_id exist in the project record
And the link_type is valid for the source item's type and target item's type
And no identical link (same source_id, link_type, target_id) already exists
When the PM adds a link of link_type from source_id to target_id
Then the link is recorded
And querying the source item shows the link under its forward label
And querying the target item shows the link under its inverse label
And the project record items are not modified
```

### Happy Path 2: Remove a link

```gherkin
Given a link of link_type from source_id to target_id exists in the record
When the PM removes the link
Then the link is no longer present
And querying either item no longer shows the link
And the project record items are not modified
```

### Happy Path 3: List all links (no item filter)

```gherkin
Given at least one link exists in the project record
When the PM lists all links without specifying an item
Then all recorded forward links are returned
And only recorded forward links are shown (no synthetic inverse entries)
And items with no links are not listed
```

### Happy Path 4: List links for a specific item

```gherkin
Given item X has one or more outgoing or incoming links
When the PM lists links for item X
Then all links originating from X are shown with their forward label
And all links targeting X are shown with their inverse label
And links between other items that do not involve X are not shown
```

### Happy Path 5: List links for an item with no links

```gherkin
Given item X exists in the project record
And item X has no outgoing or incoming links
When the PM lists links for item X
Then an empty result is returned
And no failure is signalled
```

### Failure Path 1: ItemNotFound

```gherkin
Given at least one of source_id or target_id does not exist in the project record
When the PM attempts to add or remove a link involving that item_id
Then a failure result is returned identifying which item_id was not found
And no link is recorded or removed
```

### Failure Path 2: InvalidLinkType

```gherkin
Given both items exist in the project record
And the link_type is not in the set of valid link types
  OR the link_type is valid but not permitted for the source item's type and target item's type
When the PM attempts to add a link
Then a failure result is returned identifying the invalid link_type and the
  source and target item types
And no link is recorded
```

### Failure Path 3: DuplicateLink

```gherkin
Given a link of link_type from source_id to target_id already exists
When the PM attempts to add the same link again
Then a failure result is returned indicating the link already exists
And the existing link is not modified
```

### Failure Path 4: LinkNotFound

```gherkin
Given no link of link_type from source_id to target_id exists
When the PM attempts to remove that link
Then a failure result is returned indicating the link does not exist
And no state is changed
```

## Invariants

- A link from item A to item B with type T is distinct from a link from B to A
  with type T — directionality is always preserved
- `related_to` links are recorded directionally (A → B) but displayed
  symmetrically (both A and B show "Related To" label); the stored direction
  is preserved in the event log
- No link operation modifies any item's status, priority, description, or any
  other field
- A link only remains valid while both referenced items exist in the record;
  if an item is removed from the record by other means, links referencing it
  are not automatically removed (they become dangling — resolution is out of scope)
- The set of valid link types and the type matrix are fixed; they are not
  configurable at runtime

## Preconditions

- The project record exists (events/runtime_events.jsonl is accessible)
- For add/remove: both source_id and target_id must identify items in the project record
- For list with item filter: the specified item_id must exist in the record, or an
  empty result is returned — no failure

## Postconditions

- On successful add: exactly one ItemLinked event has been appended;
  the link is visible when querying either item
- On successful remove: exactly one ItemUnlinked event has been appended;
  the link is no longer visible when querying either item
- On any failure: no ItemLinked or ItemUnlinked event has been appended;
  project record is unchanged

## Runtime Artifacts

| Artifact | Path | Lifecycle |
|---|---|---|
| Event log | events/runtime_events.jsonl | Append-only; ItemLinked and ItemUnlinked events added on each successful operation |

No files are created or modified outside the event log.

## Failure Classifications

| Failure Name    | Trigger Condition                                                         | Observable Signal                  |
|---|---|---|
| ItemNotFound    | source_id or target_id not present in the project record                  | LinkFailedItemNotFound emitted     |
| InvalidLinkType | link_type unknown, or not permitted for the source/target item type pair   | LinkFailedInvalidLinkType emitted  |
| DuplicateLink   | identical (source_id, link_type, target_id) triple already recorded       | LinkFailedDuplicateLink emitted    |
| LinkNotFound    | link to remove does not exist in the record                               | LinkFailedLinkNotFound emitted     |

---
status: APPROVED
feature_id: item_links
approved_by: human
approved_at: 2026-05-27
derived_from_intent: intents/item_links.md
derived_event_schema: events/item_links_schema.md

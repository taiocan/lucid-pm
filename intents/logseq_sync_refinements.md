# Refinement Log: logseq_sync

---

## Refinement 2026-05-29 (R3): Canonical page discovery via item-id bullet scan

### Trigger

Trigger type: HUMAN_APPROVED_EVOLUTION

### Observed Problem

`logseq_sync` discovers item pages by UUID filename (`pages/{uuid}.md`). After R3 of
`logseq_export`, pages are named by description slug (`pages/{slug}.md`). The UUID filename
lookup breaks — sync can no longer find any item pages.

### Evidence

Consequence of `logseq_export` R3 (see `intents/logseq_export_refinements.md`). If logseq_export
writes `migration-to-new-auth-service-by-june-30.md`, then
`pages_dir.join(format!("{}.md", item.item_id))` produces a path that does not exist.

### Root Cause

Page discovery was coupled to the UUID filename convention. With slug filenames, the UUID is only
present in the page content as `- item-id: <uuid>`, not in the filename.

### Refinement Type

BEHAVIORAL

### Minimal Change

Add `build_item_page_map()` to `modules/logseq_sync/src/main.rs`: scan all `.md` files in the
pages directory, read the `- item-id: <uuid>` bullet from each, and build a
`HashMap<uuid, PathBuf>`. Replace the UUID filename lookup in `cmd_sync()` with a map lookup.
`parse_page_properties()` is unchanged.

Artifacts changed:
- [ ] `intents/logseq_sync.md` — no change
- [x] `contracts/logseq_sync_contract.md` — two invariants updated, one postcondition clause added
- [ ] `events/logseq_sync_schema.md` — no change
- [x] Implementation in `modules/logseq_sync/src/main.rs`
- [x] `tests/behavioral/logseq_sync_behavior.rs`
- [ ] `tests/replay/` — no change

### Stages Re-run

- [x] Stage 2: Contracts — APPROVED 2026-05-29
- [ ] Stage 3: Event Schema — not re-run (schema unchanged)
- [x] Stage 4: Implementation — APPROVED 2026-05-30
- [x] Stage 5: Tests — APPROVED 2026-05-30
- [x] Stage 6: Runtime Execution — 13 items found via item-id bullet scan, status updates captured
- [x] Stage 7: Reconciliation Review — 17 items all ALIGNED
- [x] Stage 8: Replay Verification — 35 events schema-conformant, 15 chains complete, 13/13 replay tests pass

### Validation

- `cargo test --manifest-path modules/logseq_sync/Cargo.toml` — 37 tests (24 behavioral + 13 replay) — all pass
- Runtime sync of 13 slug-named pages via build_item_page_map() confirmed; status update chains verified
- Refinement marked COMPLETE 2026-05-30

---

<!-- Add new refinement entries above this line, newest first -->

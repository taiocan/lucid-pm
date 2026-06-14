# Intent: R14 — logseq_plugin: Workflow Step Guidance in Success Messages

The `logseq_plugin` Extract and Export operations exist to let the PM
act on their LucidPM project without leaving Logseq Desktop.

Specifically:
- The PM can complete an Extract and know what to do next to make the
  extracted items visible in Logseq.
- The PM can complete an Export and know what to do next before the
  exported pages appear in Logseq.

## Stable Guarantees

- After a successful Extract, the PM is informed of the next step required
  before extracted items become visible in Logseq.
- After a successful Export, the PM is informed of the next step required
  before exported pages become visible in Logseq.
- This refinement extends only the success indication content for Extract
  and Export. Success and failure classification, command delegation, and
  all other command behaviors are unchanged.

## Scope Boundary

This feature does NOT:
- Automate the next step on behalf of the PM
- Add next-step guidance to Sync or Suggest (those commands have no
  required next step visible to the PM)
- Change failure indication content
- Change how the plugin determines the active project or delegates commands

---

<!-- METADATA -->
status: APPROVED
feature_id: R14
approved_by: human
approved_at: 2026-06-13
derived_contracts: contracts/logseq_plugin_contract.md

# R14: `logseq_plugin` Stage 9 — Workflow Step Guidance in Success Messages

**Tier**: Refine  
**Depends on**: F14 (logseq_plugin)  
**Event spine impact**: None  
**Status**: BACKLOG

**Trigger type**: HUMAN_APPROVED_EVOLUTION

---

**The problem**

After running Extract or Export via the Logseq plugin, the PM sees a success notification but no indication of what to do next. The three-step workflow (Extract → Export → Re-index) is implicit and undiscoverable without reading documentation. PMs who are new to the tool, or who return to it after a gap, must either remember the sequence or leave Logseq to look it up.

On WSL2 setups, Logseq Desktop does not automatically detect file changes written from the WSL side. The PM must manually trigger a graph re-index after Export. This step is currently invisible to the plugin UX.

The underlying workflow friction (needing a separate Export + re-index step) is a larger problem deferred to a future "Extract and Surface" command. This refinement addresses discoverability only.

---

**What needs to change**

- **After Extract success**: the SuccessIndication includes a next-step hint pointing to Export. Example: *"Extraction complete. Run LucidPM Export to push items to Logseq."*
- **After Export success**: the SuccessIndication includes a next-step hint about making the exported content visible. Example: *"Export complete. Re-index Logseq graph to see new pages (⋯ → Re-index graph)."*
- The contract postcondition for Extract and Export SuccessIndication is made behavioral (not tied to a Logseq workaround): *"The PM is informed of the next step required before [extracted items / exported content] becomes visible in Logseq."*

**What does NOT change**

- Sync and Suggest success messages (their workflow is terminal — no next step required)
- Failure indication content
- Plugin command delegation logic
- Event spine: no new events

---

**DBA classification**

| Artifact | Change type |
|---|---|
| `contracts/logseq_plugin_contract.md` | Tighten HP1 (Extract) and HP2 (Export) SuccessIndication postconditions to include next-step guidance requirement |
| `plugin/src/index.js` | Update success message strings for Extract and Export handlers |
| Behavioral tests | Update expected success message content for Extract and Export |

Stages re-run: Stage 2 → Stage 4 → Stage 5.

---

**Open design questions for Stage 2**

1. Should the re-index hint name the exact Logseq menu path (⋯ → Re-index graph) or remain generic ("re-index Logseq graph")? Generic is less brittle if Logseq renames the menu item.
2. Should the next-step hint be part of the existing success notification body, or a separate notification? The current plugin shows one notification per command.
3. Future backlog item to capture: "Extract and Surface" — a single command that performs Extract + Export in sequence, then instructs the PM to re-index. This reduces the workflow to two steps (run command, re-index) rather than three.

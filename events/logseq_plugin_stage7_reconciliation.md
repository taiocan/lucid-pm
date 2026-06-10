# Stage 7 Reconciliation — logseq_plugin

Generated: 2026-06-10

| Clause | Description | Failure Class | Status | Layer |
|--------|-------------|---------------|--------|-------|
| FP1 | ActiveProjectNotResolved — lucid not invoked, error shown | ActiveProjectNotResolved | ✓ VERIFIED | L2a:behavioral |
| FP2 | LucidNotAvailable — lucid not found on PATH | LucidNotAvailable | ○ MANUAL-PENDING | L6 |
| FP3 | CommandFailed — lucid exits non-zero; failure indication shown | CommandFailed | ✓ VERIFIED | L2a:behavioral |
| HP1 | Sync invokes lucid sync with project path | — | ✓ VERIFIED | L2a:behavioral |
| HP2 | Export invokes lucid export with project path | — | ✓ VERIFIED | L2a:behavioral |
| HP3 | Suggest invokes lucid suggest with project path | — | ✓ VERIFIED | L2a:behavioral |
| HP4 | Project resolved via graph path when no explicit config | — | ✓ VERIFIED | L2a:behavioral |
| HP5 | Explicit project path overrides graph inference | — | ✓ VERIFIED | L2a:behavioral |
| HP6 | explicit_project_path registered in settings; only settings field | — | ✓ VERIFIED | L2a:behavioral |
| OP1 | EndpointUnavailable — CompanionServerUnavailable shown with port | CompanionServerUnavailable | ✓ VERIFIED | L2b:serialization |
| OP2 | EndpointTimeout — CompanionServerTimeout shown within 60s bound | CompanionServerTimeout | ✓ VERIFIED | L4:e2e_contract |
| OP3 | MalformedResponse — "invalid server response" shown, no crash | MalformedServerResponse | ✓ VERIFIED | L2b:serialization |

**Summary:** 11 VERIFIED · 1 MANUAL-PENDING · 0 FAIL

MANUAL-PENDING clauses require human verification via `plugin/ACCEPTANCE.md`.

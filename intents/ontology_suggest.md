# Intent: ontology_suggest

ontology_suggest exists to let a Project Manager obtain AI-generated proposals
for enriching an existing project record — typed relationships between items,
status assignments, and priority assignments — and to selectively apply only
the proposals they agree with.

Specifically:
- PM can request an AI analysis of the full project record and receive
  structured enrichment proposals with supporting rationale for each
- PM can review each proposal individually and decide to accept or reject it
- PM can correct a proposal by rejecting it and applying the desired change
  manually through the appropriate existing tool
- Accepted proposals become part of the project record and are immediately
  visible to all other features that read from it

## Stable Guarantees

- A proposal is never applied to the project record without explicit PM
  acceptance — the AI cannot modify the record unilaterally
- Only proposals that conform to the established rules of each enrichment type
  are surfaced to the PM (e.g., link type must be valid for the item type pair)
- Rejecting a proposal leaves the project record unchanged
- Accepted proposals produce the same changes as if the PM had made them
  manually — downstream features cannot distinguish AI-confirmed from
  manually-entered changes
- The set of proposals from one analysis session is preserved so the PM can
  confirm selectively at any point after the analysis completes

## Scope Boundary

This feature does NOT:
- Extract new items from text (that is pm_structuring)
- Apply any change to the project record without PM approval
- Infer or enforce workflow rules, deadlines, or dependencies beyond what the
  PM explicitly accepts
- Guarantee proposal quality — the PM is responsible for the final decision
  on each proposal

---

<!-- METADATA -->
status: APPROVED
feature_id: ontology_suggest
approved_by: human
approved_at: 2026-05-28
derived_contracts: contracts/ontology_suggest_contract.md

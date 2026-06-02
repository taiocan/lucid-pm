# R8: `report_export` Stage 9 — Schema Integration

**Tier**: Refine
**Depends on**: F6, F11
**Event spine impact**: None
**Status**: BACKLOG

**Trigger type**: HUMAN_APPROVED_EVOLUTION

---

**The problem**

`report_export` has zero schema awareness. It reads the project record and generates reports using hardcoded entity type names and labels. Three gaps follow from `project_schema`:

1. **Entity type labels in output**: Report sections that group or label items by type (e.g., "Tasks", "Risks", "Stakeholders") use hardcoded strings. The vocabulary may define different display labels.
2. **Unrecognized item exclusion**: Items whose entity type is not in the active vocabulary are currently included in reports. They should produce `SchemaTypeUnknown` events and be excluded.
3. **Alias resolution**: Items stored under an old type name (alias) should appear in reports under their canonical type name and label.

---

**What needs to change**

- Vocabulary is loaded at command startup; schema failure aborts the report before any output
- Entity type display labels in report sections come from the vocabulary renderer configuration
- Items with entity types unrecognized by the vocabulary produce `SchemaTypeUnknown` events and are excluded from report content
- Alias resolution is applied at read time: items stored as an alias type are reported under the canonical type
- Report structure (weekly, risk-register, stakeholders, full) is otherwise unchanged

**What does NOT change**

- Report type set (`weekly`, `risk-register`, `stakeholders`, `full`) — not schema-driven
- Output destination logic (stdout vs. `--graph`)
- Event spine: no new events beyond `SchemaTypeUnknown` (already in `project_schema` schema)
- `EmptyRecord`, `InvalidReportType`, `OutputNotFound` failure paths

---

**DBA classification**

| Artifact | Change type |
|---|---|
| `contracts/report_export_contract.md` | Add schema-authority clauses for type labels and unrecognized item exclusion |
| `modules/report_export/src/main.rs` | Load schema at startup; use vocabulary labels in section headers; exclude unrecognized types; apply alias resolution |
| `tests/behavioral/report_export_behavior.rs` | Update section label assertions; add exclusion and alias resolution scenarios |

Stages re-run: Stage 2 → Stage 4 → Stage 5 → Stage 7 → Stage 8.

---

**Open design questions for Stage 2**

1. If all items of a given report section's type are excluded (unrecognized), is that section omitted from the report entirely, or shown as empty?
2. The `risk-register` and `stakeholders` report types target specific entity types. If those types are renamed in the vocabulary, does the report type name stay the same or also become configurable?
3. Are relation links (from `item_links`) rendered in report output? If so, do their labels also come from the vocabulary?

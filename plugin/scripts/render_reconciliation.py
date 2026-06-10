#!/usr/bin/env python3
"""render_reconciliation.py — Generate the Stage 7 reconciliation markdown
table from the Stage 6 observation JSON artifact.

Usage: python3 render_reconciliation.py <stage6.json> > stage7_reconciliation.md
"""

import json
import pathlib
import sys
from datetime import date

STATUS_ICON = {
    'VERIFIED':       '✓',
    'FAIL':           '✗',
    'MANUAL-PENDING': '○',
}

CONTRACT_DESCRIPTIONS = {
    'HP1': 'Sync invokes lucid sync with project path',
    'HP2': 'Export invokes lucid export with project path',
    'HP3': 'Suggest invokes lucid suggest with project path',
    'HP4': 'Project resolved via graph path when no explicit config',
    'HP5': 'Explicit project path overrides graph inference',
    'HP6': 'explicit_project_path registered in settings; only settings field',
    'FP1': 'ActiveProjectNotResolved — lucid not invoked, error shown',
    'FP2': 'LucidNotAvailable — lucid not found on PATH',
    'FP3': 'CommandFailed — lucid exits non-zero; failure indication shown',
    'OP1': 'EndpointUnavailable — CompanionServerUnavailable shown with port',
    'OP2': 'EndpointTimeout — CompanionServerTimeout shown within 60s bound',
    'OP3': 'MalformedResponse — "invalid server response" shown, no crash',
}


def render(obs: list) -> str:
    lines = [
        f'# Stage 7 Reconciliation — logseq_plugin',
        f'',
        f'Generated: {date.today().isoformat()}',
        f'',
        f'| Clause | Description | Failure Class | Status | Layer |',
        f'|--------|-------------|---------------|--------|-------|',
    ]

    for entry in obs:
        clause  = entry['contract_clause']
        status  = entry['status']
        layer   = entry.get('layer', '—')
        fc      = entry.get('failure_class') or '—'
        desc    = CONTRACT_DESCRIPTIONS.get(clause, '(no description)')
        icon    = STATUS_ICON.get(status, '?')
        lines.append(f'| {clause} | {desc} | {fc} | {icon} {status} | {layer} |')

    verified = sum(1 for e in obs if e['status'] == 'VERIFIED')
    failed   = sum(1 for e in obs if e['status'] == 'FAIL')
    pending  = sum(1 for e in obs if e['status'] == 'MANUAL-PENDING')

    lines += [
        f'',
        f'**Summary:** {verified} VERIFIED · {pending} MANUAL-PENDING · {failed} FAIL',
        f'',
        f'MANUAL-PENDING clauses require human verification via `plugin/ACCEPTANCE.md`.',
    ]
    return '\n'.join(lines) + '\n'


def main():
    if len(sys.argv) < 2:
        print('Usage: render_reconciliation.py <stage6.json>', file=sys.stderr)
        sys.exit(1)

    path = pathlib.Path(sys.argv[1])
    if not path.exists():
        print(f'Not found: {path}', file=sys.stderr)
        sys.exit(1)

    obs = json.loads(path.read_text())
    print(render(obs), end='')


if __name__ == '__main__':
    main()

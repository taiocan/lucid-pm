#!/usr/bin/env python3
"""generate_stage6.py — Parse verification ladder log files and emit the
Stage 6 observation artifact as structured JSON.

Test names containing [HPx], [FPx], [OPx] are mapped to contract clauses.
Layer-level PASS/FAIL is read from summary.txt.
"""

import argparse
import json
import pathlib
import re
import sys

# Mapping from contract clause → known failure class (null for happy paths)
FAILURE_CLASS = {
    'FP1': 'ActiveProjectNotResolved',
    'FP2': 'LucidNotAvailable',
    'FP3': 'CommandFailed',
    'OP1': 'CompanionServerUnavailable',
    'OP2': 'CompanionServerTimeout',
    'OP3': 'MalformedServerResponse',
}

# Contract clauses verified at each layer (for clauses with no individual test name)
LAYER_MAP = {
    'L1:cargo_test':    [],
    'L2a:behavioral':   ['HP1', 'HP2', 'HP3', 'HP4', 'HP5', 'HP6', 'FP1', 'FP3'],
    'L2b:serialization': ['HP1', 'HP2', 'HP3', 'HP5', 'OP1', 'OP3'],
    'L3:pytest_server': ['HP1', 'HP2', 'HP3', 'FP1', 'FP3', 'OP3'],
    'L4:e2e_contract':  ['HP1', 'HP2', 'HP3', 'HP4', 'OP1', 'OP2', 'OP3'],
}

# Clauses that require manual Logseq verification
MANUAL_CLAUSES = {'HP6', 'FP2'}


def parse_layer_results(summary_path: pathlib.Path) -> dict:
    """Returns {layer_name: {'status': 'PASS'|'FAIL', 'log': path, 'duration_s': int}}"""
    results = {}
    if not summary_path.exists():
        return results
    for line in summary_path.read_text().splitlines():
        parts = line.split()
        if len(parts) >= 4:
            name, status, dur, log = parts[0], parts[1], int(parts[2]), parts[3]
            results[name] = {'status': status, 'duration_s': dur, 'log_file': log}
    return results


def parse_test_names_from_log(log_path: pathlib.Path) -> list:
    """Extract clause IDs from test names like [HP1], _HP1_, def test_HP1_."""
    if not log_path.exists():
        return []
    pattern = re.compile(r'\[([A-Z]{2}\d+)\]|_([A-Z]{2}\d+)_|def test_([A-Z]{2}\d+)_')
    clauses = []
    for line in log_path.read_text(errors='replace').splitlines():
        for m in pattern.finditer(line):
            clause = m.group(1) or m.group(2) or m.group(3)
            if clause and clause not in clauses:
                clauses.append(clause)
    return clauses


def build_observations(log_dir: pathlib.Path) -> list:
    summary   = log_dir / 'summary.txt'
    layers    = parse_layer_results(summary)
    seen      = {}  # clause → best entry (prefer VERIFIED over FAIL)

    for layer_name, meta in layers.items():
        log_path    = pathlib.Path(meta['log_file'])
        layer_status = meta['status']

        # Collect clauses from this layer's test output
        found = parse_test_names_from_log(log_path)
        # Fall back to the static map if log parsing found nothing
        if not found:
            found = LAYER_MAP.get(layer_name, [])

        for clause in found:
            status = 'VERIFIED' if layer_status == 'PASS' else 'FAIL'
            entry = {
                'contract_clause': clause,
                'failure_class':   FAILURE_CLASS.get(clause),
                'status':          status,
                'layer':           layer_name,
                'evidence':        meta['log_file'],
            }
            # Keep the first passing verification; don't overwrite with later FAIL
            if clause not in seen or seen[clause]['status'] != 'VERIFIED':
                seen[clause] = entry

    # Add manual-pending clauses that weren't covered automatically
    for clause in MANUAL_CLAUSES:
        if clause not in seen:
            seen[clause] = {
                'contract_clause': clause,
                'failure_class':   FAILURE_CLASS.get(clause),
                'status':          'MANUAL-PENDING',
                'layer':           'L6',
                'evidence':        None,
            }

    return sorted(seen.values(), key=lambda e: e['contract_clause'])


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--log-dir', required=True)
    parser.add_argument('--output',  required=True)
    args = parser.parse_args()

    log_dir = pathlib.Path(args.log_dir)
    obs     = build_observations(log_dir)

    out = pathlib.Path(args.output)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(obs, indent=2) + '\n')
    print(f'Stage 6 artifact written: {out}  ({len(obs)} clauses)', file=sys.stderr)


if __name__ == '__main__':
    main()

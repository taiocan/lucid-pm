"""L3: Server integration tests — verifies the companion server delegates to
lucid correctly, returns well-formed responses, and exposes CORS headers.

Behavioral equivalence: server response must match direct CLI invocation for
deterministic commands. lucid suggest is non-deterministic (new UUID per run)
and is tested for exit-code equivalence only.
"""

import os
import re
import subprocess

import pytest
import requests

from conftest import DEMO_PROJECT, lucid_env  # noqa: F401 (fixtures)


# ── Helpers ──────────────────────────────────────────────────────────────────

def post(base_url: str, *, args: list, project: str, timeout: int = 30) -> dict:
    r = requests.post(f'{base_url}/run',
                      json={'args': args, 'project': project},
                      timeout=timeout)
    r.raise_for_status()
    return r.json()


def normalize(text: str) -> str:
    """Strip variable fields so output can be compared across runs."""
    text = re.sub(r'\d{4}-\d{2}-\d{2}T[\d:.Z]+', '<TIMESTAMP>', text)
    text = re.sub(r'/tmp/[^\s]+', '<TMPPATH>', text)
    text = re.sub(
        r'[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}',
        '<UUID>', text,
    )
    text = re.sub(r'^review_id:.*$', '<REVIEW_ID>', text, flags=re.MULTILINE)
    return ' '.join(text.split())


# ── [HP1] Sync equivalence ───────────────────────────────────────────────────

def test_HP1_sync_equivalent_to_cli(server, lucid_env):
    cli = subprocess.run(
        ['lucid', 'sync', '--graph', 'logseq'],
        capture_output=True, text=True, cwd=DEMO_PROJECT, env=lucid_env,
    )
    resp = post(server, args=['sync', '--graph', 'logseq'], project=str(DEMO_PROJECT))

    assert resp['ok'] == (cli.returncode == 0)
    assert normalize(resp['output']) == normalize(cli.stdout or cli.stderr or '')


# ── [HP2] Export equivalence ─────────────────────────────────────────────────

def test_HP2_export_equivalent_to_cli(server, lucid_env, tmp_path):
    out_dir = str(tmp_path / 'pages')
    cli = subprocess.run(
        ['lucid', 'export', '--output-dir', out_dir],
        capture_output=True, text=True, cwd=DEMO_PROJECT, env=lucid_env,
    )
    resp = post(server, args=['export', '--output-dir', out_dir], project=str(DEMO_PROJECT))

    assert resp['ok'] == (cli.returncode == 0)


# ── [HP3] Suggest exit-code equivalence ─────────────────────────────────────

@pytest.mark.skipif(
    not os.environ.get('GEMINI_API_KEY_PMCLI'),
    reason='lucid suggest requires GEMINI_API_KEY_PMCLI',
)
def test_HP3_suggest_exit_code_matches_cli(server, lucid_env):
    # suggest produces a new UUID per run — only compare exit code, not output
    cli = subprocess.run(
        ['lucid', 'suggest', 'propose'],
        capture_output=True, text=True, cwd=DEMO_PROJECT, env=lucid_env,
    )
    resp = post(server, args=['suggest', 'propose'], project=str(DEMO_PROJECT), timeout=60)

    assert resp['ok'] == (cli.returncode == 0)


# ── [HP1-CORS] OPTIONS preflight ─────────────────────────────────────────────

def test_HP1_cors_options_preflight(server):
    r = requests.options(f'{server}/run', timeout=5)
    assert r.status_code == 204
    assert r.headers.get('Access-Control-Allow-Origin') == '*'
    assert 'POST' in r.headers.get('Access-Control-Allow-Methods', '')


# ── [FP1] Nonexistent project directory ──────────────────────────────────────

def test_FP1_nonexistent_project_returns_error_payload(server):
    resp = post(server, args=['sync', '--graph', 'logseq'],
                project='/nonexistent_lucidpm_project_xyz')
    assert resp['ok'] is False
    assert len(resp['output']) > 0


# ── [FP3] lucid exits non-zero ───────────────────────────────────────────────

def test_FP3_nonzero_exit_propagated(server, lucid_env):
    # 'lucid unknown-command' exits 1
    cli = subprocess.run(
        ['lucid', 'unknown-command'],
        capture_output=True, text=True, cwd=DEMO_PROJECT, env=lucid_env,
    )
    resp = post(server, args=['unknown-command'], project=str(DEMO_PROJECT))

    assert resp['ok'] is False
    assert cli.returncode != 0


# ── [OP3] Payload response structure ─────────────────────────────────────────

def test_OP3_response_always_contains_ok_and_output(server, lucid_env):
    resp = post(server, args=['sync', '--graph', 'logseq'], project=str(DEMO_PROJECT))
    assert 'ok' in resp
    assert isinstance(resp['ok'], bool)
    assert 'output' in resp
    assert isinstance(resp['output'], str)

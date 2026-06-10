"""Shared pytest fixtures for LucidPM plugin server tests."""

import os
import pathlib
import socket
import subprocess
import time

import pytest
import requests

REPO_ROOT    = pathlib.Path(__file__).parent.parent.parent
LUCID_BIN    = REPO_ROOT / 'bin'
DEMO_PROJECT = REPO_ROOT / 'demo'
SERVER_PY    = pathlib.Path(__file__).parent / 'lucid_plugin_server.py'


def find_free_port() -> int:
    with socket.socket() as s:
        s.bind(('127.0.0.1', 0))
        return s.getsockname()[1]


def wait_for_server(port: int, timeout: float = 5.0) -> None:
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            requests.get(f'http://127.0.0.1:{port}/', timeout=0.5)
            return
        except Exception:
            time.sleep(0.1)
    raise TimeoutError(f'Server on port {port} did not start within {timeout}s')


@pytest.fixture(scope='session')
def lucid_env() -> dict:
    env = os.environ.copy()
    env['PATH'] = str(LUCID_BIN) + os.pathsep + env.get('PATH', '')
    return env


@pytest.fixture(scope='module')
def server(lucid_env):
    port = find_free_port()
    proc = subprocess.Popen(
        ['python3', str(SERVER_PY), str(port)],
        env=lucid_env,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    wait_for_server(port)
    yield f'http://127.0.0.1:{port}'
    proc.terminate()
    proc.wait(timeout=5)

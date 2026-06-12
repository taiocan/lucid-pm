#!/usr/bin/env python3
"""LucidPM plugin companion server.

Logseq plugins in sandboxed environments cannot access Node.js directly.
This server runs in WSL (where lucid is installed) and accepts HTTP requests
from the Logseq plugin, delegating them to the lucid CLI.

Usage:
    python3 lucid_plugin_server.py [port]

Default port: 7523
"""
import http.server
import json
import re
import socketserver
import subprocess
import sys

PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 7523
BIND = '127.0.0.1'

# Matches //wsl$/Ubuntu/... or \\wsl$\Ubuntu\... (any distro name)
_WSL_UNC = re.compile(r'^[/\\]{2}wsl\$[/\\][^/\\]+', re.IGNORECASE)


def wsl_to_linux(path: str) -> str:
    """Convert a Windows WSL UNC path to its Linux equivalent.

    Logseq on Windows reports graph paths as //wsl$/Ubuntu/home/...
    The companion server runs on Linux and needs /home/... instead.
    """
    if _WSL_UNC.match(path):
        path = _WSL_UNC.sub('', path).replace('\\', '/')
    return path


class Handler(http.server.BaseHTTPRequestHandler):

    def do_OPTIONS(self):
        self.send_response(204)
        self._cors()
        self.end_headers()

    def do_POST(self):
        length   = int(self.headers.get('Content-Length', 0))
        body     = json.loads(self.rfile.read(length))
        project  = wsl_to_linux(body.get('project') or '') or None
        args     = body.get('args', [])

        cmd = ['lucid'] + args
        try:
            result = subprocess.run(cmd, capture_output=True, text=True, cwd=project)
            data   = {
                'ok':     result.returncode == 0,
                'output': (result.stdout or result.stderr or '').strip(),
            }
        except (FileNotFoundError, OSError) as exc:
            data = {'ok': False, 'output': str(exc)}

        payload = json.dumps(data).encode()

        self.send_response(200)
        self.send_header('Content-Type',   'application/json')
        self.send_header('Content-Length', str(len(payload)))
        self._cors()
        self.end_headers()
        self.wfile.write(payload)

    def _cors(self):
        self.send_header('Access-Control-Allow-Origin',  '*')
        self.send_header('Access-Control-Allow-Methods', 'POST, OPTIONS')
        self.send_header('Access-Control-Allow-Headers', 'Content-Type')

    def log_message(self, *_):
        pass  # suppress per-request noise


class ThreadingServer(socketserver.ThreadingMixIn, http.server.HTTPServer):
    daemon_threads = True


if __name__ == '__main__':
    server = ThreadingServer((BIND, PORT), Handler)
    print(f'LucidPM plugin server listening on {BIND}:{PORT}', flush=True)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print('\nStopped.')

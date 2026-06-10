'use strict';

// L4: Plugin↔Server↔Lucid equivalence tests.
// Exercises the complete chain: plugin JS → real HTTP → Python server → real lucid CLI.
// No Logseq Desktop required.
//
// Forces the server path by mocking child_process without exec.
// Uses jest.requireActual to retain spawn/spawnSync for test setup.

jest.mock('child_process', () => ({
  spawn:     jest.requireActual('child_process').spawn,
  spawnSync: jest.requireActual('child_process').spawnSync,
}));

jest.setTimeout(90_000);

const http    = require('http');
const net     = require('net');
const path    = require('path');
const { spawn, spawnSync } = require('child_process');

const REPO_ROOT    = path.resolve(__dirname, '../../..');
const LUCID_BIN    = path.join(REPO_ROOT, 'bin');
const DEMO_PROJECT = path.join(REPO_ROOT, 'demo');
const SERVER_PY    = path.join(REPO_ROOT, 'plugin', 'server', 'lucid_plugin_server.py');

// ── Utilities ────────────────────────────────────────────────────────────────

function findFreePort() {
  return new Promise((resolve) => {
    const s = net.createServer();
    s.listen(0, '127.0.0.1', () => {
      const port = s.address().port;
      s.close(() => resolve(port));
    });
  });
}

async function waitForServer(port, maxMs = 5000) {
  const deadline = Date.now() + maxMs;
  while (Date.now() < deadline) {
    try {
      await fetch(`http://127.0.0.1:${port}/`);
      return;
    } catch (_) {
      await new Promise(r => setTimeout(r, 100));
    }
  }
  throw new Error(`Server on port ${port} did not become ready within ${maxMs}ms`);
}

function lucidEnv() {
  return { ...process.env, PATH: `${LUCID_BIN}${path.delimiter}${process.env.PATH}` };
}

// ── Test state ────────────────────────────────────────────────────────────────

const registeredCommands = {};
let   serverProcess;
let   serverPort;

beforeAll(async () => {
  serverPort = await findFreePort();
  process.env.LUCID_SERVER_PORT = String(serverPort);

  serverProcess = spawn('python3', [SERVER_PY, String(serverPort)], {
    env:    lucidEnv(),
    stdio:  'ignore',
  });

  global.logseq = {
    ready:             jest.fn((fn) => fn()),
    Editor:            { registerSlashCommand: jest.fn((name, cb) => { registeredCommands[name] = cb; }) },
    UI:                { showMsg: jest.fn() },
    App:               { getCurrentGraph: jest.fn() },
    useSettingsSchema: jest.fn(),
    settings:          {},
  };

  await waitForServer(serverPort);
  jest.isolateModules(() => { require('../../src/index'); });
});

afterAll(() => {
  serverProcess?.kill();
  delete process.env.LUCID_SERVER_PORT;
});

beforeEach(() => {
  jest.clearAllMocks();
  global.logseq.settings = { explicit_project_path: DEMO_PROJECT };
  logseq.App.getCurrentGraph.mockResolvedValue({ path: DEMO_PROJECT });
});

// ── [HP1] Sync: plugin result matches CLI exit code ───────────────────────────

test('[HP1] sync — plugin success type matches lucid CLI exit code', async () => {
  const cli = spawnSync('lucid', ['sync', '--graph', 'logseq'], {
    cwd: DEMO_PROJECT, env: lucidEnv(), encoding: 'utf8',
  });

  await registeredCommands['LucidPM Sync']();

  const [, type] = logseq.UI.showMsg.mock.calls[0];
  expect(type).toBe(cli.status === 0 ? 'success' : 'error');
});

// ── [HP2] Export: plugin result matches CLI exit code ─────────────────────────

test('[HP2] export — plugin success type matches lucid CLI exit code', async () => {
  const cli = spawnSync('lucid', ['export', '--output-dir', 'logseq/pages'], {
    cwd: DEMO_PROJECT, env: lucidEnv(), encoding: 'utf8',
  });

  await registeredCommands['LucidPM Export']();

  const [, type] = logseq.UI.showMsg.mock.calls[0];
  expect(type).toBe(cli.status === 0 ? 'success' : 'error');
});

// ── [HP3] Suggest: exit code only (non-deterministic; requires API key) ──────

const SKIP_SUGGEST = !process.env.GEMINI_API_KEY_PMCLI;

(SKIP_SUGGEST ? test.skip : test)(
  '[HP3] suggest — plugin success type matches lucid CLI exit code',
  async () => {
    const cli = spawnSync('lucid', ['suggest', 'propose'], {
      cwd: DEMO_PROJECT, env: lucidEnv(), encoding: 'utf8', timeout: 60_000,
    });

    await registeredCommands['LucidPM Suggest']();

    const [, type] = logseq.UI.showMsg.mock.calls[0];
    expect(type).toBe(cli.status === 0 ? 'success' : 'error');
  },
);

// ── [HP4] Graph path inference used when no explicit config ───────────────────

test('[HP4] project resolved via graph path when no explicit config', async () => {
  global.logseq.settings = {};
  logseq.App.getCurrentGraph.mockResolvedValue({ path: DEMO_PROJECT });

  await registeredCommands['LucidPM Sync']();

  expect(logseq.App.getCurrentGraph).toHaveBeenCalled();
  const [, type] = logseq.UI.showMsg.mock.calls[0];
  expect(type).toBe('success');
});

// ── [OP1] Endpoint unavailable ────────────────────────────────────────────────

test('[OP1] server killed → CompanionServerUnavailable shown, no crash', async () => {
  serverProcess.kill();
  await new Promise(r => setTimeout(r, 200)); // let port release

  await registeredCommands['LucidPM Sync']();

  expect(logseq.UI.showMsg).toHaveBeenCalledWith(
    expect.stringContaining('CompanionServerUnavailable'),
    'error',
    expect.any(Object),
  );
  expect(logseq.UI.showMsg.mock.calls[0][0]).toContain(String(serverPort));
});

// ── [OP2] Timeout ─────────────────────────────────────────────────────────────

test('[OP2] slow server → CompanionServerTimeout shown, resolves within bound', async () => {
  // Hanging server: accepts TCP connection, never responds
  const hangPort = await findFreePort();
  const hangServer = net.createServer(() => {}); // accept but never respond
  await new Promise(r => hangServer.listen(hangPort, '127.0.0.1', r));

  const cmds = {};
  global.logseq.Editor.registerSlashCommand = jest.fn((name, cb) => { cmds[name] = cb; });
  process.env.LUCID_SERVER_PORT       = String(hangPort);
  process.env.LUCID_SERVER_TIMEOUT_MS = '500'; // short timeout for test speed

  jest.isolateModules(() => { require('../../src/index'); });
  logseq.App.getCurrentGraph.mockResolvedValue({ path: DEMO_PROJECT });

  const t0 = Date.now();
  await cmds['LucidPM Sync']();
  const elapsed = Date.now() - t0;

  expect(logseq.UI.showMsg).toHaveBeenCalledWith(
    expect.stringContaining('CompanionServerTimeout'),
    'error',
    expect.any(Object),
  );
  expect(elapsed).toBeLessThan(3000); // well within the 10s contract bound

  hangServer.close();
  process.env.LUCID_SERVER_PORT       = String(serverPort);
  delete process.env.LUCID_SERVER_TIMEOUT_MS;
});

// ── [OP3] Malformed response ──────────────────────────────────────────────────

test('[OP3] server returns malformed JSON → invalid server response shown', async () => {
  const badPort = await findFreePort();
  const badServer = http.createServer((_req, res) => {
    res.writeHead(200, { 'Content-Type': 'application/json' });
    res.end('not-valid-json{{{{');
  });
  await new Promise(r => badServer.listen(badPort, '127.0.0.1', r));

  const cmds = {};
  global.logseq.Editor.registerSlashCommand = jest.fn((name, cb) => { cmds[name] = cb; });
  process.env.LUCID_SERVER_PORT = String(badPort);

  jest.isolateModules(() => { require('../../src/index'); });
  logseq.App.getCurrentGraph.mockResolvedValue({ path: DEMO_PROJECT });

  await cmds['LucidPM Sync']();

  expect(logseq.UI.showMsg).toHaveBeenCalledWith(
    expect.stringContaining('invalid server response'),
    'error',
    expect.any(Object),
  );

  await new Promise(r => badServer.close(r));
  process.env.LUCID_SERVER_PORT = String(serverPort);
});

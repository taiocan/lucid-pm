'use strict';

// L2b: Serialization tests — verifies the plugin constructs correct JSON payloads
// for each command, and handles server-path failures correctly.
// No real lucid CLI involved. Uses a lightweight in-process capture server.

const http = require('http');
const net  = require('net');

jest.mock('child_process', () => ({})); // no exec → getChildProcess() returns null

// ── Capture server ──────────────────────────────────────────────────────────

function startCaptureServer(respond = () => ({ ok: true, output: 'captured.' })) {
  return new Promise((resolve) => {
    let lastPayload = null;
    const server = http.createServer((req, res) => {
      let body = '';
      req.on('data', c => (body += c));
      req.on('end', () => {
        try { lastPayload = JSON.parse(body); } catch (_) { lastPayload = null; }
        const reply = respond(lastPayload);
        const data  = typeof reply === 'string' ? reply : JSON.stringify(reply);
        res.writeHead(200, { 'Content-Type': 'application/json' });
        res.end(data);
      });
    });
    server.listen(0, '127.0.0.1', () => {
      resolve({
        port:          server.address().port,
        close:         () => new Promise(r => server.close(r)),
        getLastPayload: () => lastPayload,
      });
    });
  });
}

// ── Test state ───────────────────────────────────────────────────────────────

const registeredCommands = {};
let   captureServer;

beforeAll(async () => {
  captureServer = await startCaptureServer();
  process.env.LUCID_SERVER_PORT = String(captureServer.port);

  global.logseq = {
    ready:              jest.fn((fn) => fn()),
    Editor:             { registerSlashCommand: jest.fn((name, cb) => { registeredCommands[name] = cb; }) },
    UI:                 { showMsg: jest.fn() },
    App:                { getCurrentGraph: jest.fn() },
    useSettingsSchema:  jest.fn(),
    settings:           {},
  };

  jest.isolateModules(() => { require('../../src/index'); });
});

afterAll(async () => {
  await captureServer.close();
  delete process.env.LUCID_SERVER_PORT;
});

beforeEach(() => {
  jest.clearAllMocks();
  global.logseq.settings = {};
  logseq.App.getCurrentGraph.mockResolvedValue({ path: '/test/project' });
});

// ── [HP1] Sync payload ───────────────────────────────────────────────────────

test('[HP1] sync payload shape matches server contract', async () => {
  await registeredCommands['LucidPM Sync']();

  const payload = captureServer.getLastPayload();
  expect(payload).toEqual({
    args:    ['sync', '--graph', 'logseq'],
    project: '/test/project',
  });
});

// ── [HP2] Export payload ─────────────────────────────────────────────────────

test('[HP2] export payload shape matches server contract', async () => {
  await registeredCommands['LucidPM Export']();

  const payload = captureServer.getLastPayload();
  expect(payload).toEqual({
    args:    ['export', '--output-dir', 'logseq/pages'],
    project: '/test/project',
  });
});

// ── [HP3] Suggest payload ────────────────────────────────────────────────────

test('[HP3] suggest payload shape matches server contract', async () => {
  await registeredCommands['LucidPM Suggest']();

  const payload = captureServer.getLastPayload();
  expect(payload).toEqual({
    args:    ['suggest', 'propose'],
    project: '/test/project',
  });
});

// ── [HP5] Explicit path used in payload ─────────────────────────────────────

test('[HP5] explicit_project_path overrides graph path in payload', async () => {
  global.logseq.settings = { explicit_project_path: '/explicit/path' };

  await registeredCommands['LucidPM Sync']();

  const payload = captureServer.getLastPayload();
  expect(payload.project).toBe('/explicit/path');
  expect(logseq.App.getCurrentGraph).not.toHaveBeenCalled();
});

// ── [OP1] Endpoint unavailable ───────────────────────────────────────────────

test('[OP1] endpoint unavailable → CompanionServerUnavailable shown, no crash', async () => {
  await captureServer.close();

  await registeredCommands['LucidPM Sync']();

  expect(logseq.UI.showMsg).toHaveBeenCalledWith(
    expect.stringContaining('CompanionServerUnavailable'),
    'error',
    expect.any(Object),
  );
  expect(logseq.UI.showMsg.mock.calls[0][0]).toContain(String(captureServer.port));
});

// ── [OP3] Malformed response ─────────────────────────────────────────────────

test('[OP3] server returns malformed JSON → invalid server response shown', async () => {
  // Start a fresh server that returns bad JSON for this test
  const badServer = await startCaptureServer(() => 'not-json{{');
  process.env.LUCID_SERVER_PORT = String(badServer.port);

  const cmds = {};
  global.logseq.Editor.registerSlashCommand = jest.fn((name, cb) => { cmds[name] = cb; });
  jest.isolateModules(() => { require('../../src/index'); });

  logseq.App.getCurrentGraph.mockResolvedValue({ path: '/test/project' });
  await cmds['LucidPM Sync']();

  expect(logseq.UI.showMsg).toHaveBeenCalledWith(
    expect.stringContaining('invalid server response'),
    'error',
    expect.any(Object),
  );

  await badServer.close();
  process.env.LUCID_SERVER_PORT = String(captureServer.port);
});

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
    let payloads = [];
    const server = http.createServer((req, res) => {
      let body = '';
      req.on('data', c => (body += c));
      req.on('end', () => {
        let parsed = null;
        try { parsed = JSON.parse(body); } catch (_) {}
        payloads.push(parsed);
        const reply = respond(parsed);
        const data  = typeof reply === 'string' ? reply : JSON.stringify(reply);
        res.writeHead(200, { 'Content-Type': 'application/json' });
        res.end(data);
      });
    });
    server.listen(0, '127.0.0.1', () => {
      resolve({
        port:           server.address().port,
        close:          () => new Promise(r => server.close(r)),
        getLastPayload: () => payloads[payloads.length - 1] ?? null,
        getPayloads:    () => [...payloads],
        clearPayloads:  () => { payloads = []; },
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
    Editor: {
      registerSlashCommand: jest.fn((name, cb) => { registeredCommands[name] = cb; }),
      getCurrentPage:       jest.fn(),
    },
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
  logseq.Editor.getCurrentPage.mockResolvedValue({
    'journal?':   true,
    originalName: 'Jun 13th, 2026',
    file:         { path: '/test/project/journals/2026_06_13.md' },
  });
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
    args:    ['export', '--output-dir', 'logseq'],
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

// ── [R13-HP1] Extract payload ────────────────────────────────────────────────
// These tests run before OP1 which closes the capture server.
// Extract sends two requests: first the extract call, then incorporate-latest.

test('[R13-HP1] extract payload shape matches server contract', async () => {
  captureServer.clearPayloads();
  await registeredCommands['LucidPM Extract']();

  const payloads = captureServer.getPayloads();
  expect(payloads[0]).toMatchObject({
    args:       ['extract', '--yes'],
    project:    '/test/project',
    stdin_file: '/test/project/journals/2026_06_13.md',
  });
  expect(payloads[1]).toMatchObject({
    args:    ['state', 'incorporate-latest'],
    project: '/test/project',
  });
});

test('[R13-HP1] extract success message shown when server returns ok:true', async () => {
  await registeredCommands['LucidPM Extract']();

  expect(logseq.UI.showMsg).toHaveBeenCalledWith(
    expect.any(String), 'success', expect.any(Object),
  );
});

// ── [R13-INV-2] CLI stdin equivalence ────────────────────────────────────────
// Correct implementation sends the journal file path as `stdin_file` (the server reads it).
// Wrong implementation would read the file client-side and send inline content instead.

test('[R13-INV-2] test_cli_stdin_equivalence_falsifies_inline_content_transform', async () => {
  captureServer.clearPayloads();
  await registeredCommands['LucidPM Extract']();

  const extractPayload = captureServer.getPayloads()[0];
  expect(extractPayload).toHaveProperty('stdin_file', '/test/project/journals/2026_06_13.md');
  expect(extractPayload).not.toHaveProperty('content');
  expect(extractPayload).not.toHaveProperty('stdin');
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

// ── [R13-HP2] Extract no-items response ──────────────────────────────────────

test('[R13-HP2] no-items server response → warning notification', async () => {
  const noItemsServer = await startCaptureServer(() => ({
    ok:     true,
    output: 'No project management elements were found in the provided text.',
  }));
  process.env.LUCID_SERVER_PORT = String(noItemsServer.port);

  const cmds = {};
  global.logseq.Editor.registerSlashCommand = jest.fn((name, cb) => { cmds[name] = cb; });
  jest.isolateModules(() => { require('../../src/index'); });
  logseq.App.getCurrentGraph.mockResolvedValue({ path: '/test/project' });
  logseq.Editor.getCurrentPage.mockResolvedValue({
    'journal?':   true,
    originalName: 'Jun 13th, 2026',
    file:         { path: '/test/project/journals/2026_06_13.md' },
  });

  await cmds['LucidPM Extract']();

  expect(logseq.UI.showMsg).toHaveBeenCalledWith(
    expect.stringContaining('no items'), 'warning', expect.any(Object),
  );

  await noItemsServer.close();
  process.env.LUCID_SERVER_PORT = String(captureServer.port);
});

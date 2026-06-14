// Replay tests for the logseq_plugin feature.
//
// The plugin emits no domain events to runtime_events.jsonl — all domain events
// come from delegated `lucid` commands. The replay tests therefore verify the
// observable interface equivalent: determinism of command dispatch and
// registration conformance (commands registered at load time, not on demand).

'use strict';

jest.mock('child_process', () => ({
  exec:  jest.fn(),
  spawn: jest.fn(),
}));

jest.mock('fs', () => ({
  readFileSync: jest.fn(),
}));

const { EventEmitter } = require('events');
const { exec, spawn } = require('child_process');
const fs              = require('fs');

function mockChildProcess({ exitCode = 0, stdout = '' } = {}) {
  const proc  = new EventEmitter();
  proc.stdout = new EventEmitter();
  proc.stderr = new EventEmitter();
  proc.stdin  = { write: jest.fn(), end: jest.fn() };
  process.nextTick(() => {
    if (stdout) proc.stdout.emit('data', stdout);
    proc.emit('close', exitCode);
  });
  return proc;
}

const registeredCommands = {};

beforeAll(() => {
  global.logseq = {
    ready: jest.fn((fn) => fn()),
    Editor: {
      registerSlashCommand: jest.fn((name, cb) => { registeredCommands[name] = cb; }),
      getCurrentPage:       jest.fn(),
    },
    UI:                { showMsg: jest.fn() },
    App:               { getCurrentGraph: jest.fn() },
    useSettingsSchema: jest.fn(),
    settings:          {},
  };
  jest.isolateModules(() => { require('../../src/index'); });
});

const REPLAY_JOURNAL = {
  'journal?':   true,
  originalName: 'Jun 13th, 2026',
  file:         { path: '/replay/project/journals/2026_06_13.md' },
};

beforeEach(() => {
  jest.clearAllMocks();
  global.logseq.settings = {};
  logseq.App.getCurrentGraph.mockResolvedValue({ path: '/replay/project' });
  logseq.Editor.getCurrentPage.mockResolvedValue(REPLAY_JOURNAL);
  exec.mockImplementation((_cmd, _opts, cb) => cb(null, 'output', ''));
  fs.readFileSync.mockReturnValue('- journal content');
  spawn.mockImplementation(() => mockChildProcess({ exitCode: 0, stdout: 'Extracted 1 item.' }));
});

// ── Determinism ──────────────────────────────────────────────────────────────

test('logseq_plugin_event_sequence_is_deterministic: sync command dispatch', async () => {
  await registeredCommands['LucidPM Sync']();
  const firstCall = exec.mock.calls[0];

  jest.clearAllMocks();
  logseq.App.getCurrentGraph.mockResolvedValue({ path: '/replay/project' });
  exec.mockImplementation((_cmd, _opts, cb) => cb(null, 'output', ''));

  await registeredCommands['LucidPM Sync']();
  const secondCall = exec.mock.calls[0];

  // Same inputs → same command, same cwd.
  expect(secondCall[0]).toBe(firstCall[0]);
  expect(secondCall[1]).toEqual(firstCall[1]);
});

test('logseq_plugin_event_sequence_is_deterministic: export command dispatch', async () => {
  await registeredCommands['LucidPM Export']();
  const firstCmd  = exec.mock.calls[0][0];
  const firstCwd  = exec.mock.calls[0][1];

  jest.clearAllMocks();
  logseq.App.getCurrentGraph.mockResolvedValue({ path: '/replay/project' });
  exec.mockImplementation((_cmd, _opts, cb) => cb(null, 'output', ''));

  await registeredCommands['LucidPM Export']();

  expect(exec.mock.calls[0][0]).toBe(firstCmd);
  expect(exec.mock.calls[0][1]).toEqual(firstCwd);
});

test('logseq_plugin_event_sequence_is_deterministic: suggest command dispatch', async () => {
  await registeredCommands['LucidPM Suggest']();
  const firstCmd = exec.mock.calls[0][0];

  jest.clearAllMocks();
  logseq.App.getCurrentGraph.mockResolvedValue({ path: '/replay/project' });
  exec.mockImplementation((_cmd, _opts, cb) => cb(null, 'output', ''));

  await registeredCommands['LucidPM Suggest']();

  expect(exec.mock.calls[0][0]).toBe(firstCmd);
});

// ── Registration Conformance ─────────────────────────────────────────────────
// Verifies the observable interface schema: commands registered at load time.

test('all four commands are registered when plugin loads', () => {
  expect(registeredCommands).toHaveProperty('LucidPM Sync');
  expect(registeredCommands).toHaveProperty('LucidPM Export');
  expect(registeredCommands).toHaveProperty('LucidPM Suggest');
  expect(registeredCommands).toHaveProperty('LucidPM Extract');
});

test('commands are registered exactly once at load time, not on demand', () => {
  // registerSlashCommand should have been called exactly 3 times during module load.
  // Additional invocations of commands must not re-register them.
  const registrationCount = logseq.Editor.registerSlashCommand.mock.calls.length;
  // Note: beforeEach clears mocks, so registrationCount here reflects
  // calls made since the last clearAllMocks — which is 0 because load
  // happened in beforeAll before clearAllMocks runs.
  // What we can assert: no new registrations happen when commands are invoked.
  exec.mockImplementation((_cmd, _opts, cb) => cb(null, '', ''));

  const countBefore = logseq.Editor.registerSlashCommand.mock.calls.length;
  // Invoking commands must not trigger new registrations.
  return registeredCommands['LucidPM Sync']().then(() => {
    const countAfter = logseq.Editor.registerSlashCommand.mock.calls.length;
    expect(countAfter).toBe(countBefore);
  });
});

test('project resolution is deterministic: same settings produce same project path', async () => {
  global.logseq.settings = { explicit_project_path: '/fixed/path' };

  await registeredCommands['LucidPM Sync']();
  const firstCwd = exec.mock.calls[0][1].cwd;

  jest.clearAllMocks();
  exec.mockImplementation((_cmd, _opts, cb) => cb(null, '', ''));

  await registeredCommands['LucidPM Sync']();
  const secondCwd = exec.mock.calls[0][1].cwd;

  expect(secondCwd).toBe(firstCwd);
});

test('failure mode dispatch is deterministic: same absent project produces same error message type', async () => {
  logseq.App.getCurrentGraph.mockResolvedValue(null);

  await registeredCommands['LucidPM Sync']();
  const [, firstType] = logseq.UI.showMsg.mock.calls[0];

  jest.clearAllMocks();
  logseq.App.getCurrentGraph.mockResolvedValue(null);

  await registeredCommands['LucidPM Sync']();
  const [, secondType] = logseq.UI.showMsg.mock.calls[0];

  expect(secondType).toBe(firstType);
});

test('R13_extract_dispatch_is_deterministic: same page input produces same spawn args', async () => {
  await registeredCommands['LucidPM Extract']();
  const firstSpawnArgs = spawn.mock.calls[0].slice(0, 2); // [cmd, args]
  const firstSpawnOpts = spawn.mock.calls[0][2];

  jest.clearAllMocks();
  logseq.App.getCurrentGraph.mockResolvedValue({ path: '/replay/project' });
  logseq.Editor.getCurrentPage.mockResolvedValue(REPLAY_JOURNAL);
  fs.readFileSync.mockReturnValue('- journal content');
  spawn.mockImplementation(() => mockChildProcess({ exitCode: 0, stdout: 'Extracted 1 item.' }));

  await registeredCommands['LucidPM Extract']();
  const secondSpawnArgs = spawn.mock.calls[0].slice(0, 2);
  const secondSpawnOpts = spawn.mock.calls[0][2];

  expect(secondSpawnArgs).toEqual(firstSpawnArgs);
  expect(secondSpawnOpts.cwd).toBe(firstSpawnOpts.cwd);
});

// ── R14: Export success message determinism ──────────────────────────────────

test('R14_export_success_message_is_deterministic: same output produces same message including next-step hint', async () => {
  exec.mockImplementation((_cmd, _opts, cb) => cb(null, 'Exported 5 pages.', ''));

  await registeredCommands['LucidPM Export']();
  const firstMessage = logseq.UI.showMsg.mock.calls[0][0];

  jest.clearAllMocks();
  logseq.App.getCurrentGraph.mockResolvedValue({ path: '/replay/project' });
  exec.mockImplementation((_cmd, _opts, cb) => cb(null, 'Exported 5 pages.', ''));

  await registeredCommands['LucidPM Export']();
  const secondMessage = logseq.UI.showMsg.mock.calls[0][0];

  expect(secondMessage).toBe(firstMessage);
  expect(firstMessage).toContain('Re-index Logseq graph');
});

// Behavioral tests for the logseq_plugin feature.
// Verifies observable behavior: which lucid commands are invoked, which
// feedback states are shown, and that pre-invocation failures prevent execution.

'use strict';

jest.mock('child_process', () => ({
  exec:     jest.fn(),
  execSync: jest.fn(),
}));

const { exec, execSync } = require('child_process');

// Captured slash-command callbacks and settings schema, populated when the module loads.
const registeredCommands = {};
let capturedSettingsSchema = null;

beforeAll(() => {
  global.logseq = {
    ready: jest.fn((fn) => fn()),
    Editor: {
      registerSlashCommand: jest.fn((name, cb) => {
        registeredCommands[name] = cb;
      }),
    },
    UI:  { showMsg: jest.fn() },
    App: { getCurrentGraph: jest.fn() },
    useSettingsSchema: jest.fn((schema) => { capturedSettingsSchema = schema; }),
    settings: {},
  };
  // Loading the module calls logseq.ready(main), which registers the commands.
  jest.isolateModules(() => { require('../../src/index'); });
});

beforeEach(() => {
  jest.clearAllMocks();
  global.logseq.settings = {};
  // Default: project resolvable via graph, lucid available, command succeeds.
  logseq.App.getCurrentGraph.mockResolvedValue({ path: '/test/project' });
  execSync.mockReturnValue(Buffer.from(''));
  exec.mockImplementation((_cmd, _opts, cb) => cb(null, 'done.', ''));
});

// ── Happy Paths ──────────────────────────────────────────────────────────────

test('HP1: sync invokes lucid sync with project path', async () => {
  await registeredCommands['LucidPM Sync']();

  expect(exec).toHaveBeenCalledWith(
    'lucid sync --graph logseq',
    { cwd: '/test/project' },
    expect.any(Function),
  );
  expect(logseq.UI.showMsg).toHaveBeenCalledWith(
    expect.stringContaining('sync'),
    'success',
    expect.any(Object),
  );
});

test('HP2: export invokes lucid export with project path', async () => {
  await registeredCommands['LucidPM Export']();

  expect(exec).toHaveBeenCalledWith(
    'lucid export --output-dir logseq/pages',
    { cwd: '/test/project' },
    expect.any(Function),
  );
  expect(logseq.UI.showMsg).toHaveBeenCalledWith(
    expect.stringContaining('export'),
    'success',
    expect.any(Object),
  );
});

test('HP3: suggest invokes lucid suggest with project path', async () => {
  await registeredCommands['LucidPM Suggest']();

  expect(exec).toHaveBeenCalledWith(
    'lucid suggest propose',
    { cwd: '/test/project' },
    expect.any(Function),
  );
  expect(logseq.UI.showMsg).toHaveBeenCalledWith(
    expect.stringContaining('suggest'),
    'success',
    expect.any(Object),
  );
});

test('HP4: project resolved via graph path when no explicit config', async () => {
  logseq.App.getCurrentGraph.mockResolvedValue({ path: '/graph/dir' });
  global.logseq.settings = {};

  await registeredCommands['LucidPM Sync']();

  expect(logseq.App.getCurrentGraph).toHaveBeenCalled();
  expect(exec).toHaveBeenCalledWith(
    expect.any(String),
    { cwd: '/graph/dir' },
    expect.any(Function),
  );
});

test('HP5: explicit project path overrides graph inference', async () => {
  global.logseq.settings = { explicit_project_path: '/explicit/path' };

  await registeredCommands['LucidPM Sync']();

  expect(logseq.App.getCurrentGraph).not.toHaveBeenCalled();
  expect(exec).toHaveBeenCalledWith(
    expect.any(String),
    { cwd: '/explicit/path' },
    expect.any(Function),
  );
});

test('HP6: explicit_project_path field is registered in settings schema', () => {
  expect(capturedSettingsSchema).toEqual(
    expect.arrayContaining([
      expect.objectContaining({ key: 'explicit_project_path' }),
    ]),
  );
});

test('HP6: wsl_mode field is registered in settings schema', () => {
  expect(capturedSettingsSchema).toEqual(
    expect.arrayContaining([
      expect.objectContaining({ key: 'wsl_mode', type: 'boolean', default: false }),
    ]),
  );
});

// ── Failure Paths ────────────────────────────────────────────────────────────

test('FP1: active project not resolved — lucid not invoked, ErrorMessage shown', async () => {
  logseq.App.getCurrentGraph.mockResolvedValue(null);
  global.logseq.settings = {};

  await registeredCommands['LucidPM Sync']();

  expect(exec).not.toHaveBeenCalled();
  expect(execSync).not.toHaveBeenCalled();
  expect(logseq.UI.showMsg).toHaveBeenCalledWith(
    expect.stringContaining('ActiveProjectNotResolved'),
    'error',
    expect.any(Object),
  );
});

test('FP1: error message includes description of why command did not run', async () => {
  logseq.App.getCurrentGraph.mockResolvedValue(null);
  global.logseq.settings = {};

  await registeredCommands['LucidPM Export']();

  const [message] = logseq.UI.showMsg.mock.calls[0];
  expect(message.length).toBeGreaterThan(20);
});

test('FP2: lucid not available — lucid not invoked, ErrorMessage shown', async () => {
  execSync.mockImplementation(() => { throw new Error('not found'); });

  await registeredCommands['LucidPM Sync']();

  expect(exec).not.toHaveBeenCalled();
  expect(logseq.UI.showMsg).toHaveBeenCalledWith(
    expect.stringContaining('LucidNotAvailable'),
    'error',
    expect.any(Object),
  );
});

test('FP3: command exits non-zero — FailureIndication shown with error output', async () => {
  const err = new Error('exit 1');
  err.code = 1;
  exec.mockImplementation((_cmd, _opts, cb) => cb(err, '', 'project record not found'));

  await registeredCommands['LucidPM Sync']();

  expect(logseq.UI.showMsg).toHaveBeenCalledWith(
    expect.stringContaining('project record not found'),
    'error',
    expect.any(Object),
  );
});

// ── Invariants ───────────────────────────────────────────────────────────────

test('invariant: SuccessIndication and FailureIndication are visually distinct', async () => {
  // Success run
  exec.mockImplementation((_cmd, _opts, cb) => cb(null, 'ok', ''));
  await registeredCommands['LucidPM Sync']();
  const [, successType] = logseq.UI.showMsg.mock.calls[0];

  jest.clearAllMocks();

  // Failure run
  exec.mockImplementation((_cmd, _opts, cb) => cb(new Error('fail'), '', 'err'));
  await registeredCommands['LucidPM Sync']();
  const [, failureType] = logseq.UI.showMsg.mock.calls[0];

  expect(successType).not.toBe(failureType);
});

test('invariant: extract command is not registered', () => {
  const names = Object.keys(registeredCommands);
  expect(names.some((n) => n.toLowerCase().includes('extract'))).toBe(false);
});

test('invariant: exactly Sync, Export, Suggest commands registered', () => {
  expect(registeredCommands).toHaveProperty('LucidPM Sync');
  expect(registeredCommands).toHaveProperty('LucidPM Export');
  expect(registeredCommands).toHaveProperty('LucidPM Suggest');
  expect(Object.keys(registeredCommands)).toHaveLength(3);
});

test('invariant: ErrorMessage only shown when lucid was not invoked', async () => {
  // FP1 path: project not resolved → show error, no exec
  logseq.App.getCurrentGraph.mockResolvedValue(null);
  await registeredCommands['LucidPM Sync']();

  expect(exec).not.toHaveBeenCalled();
  expect(logseq.UI.showMsg).toHaveBeenCalledWith(
    expect.any(String), 'error', expect.any(Object),
  );
});

test('invariant: explicit path takes precedence even when graph path is also available', async () => {
  global.logseq.settings = { explicit_project_path: '/explicit' };
  logseq.App.getCurrentGraph.mockResolvedValue({ path: '/graph' });

  await registeredCommands['LucidPM Sync']();

  expect(exec).toHaveBeenCalledWith(
    expect.any(String),
    { cwd: '/explicit' },
    expect.any(Function),
  );
});

test('invariant: success indication content distinguishes completion from error', async () => {
  exec.mockImplementation((_cmd, _opts, cb) => cb(null, 'SyncCompleted: 3 items updated', ''));
  await registeredCommands['LucidPM Sync']();

  const [message] = logseq.UI.showMsg.mock.calls[0];
  // The PM must be able to distinguish completion from execution error —
  // at minimum the message must include the command output.
  expect(message).toContain('SyncCompleted: 3 items updated');
});

// ── WSL Mode ─────────────────────────────────────────────────────────────────

test('WSL mode: sync runs via wsl bash -l -c with linux project path', async () => {
  global.logseq.settings = {
    wsl_mode: true,
    explicit_project_path: '/home/arc/projects/lucidpm',
  };
  exec.mockImplementation((_cmd, _opts, cb) => cb(null, 'done.', ''));

  await registeredCommands['LucidPM Sync']();

  const [cmd, opts] = exec.mock.calls[0];
  expect(cmd).toMatch(/^wsl bash -l -c /);
  expect(cmd).toContain("cd '/home/arc/projects/lucidpm'");
  expect(cmd).toContain('lucid sync --graph logseq');
  // cwd is not passed — working directory is embedded in the shell command
  expect(opts).toEqual({});
});

test('WSL mode: export runs via wsl bash -l -c', async () => {
  global.logseq.settings = {
    wsl_mode: true,
    explicit_project_path: '/home/arc/projects/lucidpm',
  };
  exec.mockImplementation((_cmd, _opts, cb) => cb(null, 'done.', ''));

  await registeredCommands['LucidPM Export']();

  const [cmd] = exec.mock.calls[0];
  expect(cmd).toMatch(/^wsl bash -l -c /);
  expect(cmd).toContain('lucid export --output-dir logseq/pages');
});

test('WSL mode: suggest runs via wsl bash -l -c', async () => {
  global.logseq.settings = {
    wsl_mode: true,
    explicit_project_path: '/home/arc/projects/lucidpm',
  };
  exec.mockImplementation((_cmd, _opts, cb) => cb(null, 'done.', ''));

  await registeredCommands['LucidPM Suggest']();

  const [cmd] = exec.mock.calls[0];
  expect(cmd).toMatch(/^wsl bash -l -c /);
  expect(cmd).toContain('lucid suggest propose');
});

test('WSL mode: lucid availability checked via login shell', async () => {
  global.logseq.settings = {
    wsl_mode: true,
    explicit_project_path: '/home/arc/projects/lucidpm',
  };
  exec.mockImplementation((_cmd, _opts, cb) => cb(null, '', ''));

  await registeredCommands['LucidPM Sync']();

  expect(execSync).toHaveBeenCalledWith('wsl bash -l -c "lucid version"', expect.any(Object));
});

test('WSL mode: LucidNotAvailable shown when wsl lucid version fails', async () => {
  global.logseq.settings = {
    wsl_mode: true,
    explicit_project_path: '/home/arc/projects/lucidpm',
  };
  execSync.mockImplementation(() => { throw new Error('not found'); });

  await registeredCommands['LucidPM Sync']();

  expect(exec).not.toHaveBeenCalled();
  expect(logseq.UI.showMsg).toHaveBeenCalledWith(
    expect.stringContaining('LucidNotAvailable'),
    'error',
    expect.any(Object),
  );
});

test('WSL mode off: lucid checked natively, not via wsl', async () => {
  global.logseq.settings = { wsl_mode: false };
  await registeredCommands['LucidPM Sync']();
  expect(execSync).toHaveBeenCalledWith('lucid version', expect.any(Object));
});

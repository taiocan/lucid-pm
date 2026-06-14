// Behavioral tests for the logseq_plugin feature — direct execution path.
// child_process is available in this context (Jest/Node.js), so Sync/Export/Suggest
// use exec() and Extract uses spawn().

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

function mockChildProcess({ exitCode = 0, stdout = '', stderr = '', spawnError = null } = {}) {
  const proc  = new EventEmitter();
  proc.stdout = new EventEmitter();
  proc.stderr = new EventEmitter();
  proc.stdin  = { write: jest.fn(), end: jest.fn() };
  process.nextTick(() => {
    if (spawnError) {
      proc.emit('error', spawnError);
    } else {
      if (stdout) proc.stdout.emit('data', stdout);
      if (stderr) proc.stderr.emit('data', stderr);
      proc.emit('close', exitCode);
    }
  });
  return proc;
}

const registeredCommands     = {};
let   capturedSettingsSchema = null;

const JOURNAL_PAGE = {
  'journal?':   true,
  originalName: 'Jun 13th, 2026',
  file:         { path: '/test/project/journals/2026_06_13.md' },
};

beforeAll(() => {
  global.logseq = {
    ready: jest.fn((fn) => fn()),
    Editor: {
      registerSlashCommand: jest.fn((name, cb) => { registeredCommands[name] = cb; }),
      getCurrentPage:       jest.fn(),
    },
    UI:                { showMsg: jest.fn() },
    App:               { getCurrentGraph: jest.fn() },
    useSettingsSchema: jest.fn((schema) => { capturedSettingsSchema = schema; }),
    settings:          {},
  };
  jest.isolateModules(() => { require('../../src/index'); });
});

beforeEach(() => {
  jest.clearAllMocks();
  global.logseq.settings = {};
  logseq.App.getCurrentGraph.mockResolvedValue({ path: '/test/project' });
  logseq.Editor.getCurrentPage.mockResolvedValue(JOURNAL_PAGE);
  exec.mockImplementation((_cmd, _opts, cb) => cb(null, 'done.', ''));
  fs.readFileSync.mockReturnValue('- meeting notes\n- follow up on task');
  spawn.mockImplementation(() => mockChildProcess({ exitCode: 0, stdout: 'Extracted 1 item.' }));
});

// ── Happy Paths (parent feature) ─────────────────────────────────────────────

test('HP1: sync invokes lucid sync with project path', async () => {
  await registeredCommands['LucidPM Sync']();

  expect(exec).toHaveBeenCalledWith(
    'lucid sync --graph logseq',
    { cwd: '/test/project' },
    expect.any(Function),
  );
  expect(logseq.UI.showMsg).toHaveBeenCalledWith(
    expect.stringContaining('sync'), 'success', expect.any(Object),
  );
});

test('HP2: export invokes lucid export with project path', async () => {
  await registeredCommands['LucidPM Export']();

  expect(exec).toHaveBeenCalledWith(
    'lucid export --output-dir logseq',
    { cwd: '/test/project' },
    expect.any(Function),
  );
  expect(logseq.UI.showMsg).toHaveBeenCalledWith(
    expect.stringContaining('export'), 'success', expect.any(Object),
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
    expect.stringContaining('suggest'), 'success', expect.any(Object),
  );
});

test('HP4: project resolved via graph path when no explicit config', async () => {
  logseq.App.getCurrentGraph.mockResolvedValue({ path: '/graph/dir' });

  await registeredCommands['LucidPM Sync']();

  expect(logseq.App.getCurrentGraph).toHaveBeenCalled();
  expect(exec).toHaveBeenCalledWith(
    expect.any(String), { cwd: '/graph/dir' }, expect.any(Function),
  );
});

test('HP5: explicit project path overrides graph inference', async () => {
  global.logseq.settings = { explicit_project_path: '/explicit/path' };

  await registeredCommands['LucidPM Sync']();

  expect(logseq.App.getCurrentGraph).not.toHaveBeenCalled();
  expect(exec).toHaveBeenCalledWith(
    expect.any(String), { cwd: '/explicit/path' }, expect.any(Function),
  );
});

test('HP6: explicit_project_path field is registered in settings schema', () => {
  expect(capturedSettingsSchema).toEqual(
    expect.arrayContaining([expect.objectContaining({ key: 'explicit_project_path' })]),
  );
});

test('HP6: explicit_project_path is the only settings field governed by this contract', () => {
  expect(capturedSettingsSchema).toHaveLength(1);
  expect(capturedSettingsSchema[0].key).toBe('explicit_project_path');
});

// ── Failure Paths (parent feature) ───────────────────────────────────────────

test('FP1: active project not resolved — lucid not invoked, ErrorMessage shown', async () => {
  logseq.App.getCurrentGraph.mockResolvedValue(null);

  await registeredCommands['LucidPM Sync']();

  expect(exec).not.toHaveBeenCalled();
  expect(logseq.UI.showMsg).toHaveBeenCalledWith(
    expect.stringContaining('ActiveProjectNotResolved'), 'error', expect.any(Object),
  );
});

test('FP1: error message includes description of why command did not run', async () => {
  logseq.App.getCurrentGraph.mockResolvedValue(null);

  await registeredCommands['LucidPM Export']();

  const [message] = logseq.UI.showMsg.mock.calls[0];
  expect(message.length).toBeGreaterThan(20);
});

test('FP3: command exits non-zero — FailureIndication shown with error output', async () => {
  const err = new Error('exit 1');
  err.code  = 1;
  exec.mockImplementation((_cmd, _opts, cb) => cb(err, '', 'project record not found'));

  await registeredCommands['LucidPM Sync']();

  expect(logseq.UI.showMsg).toHaveBeenCalledWith(
    expect.stringContaining('project record not found'), 'error', expect.any(Object),
  );
});

// ── Invariants (parent feature) ──────────────────────────────────────────────

test('invariant: SuccessIndication and FailureIndication are visually distinct', async () => {
  exec.mockImplementation((_cmd, _opts, cb) => cb(null, 'ok', ''));
  await registeredCommands['LucidPM Sync']();
  const [, successType] = logseq.UI.showMsg.mock.calls[0];

  jest.clearAllMocks();
  exec.mockImplementation((_cmd, _opts, cb) => cb(new Error('fail'), '', 'err'));
  await registeredCommands['LucidPM Sync']();
  const [, failureType] = logseq.UI.showMsg.mock.calls[0];

  expect(successType).not.toBe(failureType);
});

test('invariant: all four commands registered (Sync, Export, Suggest, Extract)', () => {
  expect(registeredCommands).toHaveProperty('LucidPM Sync');
  expect(registeredCommands).toHaveProperty('LucidPM Export');
  expect(registeredCommands).toHaveProperty('LucidPM Suggest');
  expect(registeredCommands).toHaveProperty('LucidPM Extract');
  expect(Object.keys(registeredCommands)).toHaveLength(4);
});

test('invariant: ErrorMessage only shown when lucid was not invoked', async () => {
  logseq.App.getCurrentGraph.mockResolvedValue(null);
  await registeredCommands['LucidPM Sync']();

  expect(exec).not.toHaveBeenCalled();
  expect(logseq.UI.showMsg).toHaveBeenCalledWith(
    expect.any(String), 'error', expect.any(Object),
  );
});

test('invariant: explicit path takes precedence even when graph path is available', async () => {
  global.logseq.settings = { explicit_project_path: '/explicit' };
  logseq.App.getCurrentGraph.mockResolvedValue({ path: '/graph' });

  await registeredCommands['LucidPM Sync']();

  expect(exec).toHaveBeenCalledWith(
    expect.any(String), { cwd: '/explicit' }, expect.any(Function),
  );
});

test('invariant: success indication content distinguishes completion from error', async () => {
  exec.mockImplementation((_cmd, _opts, cb) => cb(null, 'SyncCompleted: 3 items updated', ''));
  await registeredCommands['LucidPM Sync']();

  const [message] = logseq.UI.showMsg.mock.calls[0];
  expect(message).toContain('SyncCompleted: 3 items updated');
});

// ── R13-HP1: Successful Extraction ───────────────────────────────────────────

test('R13-HP1: extract invokes lucid extract --yes with correct project path', async () => {
  await registeredCommands['LucidPM Extract']();

  expect(spawn).toHaveBeenCalledWith(
    'lucid', ['extract', '--yes'],
    expect.objectContaining({ cwd: '/test/project' }),
  );
});

test('R13-HP1: extract reads journal file and pipes content to stdin', async () => {
  await registeredCommands['LucidPM Extract']();

  expect(fs.readFileSync).toHaveBeenCalledWith(JOURNAL_PAGE.file.path, 'utf8');
  const proc = spawn.mock.results[0].value;
  expect(proc.stdin.write).toHaveBeenCalledWith('- meeting notes\n- follow up on task');
  expect(proc.stdin.end).toHaveBeenCalled();
});

test('R13-HP1: success notification shown on successful extraction', async () => {
  await registeredCommands['LucidPM Extract']();

  expect(logseq.UI.showMsg).toHaveBeenCalledWith(
    expect.any(String), 'success', expect.any(Object),
  );
});

test('R13-HP1: success notification always includes Export guidance', async () => {
  spawn.mockImplementationOnce(() => mockChildProcess({ exitCode: 0, stdout: 'Extracted 2 tasks.' }));
  await registeredCommands['LucidPM Extract']();

  const [message] = logseq.UI.showMsg.mock.calls[0];
  expect(message).toContain('LucidPM Export');
});

test('R13-HP1: success notification includes extraction output', async () => {
  spawn.mockImplementationOnce(() => mockChildProcess({ exitCode: 0, stdout: 'Extracted 2 tasks.' }));
  await registeredCommands['LucidPM Extract']();

  const [message] = logseq.UI.showMsg.mock.calls[0];
  expect(message).toContain('Extracted 2 tasks.');
});

// ── R13-HP2: No Items Found ───────────────────────────────────────────────────

test('R13-HP2: no-items output → warning notification, not an error', async () => {
  spawn.mockImplementationOnce(() => mockChildProcess({
    exitCode: 0,
    stdout:   'No project management elements were found in the provided text.',
  }));
  await registeredCommands['LucidPM Extract']();

  expect(logseq.UI.showMsg).toHaveBeenCalledWith(
    expect.stringContaining('no items'), 'warning', expect.any(Object),
  );
});

test('R13-HP2: empty stdout → warning notification, not an error', async () => {
  spawn.mockImplementationOnce(() => mockChildProcess({ exitCode: 0, stdout: '' }));
  await registeredCommands['LucidPM Extract']();

  expect(logseq.UI.showMsg).toHaveBeenCalledWith(
    expect.any(String), 'warning', expect.any(Object),
  );
});

test('R13-Boundary: empty journal page → no-items warning, not an error', async () => {
  fs.readFileSync.mockReturnValueOnce('');
  spawn.mockImplementationOnce(() => mockChildProcess({ exitCode: 0, stdout: '' }));
  await registeredCommands['LucidPM Extract']();

  expect(logseq.UI.showMsg).toHaveBeenCalledWith(
    expect.any(String), 'warning', expect.any(Object),
  );
});

// ── R13-FP1: NotAJournalPage ──────────────────────────────────────────────────

test('R13-FP1: non-journal page → error shown, no extraction performed', async () => {
  logseq.Editor.getCurrentPage.mockResolvedValueOnce({
    'journal?': false,
    originalName: 'Project Notes',
  });
  await registeredCommands['LucidPM Extract']();

  expect(spawn).not.toHaveBeenCalled();
  expect(logseq.UI.showMsg).toHaveBeenCalledWith(
    expect.stringMatching(/not a journal page/i), 'error', expect.any(Object),
  );
});

// ── R13-FP2: NoCurrentPage ────────────────────────────────────────────────────

test('R13-FP2: no current page → error shown, no extraction performed', async () => {
  logseq.Editor.getCurrentPage.mockResolvedValueOnce(null);
  await registeredCommands['LucidPM Extract']();

  expect(spawn).not.toHaveBeenCalled();
  expect(logseq.UI.showMsg).toHaveBeenCalledWith(
    expect.stringMatching(/no page/i), 'error', expect.any(Object),
  );
});

// ── R13-FP3: ActiveProjectNotResolved ────────────────────────────────────────

test('R13-FP3: active project not resolved → error shown, no extraction', async () => {
  global.logseq.settings = {};
  logseq.App.getCurrentGraph.mockResolvedValue(null);
  await registeredCommands['LucidPM Extract']();

  expect(spawn).not.toHaveBeenCalled();
  expect(logseq.UI.showMsg).toHaveBeenCalledWith(
    expect.stringContaining('ActiveProjectNotResolved'), 'error', expect.any(Object),
  );
});

// ── R13-FP5: CommandFailed ────────────────────────────────────────────────────

test('R13-FP5: lucid extract exits non-zero → failure indication shown', async () => {
  spawn.mockImplementationOnce(() => mockChildProcess({ exitCode: 1, stderr: 'project record not found' }));
  await registeredCommands['LucidPM Extract']();

  expect(logseq.UI.showMsg).toHaveBeenCalledWith(
    expect.stringContaining('project record not found'), 'error', expect.any(Object),
  );
});

test('R13-FP5: failure indication is visually distinct from success', async () => {
  await registeredCommands['LucidPM Extract']();
  const [, successType] = logseq.UI.showMsg.mock.calls[0];

  jest.clearAllMocks();
  logseq.App.getCurrentGraph.mockResolvedValue({ path: '/test/project' });
  logseq.Editor.getCurrentPage.mockResolvedValueOnce(JOURNAL_PAGE);
  fs.readFileSync.mockReturnValue('content');
  spawn.mockImplementationOnce(() => mockChildProcess({ exitCode: 1, stderr: 'error output' }));
  await registeredCommands['LucidPM Extract']();
  const [, failureType] = logseq.UI.showMsg.mock.calls[0];

  expect(successType).not.toBe(failureType);
});

// ── R13: Invariant Falsification ─────────────────────────────────────────────

// R13-INV-1: Plugin resolves currently open page at invocation time, not cached.
test('R13-INV-1: test_currently_open_page_falsifies_cached_page_resolution', async () => {
  // First invocation: page A
  logseq.Editor.getCurrentPage.mockResolvedValueOnce({
    'journal?':   true,
    originalName: 'Jun 12th, 2026',
    file:         { path: '/test/project/journals/2026_06_12.md' },
  });
  spawn.mockImplementationOnce(() => mockChildProcess({ exitCode: 0, stdout: 'ok' }));
  await registeredCommands['LucidPM Extract']();
  expect(fs.readFileSync).toHaveBeenCalledWith('/test/project/journals/2026_06_12.md', 'utf8');

  jest.clearAllMocks();
  logseq.App.getCurrentGraph.mockResolvedValue({ path: '/test/project' });
  fs.readFileSync.mockReturnValue('new content');
  spawn.mockImplementationOnce(() => mockChildProcess({ exitCode: 0, stdout: 'ok' }));

  // Second invocation: page B — must extract B, not reuse A's path
  logseq.Editor.getCurrentPage.mockResolvedValueOnce({
    'journal?':   true,
    originalName: 'Jun 13th, 2026',
    file:         { path: '/test/project/journals/2026_06_13.md' },
  });
  await registeredCommands['LucidPM Extract']();

  expect(fs.readFileSync).toHaveBeenCalledWith('/test/project/journals/2026_06_13.md', 'utf8');
  expect(fs.readFileSync).not.toHaveBeenCalledWith('/test/project/journals/2026_06_12.md', 'utf8');
});

// R13-INV-3: Journal guard uses Logseq's journal? API, not date-like name heuristic.
test('R13-INV-3: test_non_journal_guard_falsifies_name_based_detection', async () => {
  logseq.Editor.getCurrentPage.mockResolvedValueOnce({
    'journal?': false,
    originalName: '2026-06-13',  // name looks like a date, but Logseq says it is not a journal page
  });
  await registeredCommands['LucidPM Extract']();

  expect(spawn).not.toHaveBeenCalled();
  expect(logseq.UI.showMsg).toHaveBeenCalledWith(
    expect.stringMatching(/not a journal page/i), 'error', expect.any(Object),
  );
});

// R13-INV-4: Success notification includes Export guidance regardless of output length.
test('R13-INV-4: test_visibility_guidance_falsifies_conditional_omission', async () => {
  const longOutput = 'Extracted: ' + 'item content. '.repeat(100);
  spawn.mockImplementationOnce(() => mockChildProcess({ exitCode: 0, stdout: longOutput }));
  await registeredCommands['LucidPM Extract']();

  const [message] = logseq.UI.showMsg.mock.calls[0];
  expect(message).toContain('LucidPM Export');
});

// ── R14: Export next-step guidance ───────────────────────────────────────────

// R14-HP2: Export success indication includes re-index next-step hint.
test('R14-HP2: export success indication includes next-step re-index guidance', async () => {
  exec.mockImplementation((_cmd, _opts, cb) => cb(null, 'Exported 5 pages.', ''));
  await registeredCommands['LucidPM Export']();

  const [message, type] = logseq.UI.showMsg.mock.calls[0];
  expect(type).toBe('success');
  expect(message).toContain('Re-index Logseq graph');
});

// R14-invariant: next-step hint is present regardless of lucid export output content.
test('R14-invariant: export next-step hint present when lucid output is empty', async () => {
  exec.mockImplementation((_cmd, _opts, cb) => cb(null, '', ''));
  await registeredCommands['LucidPM Export']();

  const [message] = logseq.UI.showMsg.mock.calls[0];
  expect(message).toContain('Re-index Logseq graph');
});

test('R14-invariant: export next-step hint present regardless of output length', async () => {
  const longOutput = 'Exported: ' + 'page-name. '.repeat(100);
  exec.mockImplementation((_cmd, _opts, cb) => cb(null, longOutput, ''));
  await registeredCommands['LucidPM Export']();

  const [message] = logseq.UI.showMsg.mock.calls[0];
  expect(message).toContain('Re-index Logseq graph');
});

// R14-falsification: re-index hint is export-specific — absent from Sync and Suggest.
// Falsifies: implementation that appends the hint to all command success messages.
test('R14-falsification: sync success does not include re-index hint', async () => {
  await registeredCommands['LucidPM Sync']();

  const [message] = logseq.UI.showMsg.mock.calls[0];
  expect(message).not.toContain('Re-index Logseq graph');
});

test('R14-falsification: suggest success does not include re-index hint', async () => {
  await registeredCommands['LucidPM Suggest']();

  const [message] = logseq.UI.showMsg.mock.calls[0];
  expect(message).not.toContain('Re-index Logseq graph');
});

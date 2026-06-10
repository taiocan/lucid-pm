// LucidPM Logseq Plugin
// Registers Sync, Export, and Suggest commands. Delegates each to the `lucid`
// CLI via either direct child_process (unsandboxed Logseq) or the companion
// server (sandboxed Logseq / WSL setups).

/* global logseq, fetch */

const SERVER_PORT       = parseInt(process.env.LUCID_SERVER_PORT       || '7523',  10);
const SERVER_TIMEOUT_MS = parseInt(process.env.LUCID_SERVER_TIMEOUT_MS || '60000', 10);

// First element of each array is the lucid subcommand; remaining are its flags.
const COMMAND_ARGS = {
  sync:    ['sync',    '--graph',      'logseq'],
  export:  ['export',  '--output-dir', 'logseq/pages'],
  suggest: ['suggest', 'propose'],
};

async function main() {
  logseq.useSettingsSchema([
    {
      key:         'explicit_project_path',
      type:        'string',
      default:     '',
      title:       'LucidPM Project Path',
      description: 'Absolute path to the LucidPM project directory. Required when the ' +
                   'Logseq graph is not the project directory (e.g. WSL setups — use the ' +
                   'Linux path: /home/user/projects/myproject). Leave blank to infer from ' +
                   'the current Logseq graph.',
    },
  ]);

  logseq.Editor.registerSlashCommand('LucidPM Sync',    () => invokeCommand('sync'));
  logseq.Editor.registerSlashCommand('LucidPM Export',  () => invokeCommand('export'));
  logseq.Editor.registerSlashCommand('LucidPM Suggest', () => invokeCommand('suggest'));
}

async function resolveProject() {
  const explicit = (logseq.settings?.explicit_project_path || '').trim();
  if (explicit !== '') return explicit;
  const graph = await logseq.App.getCurrentGraph();
  return graph?.path || null;
}

function getChildProcess() {
  try {
    const cp = typeof require !== 'undefined'
      ? require('child_process')
      : (typeof window !== 'undefined' && typeof window.require !== 'undefined')
        ? window.require('child_process')
        : null;
    return (cp && typeof cp.exec === 'function') ? cp : null;
  } catch (_) {
    return null;
  }
}

async function runDirect(subcommand, projectPath) {
  const cp  = getChildProcess();
  const cmd = ['lucid', ...COMMAND_ARGS[subcommand]].join(' ');
  return new Promise((resolve) => {
    cp.exec(cmd, { cwd: projectPath }, (error, stdout, stderr) => {
      if (error) {
        resolve({ ok: false, output: (stderr || stdout || error.message).trim() });
      } else {
        resolve({ ok: true, output: stdout.trim() || `${subcommand} completed.` });
      }
    });
  });
}

async function runViaServer(subcommand, projectPath) {
  const controller = new AbortController();
  const timer      = setTimeout(() => controller.abort(), SERVER_TIMEOUT_MS);
  let response;
  try {
    response = await fetch(`http://localhost:${SERVER_PORT}/run`, {
      method:  'POST',
      headers: { 'Content-Type': 'application/json' },
      body:    JSON.stringify({ project: projectPath, args: COMMAND_ARGS[subcommand] }),
      signal:  controller.signal,
    });
    clearTimeout(timer);
  } catch (err) {
    clearTimeout(timer);
    if (err.name === 'AbortError') {
      throw new Error(
        `CompanionServerTimeout: server at localhost:${SERVER_PORT} did not respond within ` +
        `${SERVER_TIMEOUT_MS / 1000}s (CompanionServerTimeout). ` +
        `Check that it is running: python3 plugin/server/lucid_plugin_server.py`
      );
    }
    throw new Error(
      `CompanionServerUnavailable: could not reach localhost:${SERVER_PORT}. ` +
      `Start it in WSL: python3 plugin/server/lucid_plugin_server.py`
    );
  }

  let payload;
  try { payload = await response.json(); } catch (_) { payload = null; }
  if (!payload || typeof payload.ok !== 'boolean') {
    return { ok: false, output: `invalid server response from localhost:${SERVER_PORT}` };
  }
  return payload;
}

async function invokeCommand(subcommand) {
  const projectPath = await resolveProject();

  if (!projectPath) {
    logseq.UI.showMsg(
      'LucidPM — ActiveProjectNotResolved: Could not determine the active project. ' +
      'Set an explicit project path in plugin settings.',
      'error',
      { timeout: 8000 },
    );
    return;
  }

  let result;
  try {
    const cp = getChildProcess();
    result = cp
      ? await runDirect(subcommand, projectPath)
      : await runViaServer(subcommand, projectPath);
  } catch (err) {
    logseq.UI.showMsg(
      `LucidPM — ${err.message}`,
      'error',
      { timeout: 10000 },
    );
    return;
  }

  if (result.ok) {
    logseq.UI.showMsg(
      `LucidPM — ${subcommand} completed:\n${result.output || `${subcommand} completed.`}`,
      'success',
      { timeout: 6000 },
    );
  } else {
    logseq.UI.showMsg(
      `LucidPM — ${subcommand} failed:\n${result.output}`,
      'error',
      { timeout: 10000 },
    );
  }
}

logseq.ready(main).catch(console.error);

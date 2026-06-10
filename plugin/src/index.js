// LucidPM Logseq Plugin
// Registers Sync, Export, and Suggest commands. All domain operations are
// delegated entirely to the `lucid` CLI; this plugin is a trigger layer only.

/* global logseq */

// Logseq plugins run in a sandboxed iframe. Node.js modules are accessed via
// window.require (Electron's renderer-side require), not the global require.

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
      description: 'Explicit path to the LucidPM project directory. When set, takes ' +
                   'precedence over graph path inference. Leave blank to infer from the ' +
                   'current Logseq graph. In WSL mode, use the Linux path ' +
                   '(e.g. /home/user/projects/myproject).',
    },
    {
      key:         'wsl_mode',
      type:        'boolean',
      default:     false,
      title:       'WSL Mode',
      description: 'Run lucid via WSL. Enable this when Logseq runs on Windows but ' +
                   'lucid is installed in WSL. Requires LucidPM Project Path to be ' +
                   'set to the Linux path of the project.',
    },
  ]);

  logseq.Editor.registerSlashCommand('LucidPM Sync',    () => invokeCommand('sync'));
  logseq.Editor.registerSlashCommand('LucidPM Export',  () => invokeCommand('export'));
  logseq.Editor.registerSlashCommand('LucidPM Suggest', () => invokeCommand('suggest'));
}

async function resolveProject() {
  const explicit = (logseq.settings?.explicit_project_path || '').trim();
  if (explicit !== '') {
    return explicit;
  }
  const graph = await logseq.App.getCurrentGraph();
  return graph?.path || null;
}

function getChildProcess() {
  if (typeof require !== 'undefined') return require('child_process');
  if (typeof window !== 'undefined' && typeof window.require !== 'undefined') {
    return window.require('child_process');
  }
  return null;
}

function isLucidAvailable() {
  const wslMode = !!logseq.settings?.wsl_mode;
  const cp = getChildProcess();
  if (!cp) return { ok: false, detail: 'child_process not available in this Logseq version' };
  try {
    const check = wslMode ? 'wsl bash -l -c "lucid version"' : 'lucid version';
    cp.execSync(check, { stdio: 'pipe' });
    return { ok: true };
  } catch (err) {
    const detail = (err.stderr ? String(err.stderr).trim() : '') || err.message || String(err);
    return { ok: false, detail };
  }
}

async function invokeCommand(subcommand) {
  const projectPath = await resolveProject();
  const wslMode     = !!logseq.settings?.wsl_mode;

  if (!projectPath) {
    logseq.UI.showMsg(
      'LucidPM — ActiveProjectNotResolved: Could not determine the active project. ' +
      'Set an explicit project path in plugin settings.',
      'error',
      { timeout: 8000 },
    );
    return;
  }

  const lucidCheck = isLucidAvailable();
  if (!lucidCheck.ok) {
    const hint = wslMode
      ? 'Install lucid in your WSL environment before using this plugin.'
      : 'Install lucid before using this plugin.';
    logseq.UI.showMsg(
      `LucidPM — LucidNotAvailable: \`lucid\` was not found. ${hint}\n(${lucidCheck.detail})`,
      'error',
      { timeout: 12000 },
    );
    return;
  }

  const cp = getChildProcess();
  if (!cp) {
    logseq.UI.showMsg(
      'LucidPM — child_process not available in this Logseq version. ' +
      'The plugin requires Node.js access (Logseq Desktop with nodeIntegration).',
      'error',
      { timeout: 10000 },
    );
    return;
  }
  const { exec } = cp;
  let cmd, execOptions;

  if (wslMode) {
    const fullCmd  = ['lucid', ...COMMAND_ARGS[subcommand]].join(' ');
    // Escape single quotes in the path so the shell -c string stays valid.
    const safePath = projectPath.replace(/'/g, "'\\''");
    // Login shell (-l) ensures ~/.profile is sourced so lucid is on PATH.
    cmd         = `wsl bash -l -c "cd '${safePath}' && ${fullCmd}"`;
    execOptions = {};
  } else {
    cmd         = ['lucid', ...COMMAND_ARGS[subcommand]].join(' ');
    execOptions = { cwd: projectPath };
  }

  exec(cmd, execOptions, (error, stdout, stderr) => {
    if (error) {
      const output = (stderr || stdout || error.message).trim();
      logseq.UI.showMsg(
        `LucidPM — ${subcommand} failed:\n${output}`,
        'error',
        { timeout: 10000 },
      );
    } else {
      const output = stdout.trim() || `${subcommand} completed.`;
      logseq.UI.showMsg(
        `LucidPM — ${subcommand} completed:\n${output}`,
        'success',
        { timeout: 6000 },
      );
    }
  });
}

logseq.ready(main).catch(console.error);

// LucidPM Logseq Plugin
// Registers Sync, Export, and Suggest commands. All domain operations are
// delegated entirely to the `lucid` CLI; this plugin is a trigger layer only.

/* global logseq */

// child_process is required lazily inside functions to avoid blocking plugin
// load and triggering Logseq's slow-startup timeout.

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

function isLucidAvailable() {
  const wslMode = !!logseq.settings?.wsl_mode;
  try {
    const { execSync } = require('child_process');
    const check = wslMode ? 'wsl bash -l -c "lucid version"' : 'lucid version';
    execSync(check, { stdio: 'pipe' });
    return { ok: true };
  } catch (err) {
    const stderr = err.stderr instanceof Buffer ? err.stderr.toString('utf8').trim() : '';
    const detail = stderr || err.message || String(err);
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

  const { exec } = require('child_process');
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

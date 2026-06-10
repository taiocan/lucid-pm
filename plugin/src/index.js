// LucidPM Logseq Plugin
// Registers Sync, Export, and Suggest commands. All domain operations are
// delegated entirely to the `lucid` CLI; this plugin is a trigger layer only.

/* global logseq */

const { exec, execSync } = require('child_process');

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
                   'current Logseq graph.',
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
  try {
    execSync('lucid version', { stdio: 'ignore' });
    return true;
  } catch (_) {
    return false;
  }
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

  if (!isLucidAvailable()) {
    logseq.UI.showMsg(
      'LucidPM — LucidNotAvailable: `lucid` was not found on the system PATH. ' +
      'Install lucid before using this plugin.',
      'error',
      { timeout: 8000 },
    );
    return;
  }

  const cmd = ['lucid', ...COMMAND_ARGS[subcommand]].join(' ');

  exec(cmd, { cwd: projectPath }, (error, stdout, stderr) => {
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

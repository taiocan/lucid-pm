// LucidPM Logseq Plugin
// Registers Sync, Export, Suggest, and Extract commands. Delegates each to the
// `lucid` CLI via either direct child_process (unsandboxed Logseq) or the
// companion server (sandboxed Logseq / WSL setups).

/* global logseq, fetch */

// process is not available in sandboxed Electron renderers (Logseq plugin context)
const _env = typeof process !== 'undefined' ? process.env : {};
const SERVER_PORT       = parseInt(_env.LUCID_SERVER_PORT       || '7523',  10);
const SERVER_TIMEOUT_MS = parseInt(_env.LUCID_SERVER_TIMEOUT_MS || '60000', 10);

// First element of each array is the lucid subcommand; remaining are its flags.
const COMMAND_ARGS = {
  sync:    ['sync',    '--graph',      'logseq'],
  export:  ['export',  '--output-dir', 'logseq'],
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
  logseq.Editor.registerSlashCommand('LucidPM Extract', async (e) => {
    // Resolve the page from the slash command block context first.
    // getCurrentPage() can return null when the editing context is cleared by the
    // slash command menu; the block UUID from the callback is more reliable.
    let resolvedPage = null;
    if (e?.uuid) {
      try {
        const block = await logseq.Editor.getBlock(e.uuid);
        if (block?.page?.id) {
          resolvedPage = await logseq.Editor.getPage(block.page.id);
        }
      } catch (_) {}
    }
    await extractCurrentPage(resolvedPage);
  });
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

// options.args     — override args (default: COMMAND_ARGS[subcommand])
// options.stdinFile — path to a file whose content is piped to the subprocess stdin
async function runViaServer(subcommand, projectPath, options = {}) {
  const controller = new AbortController();
  const timer      = setTimeout(() => controller.abort(), SERVER_TIMEOUT_MS);
  const args       = options.args !== undefined ? options.args : COMMAND_ARGS[subcommand];
  const body       = { project: projectPath, args };
  if (options.stdinFile) body.stdin_file = options.stdinFile;

  let response;
  try {
    response = await fetch(`http://localhost:${SERVER_PORT}/run`, {
      method:  'POST',
      headers: { 'Content-Type': 'application/json' },
      body:    JSON.stringify(body),
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

// Direct-mode extraction: spawns lucid extract --yes with file content on stdin.
// Used only when child_process is available (unsandboxed Logseq / Node.js context).
async function runDirectExtract(journalFilePath, projectPath, cp) {
  let fs;
  try {
    fs = typeof require !== 'undefined' ? require('fs') : window.require('fs');
  } catch (_) {
    return { ok: false, output: 'Could not load filesystem module for direct extraction.' };
  }

  let fileContent;
  try {
    fileContent = fs.readFileSync(journalFilePath, 'utf8');
  } catch (err) {
    return { ok: false, output: `Could not read journal file: ${err.message}` };
  }

  return new Promise((resolve) => {
    const child = cp.spawn('lucid', ['extract', '--yes'], {
      cwd:   projectPath,
      stdio: ['pipe', 'pipe', 'pipe'],
    });
    let stdout = '', stderr = '';
    child.stdout.on('data', (d) => { stdout += d; });
    child.stderr.on('data', (d) => { stderr += d; });
    child.stdin.write(fileContent);
    child.stdin.end();
    child.on('close', (code) => {
      resolve({ ok: code === 0, output: (stdout || stderr || '').trim() });
    });
    child.on('error', (err) => {
      resolve({ ok: false, output: err.message });
    });
  });
}

// Resolve the filesystem path to a journal page's vault file.
// Prefers the path Logseq exposes directly; falls back to constructing it from
// journalDay (YYYYMMDD) and the vault root using Logseq's default naming convention.
function resolveJournalFilePath(page, graphPath) {
  if (page.file?.path) return page.file.path;
  if (page.journalDay && graphPath) {
    const d        = String(page.journalDay);
    const filename = `${d.slice(0, 4)}_${d.slice(4, 6)}_${d.slice(6, 8)}.md`;
    return `${graphPath}/journals/${filename}`;
  }
  return null;
}

async function extractCurrentPage(resolvedPage) {
  // NoCurrentPage guard — use pre-resolved page from block context if available,
  // fall back to getCurrentPage() for non-slash-command invocations.
  const page = resolvedPage || await logseq.Editor.getCurrentPage();
  if (!page) {
    logseq.UI.showMsg(
      'LucidPM — no page is currently open.',
      'error',
      { timeout: 6000 },
    );
    return;
  }

  // NotAJournalPage guard
  if (!page['journal?']) {
    logseq.UI.showMsg(
      `LucidPM — "${page.originalName}" is not a journal page. ` +
      'LucidPM Extract only works on daily journal pages.',
      'error',
      { timeout: 6000 },
    );
    return;
  }

  // ActiveProjectNotResolved guard
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

  // Resolve journal file path
  const graph          = await logseq.App.getCurrentGraph();
  const journalFilePath = resolveJournalFilePath(page, graph?.path);
  if (!journalFilePath) {
    logseq.UI.showMsg(
      'LucidPM — could not determine the file path for this journal page.',
      'error',
      { timeout: 6000 },
    );
    return;
  }

  // Delegate to lucid extract
  let result;
  try {
    const cp = getChildProcess();
    result = cp
      ? await runDirectExtract(journalFilePath, projectPath, cp)
      : await runViaServer(null, projectPath, {
          args:      ['extract', '--yes'],
          stdinFile: journalFilePath,
        });
  } catch (err) {
    logseq.UI.showMsg(
      `LucidPM — ${err.message}`,
      'error',
      { timeout: 10000 },
    );
    return;
  }

  if (result.ok) {
    const output = result.output || '';
    if (!output || output.includes('No project management elements')) {
      logseq.UI.showMsg(
        'LucidPM — no items were extracted from this journal page.',
        'warning',
        { timeout: 6000 },
      );
    } else {
      // Items were extracted — incorporate the session into the project record so
      // that a subsequent Export picks them up.
      try {
        const cp = getChildProcess();
        if (cp) {
          await new Promise((resolve) => {
            const child = cp.spawn('lucid', ['state', 'incorporate-latest'], {
              cwd: projectPath, stdio: 'ignore',
            });
            child.on('close', resolve);
            child.on('error', resolve);
          });
        } else {
          await runViaServer(null, projectPath, { args: ['state', 'incorporate-latest'] });
        }
      } catch (_) { /* non-fatal: items are extracted even if incorporate fails */ }

      logseq.UI.showMsg(
        `LucidPM — extract completed.\n` +
        `Run "LucidPM Export" to make extracted items visible in Logseq.\n\n${output}`,
        'success',
        { timeout: 10000 },
      );
    }
  } else {
    logseq.UI.showMsg(
      `LucidPM — extract failed:\n${result.output}`,
      'error',
      { timeout: 10000 },
    );
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
    const body = result.output || `${subcommand} completed.`;
    const nextStep = subcommand === 'export'
      ? '\nRe-index Logseq graph to see new pages (⋯ → Re-index graph).'
      : '';
    logseq.UI.showMsg(
      `LucidPM — ${subcommand} completed:\n${body}${nextStep}`,
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

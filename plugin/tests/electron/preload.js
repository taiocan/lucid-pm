// L5.5 Electron Harness — preload script
//
// SCOPE NOTE: This harness proves the plugin executes correctly in a generic sandboxed
// Electron renderer with a minimal logseq mock. It does NOT reproduce Logseq's actual
// renderer environment (CSP rules, preload sequencing, plugin iframe architecture,
// bundler behavior). A green L5.5 run means the plugin does not crash in Electron's
// sandbox and registers its commands. It does not mean the plugin works correctly
// inside Logseq Desktop.
//
// Mock surface for Logseq SDK ~0.0.15:
//   logseq.ready, logseq.useSettingsSchema, logseq.Editor.registerSlashCommand,
//   logseq.App.getCurrentGraph, logseq.UI.showMsg, logseq.settings
// If the real SDK adds/changes signatures, update this comment AND the mock together.

'use strict';
const { contextBridge, ipcRenderer } = require('electron');

const scenario      = (process.argv.find(a => a.startsWith('--scenario=')) || '--scenario=happy').split('=')[1];
const READY_TIMEOUT = 5000;

let   readyCallback = null;
let   errorCaught   = false;
const registered    = [];
const globalErrors  = [];

// Always expose __harness so renderer.html can report errors via window.onerror
contextBridge.exposeInMainWorld('__harness', {
  reportError: (msg) => { globalErrors.push(msg); },
});

function buildMock(name) {
  if (name === 'no-logseq-global') return null;

  const mock = {
    ready:             (cb)    => { readyCallback = cb; return Promise.resolve(); },
    useSettingsSchema: ()      => {},
    Editor: {
      registerSlashCommand: (cmdName) => { registered.push(cmdName); },
    },
    App:      { getCurrentGraph: async () => ({ path: '/test/project' }) },
    UI:       { showMsg: () => {} },
    settings: { explicit_project_path: '' },
  };

  if (name === 'ready-throws') {
    // Causes main() to reject immediately (first call inside main())
    mock.useSettingsSchema = () => { throw new Error('simulated: useSettingsSchema threw'); };
  } else if (name === 'editor-missing') {
    // logseq.Editor is absent — plugin's registerSlashCommand call throws TypeError
    delete mock.Editor;
  }

  return mock;
}

const mock = buildMock(scenario);
if (mock !== null) {
  contextBridge.exposeInMainWorld('logseq', mock);
}
// no-logseq-global: logseq not injected → plugin's `logseq.ready(main)` throws
// ReferenceError at module scope → fires window.onerror → captured by __harness

window.addEventListener('DOMContentLoaded', async () => {
  if (scenario === 'no-logseq-global') {
    // Plugin script has already thrown; wait briefly for onerror to propagate
    await new Promise(r => setTimeout(r, 300));
    ipcRenderer.send(`result_${scenario}`, { registered, errors: globalErrors, errorCaught: false });
    return;
  }

  // Poll for logseq.ready(cb) to be called by the plugin (synchronous at module load)
  const deadline = Date.now() + READY_TIMEOUT;
  while (readyCallback === null && Date.now() < deadline) {
    await new Promise(r => setTimeout(r, 10));
  }
  if (readyCallback === null) {
    ipcRenderer.send(`result_${scenario}`, {
      registered,
      errors: [...globalErrors, 'logseq.ready() was never called'],
      errorCaught: false,
    });
    return;
  }

  try {
    await readyCallback();
  } catch (_err) {
    errorCaught = true;
    // Expected in ready-throws and editor-missing scenarios.
    // Plugin's logseq.ready(main).catch(console.error) handles this on the plugin side;
    // errorCaught here confirms the scenario's injected failure was triggered.
  }

  // Brief pause to allow any async unhandledrejection to propagate to __harness
  await new Promise(r => setTimeout(r, 100));
  ipcRenderer.send(`result_${scenario}`, { registered, errors: globalErrors, errorCaught });
});

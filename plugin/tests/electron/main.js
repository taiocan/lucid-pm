// L5.5 Electron Harness — generic sandbox runtime validation
//
// SCOPE: Proves the plugin executes correctly in a generic sandboxed Electron renderer
// with a minimal logseq mock. Does NOT reproduce Logseq's actual renderer environment
// (CSP rules, preload sequencing, plugin iframe architecture, bundler behavior).
// A green L5.5 run means the plugin does not crash in Electron's sandbox and registers
// its commands. It does NOT mean the plugin works correctly inside Logseq Desktop.
//
// Usage: electron tests/electron/main.js --no-sandbox
//   or:  npm run electron:test

'use strict';
const { app, BrowserWindow, ipcMain } = require('electron');
const path = require('path');

const PRELOAD  = path.join(__dirname, 'preload.js');
const RENDERER = `file://${path.join(__dirname, 'renderer.html')}`;
const EXPECTED_COMMANDS = ['LucidPM Sync', 'LucidPM Export', 'LucidPM Suggest'];

const SCENARIOS = [
  {
    name: 'happy',
    desc: 'Plugin loads in sandboxed renderer; all 3 commands register; no errors',
    check: (d) => EXPECTED_COMMANDS.every(c => d.registered.includes(c)) && d.errors.length === 0,
  },
  {
    name: 'ready-throws',
    desc: 'main() throws → caught by .catch(); no unhandled rejection in window',
    check: (d) => d.errorCaught === true && d.errors.length === 0,
  },
  {
    name: 'editor-missing',
    desc: 'logseq.Editor absent → TypeError caught gracefully; no crash',
    check: (d) => d.errorCaught === true && d.errors.length === 0,
  },
  {
    name: 'no-logseq-global',
    desc: 'logseq not injected → ReferenceError captured by window.onerror; no crash',
    check: (d) => d.errors.length > 0,
  },
];

async function runScenario(scenario) {
  return new Promise((resolve) => {
    // sandbox: false — Electron's OS-level process sandbox requires the zygote helper
    // which is unavailable in WSL2. With nodeIntegration: false + contextIsolation: true,
    // the renderer still has no require/module access; only contextBridge APIs are exposed.
    // Note: with sandbox: false, process.env IS available in the renderer (Electron exposes
    // a limited process object). The process.env guard in src/index.js is still correct and
    // needed for Logseq's actual plugin context (which may use sandbox: true).
    const win = new BrowserWindow({
      show: false,
      webPreferences: {
        preload:             PRELOAD,
        contextIsolation:    true,
        sandbox:             false,
        nodeIntegration:     false,
        additionalArguments: [`--scenario=${scenario.name}`],
      },
    });

    const tid = setTimeout(() => {
      win.destroy();
      resolve({ name: scenario.name, pass: false, reason: 'timeout after 7s' });
    }, 7000);

    ipcMain.once(`result_${scenario.name}`, (_e, data) => {
      clearTimeout(tid);
      const pass = scenario.check(data);
      win.destroy();
      resolve({
        name:   scenario.name,
        pass,
        reason: pass ? 'ok' : JSON.stringify(data).slice(0, 300),
      });
    });

    win.loadURL(RENDERER);
  });
}

// Prevent Electron's default quit-on-last-window-close from firing between scenarios
app.on('window-all-closed', () => {});

app.whenReady().then(async () => {
  const results = [];
  for (const s of SCENARIOS) {
    results.push(await runScenario(s));
  }

  const passed = results.filter(r => r.pass).length;
  const failed = results.filter(r => !r.pass).length;

  let summary = '\n=== L5.5 Electron Harness ===\n\n';
  for (const r of results) {
    summary += `  ${r.pass ? 'PASS' : 'FAIL'}  ${r.name}  —  ${r.reason}\n`;
  }
  summary += `\n${passed} passed, ${failed} failed\n`;

  // Write summary then exit; callback ensures flush before exit
  process.stdout.write(summary, () => app.exit(failed > 0 ? 1 : 0));
});

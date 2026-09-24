// Diagnóstico curto do startup da galeria no bundle de produção. Lê os erros
// do próprio navegador antes de gastar minutos nos 26 testes Puppeteer.
// Uso: node scripts/sondar-limitless-ui.mjs http://127.0.0.1:8081/
import { spawn } from 'node:child_process';
import { existsSync, mkdirSync } from 'node:fs';
import { join, resolve } from 'node:path';

const url = process.argv[2];
if (!url) throw new Error('informe a URL da galeria');
const browser = process.env.PUPPETEER_EXECUTABLE_PATH || process.env.CHROME_EXECUTABLE ||
  ['C:/Program Files/Google/Chrome/Application/chrome.exe',
   'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe'].find(existsSync);
if (!browser) throw new Error('Chrome ou Edge não encontrado');

const profile = resolve('target', `tmp-lui-cdp-${process.pid}`);
mkdirSync(profile, { recursive: true });
const port = 9336;
const child = spawn(browser, [
  '--headless=new', '--no-sandbox', '--disable-gpu',
  `--user-data-dir=${profile}`, `--remote-debugging-port=${port}`,
  'about:blank',
], { stdio: 'ignore' });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function tab() {
  for (let i = 0; i < 100; i++) {
    try {
      const tabs = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
      const page = tabs.find((t) => t.type === 'page');
      if (page) return page;
    } catch { /* Chrome ainda inicia. */ }
    await sleep(100);
  }
  throw new Error('CDP não abriu em 10 s');
}

async function sondar() {
  const target = await tab();
  const ws = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((ok, fail) => { ws.addEventListener('open', ok, { once: true }); ws.addEventListener('error', fail, { once: true }); });
  const pending = new Map();
  const diagnostics = [];
  const pausas = [];
  let seq = 0;
  ws.addEventListener('message', ({ data }) => {
    const m = JSON.parse(data);
    if (m.id) {
      const p = pending.get(m.id);
      pending.delete(m.id);
      if (p) m.error ? p.fail(new Error(JSON.stringify(m.error))) : p.ok(m.result);
    } else if (m.method === 'Runtime.exceptionThrown') {
      diagnostics.push(m.params.exceptionDetails.exception?.description || m.params.exceptionDetails.text);
    } else if (m.method === 'Runtime.consoleAPICalled' && m.params.type === 'error') {
      diagnostics.push(`console.error: ${m.params.args.map((a) => a.description || a.value).join(' ')}`);
    } else if (m.method === 'Log.entryAdded' && m.params.entry.level === 'error') {
      diagnostics.push(`log: ${m.params.entry.text}`);
    } else if (m.method === 'Network.responseReceived' && m.params.response.status >= 400) {
      diagnostics.push(`HTTP ${m.params.response.status}: ${m.params.response.url}`);
    } else if (m.method === 'Debugger.paused') {
      if (pausas.length < 10) {
        pausas.push({
          reason: m.params.reason,
          error: m.params.data?.description || m.params.data?.value || m.params.data,
          at: m.params.callFrames?.[0]?.functionName,
        });
      }
      void send('Debugger.resume');
    }
  });
  function send(method, params = {}) {
    const id = ++seq;
    ws.send(JSON.stringify({ id, method, params }));
    return new Promise((ok, fail) => pending.set(id, { ok, fail }));
  }
  try {
    await Promise.all(['Runtime.enable', 'Log.enable', 'Network.enable', 'Page.enable', 'Debugger.enable'].map((m) => send(m)));
    await send('Debugger.setPauseOnExceptions', { state: 'all' });
    await send('Page.navigate', { url });
    await sleep(8000);
    const result = await send('Runtime.evaluate', {
      expression: `({url:location.href, pronto:document.readyState, montados:document.querySelectorAll('.demo-page, .content').length, erros:window.__erros || [], titulo:document.title, corpo:(document.body?.innerText || '').slice(0, 500)})`,
      returnByValue: true,
    });
    const state = result.result.value;
    const line = /main\.dart\.js:(\d+)/.exec(diagnostics[0] || state.erros[0] || '');
    let source = [];
    if (line) {
      const lines = (await (await fetch(new URL('/main.dart.js', url))).text()).split('\n');
      const n = Number(line[1]);
      source = lines.slice(Math.max(0, n - 6), n + 5).map((t, i) => `${Math.max(0, n - 6) + i + 1}: ${t}`);
    }
    console.log(JSON.stringify({ state, diagnostics: diagnostics.slice(0, 30), pausas, source }, null, 2));
    if (!state.montados || state.erros.length || diagnostics.length) process.exitCode = 1;
  } finally {
    ws.close();
  }
}

try { await sondar(); } finally { child.kill(); }

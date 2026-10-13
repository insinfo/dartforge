#!/usr/bin/env node
// Compares DartForge's development JavaScript compiler (`dartforge serve`,
// crates/dev + crates/emit_js) with upstream DDC as the Dart toolchain drives it
// in development (`webdev serve`: build_runner daemon + build_web_compilers'
// persistent `dartdevc`/`kernel_worker` workers with `--use-incremental-compiler`).
//
// Commands (results are appended as JSON lines to <out>/results.jsonl; the
// `report` command turns them into the tables of docs/DEV-VS-DDC.md):
//
//   cold   cold and warm start of the server until it is ready, and until the
//          first page rendered in headless Chrome; plus `build_runner build`
//   edits  edit-to-reload latency (body, public API, template edits) with both
//          servers running, alternating tools; memory after every edit;
//          JavaScript served; page load and an interaction tour in Chrome
//   node   bench/performance programs compiled by `dartdevc` (ES modules) and by
//          `dartforge compile-js`, run under Node; results checked against the VM
//   report prints Markdown tables from results.jsonl
//
// Both tools work on separate COPIES of the same project (never the original):
// an edit is written to the copy each tool watches. The copies must already
// have `.dart_tool/package_config.json`, and `webdev`, `build_runner` and
// `build_web_compilers` must resolve in the DDC copy.
//
// Usage:
//   node scripts/compare-dev-ddc.mjs cold  --preset lui --ours <copy> --ddc <copy> [--reps 3]
//   node scripts/compare-dev-ddc.mjs edits --preset lui --ours <copy> --ddc <copy> [--reps 7]
//   node scripts/compare-dev-ddc.mjs node  [--reps 3]
//   node scripts/compare-dev-ddc.mjs report
// Common options: --out <dir> (default target/dev-vs-ddc), --dartforge <bin>,
//   --dart-sdk <dir>, --chrome <exe>, --pub-cache <dir>, --tag <label>.
// Environment fallbacks: DARTFORGE_BIN, DART_SDK, CHROME, PUB_CACHE.
//
// macOS is assumed for memory (`footprint`, physical footprint and its peak;
// RSS is unreliable there under memory pressure). Elsewhere memory falls back
// to RSS from `ps`.

import { spawn, spawnSync, execFileSync } from 'node:child_process';
import { appendFileSync, existsSync, mkdirSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync, mkdtempSync, cpSync } from 'node:fs';
import { dirname, join, resolve, relative } from 'node:path';
import { fileURLToPath } from 'node:url';
import { gzipSync } from 'node:zlib';
import os from 'node:os';

const repo = resolve(dirname(fileURLToPath(import.meta.url)), '..');

// ------------------------------------------------------------------ options

function parseArgs(argv) {
  const o = { _: [] };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a.startsWith('--')) {
      const k = a.slice(2);
      const v = argv[i + 1] && !argv[i + 1].startsWith('--') ? argv[++i] : 'true';
      o[k] = v;
    } else o._.push(a);
  }
  return o;
}

const opts = parseArgs(process.argv.slice(2));
const cmd = opts._[0];
const OUT = resolve(opts.out || join(repo, 'target/dev-vs-ddc'));
const DARTFORGE = resolve(opts.dartforge || process.env.DARTFORGE_BIN || join(repo, 'target/release/dartforge'));
const DART_SDK = resolve(opts['dart-sdk'] || process.env.DART_SDK || '/usr/local/dart-sdk');
const DART = join(DART_SDK, 'bin/dart');
const CHROME = opts.chrome || process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const PUB_CACHE = opts['pub-cache'] || process.env.PUB_CACHE;
const TAG = opts.tag || '';
const REPS = Number(opts.reps || 3);
mkdirSync(OUT, { recursive: true });

// Project presets: what to start, what "ready" means in the page, and where
// each edit kind goes. Paths are relative to the project directory (the one
// with pubspec.yaml); `apiFile` may leave it (a path dependency).
const PRESETS = {
  // limitless_ui/example: the component gallery (186 files in lib/, plus the
  // limitless_ui package through `path: ..`).
  lui: {
    entry: 'web/main.dart',
    web: 'web',
    ready: "!!document.querySelector('my-app .navbar')",
    component: 'lib/src/app/app_component.dart',
    anchor: 'AppComponent(this.i18n, this.theme, this._ngZone) {',
    template: 'lib/src/app/app_component.html',
    // Exported by package:limitless_ui/limitless_ui.dart, which every page imports.
    apiFile: '../lib/src/components/currency_input/currency_input_formatter.dart',
    // Hash routes visited by the interaction tour (laps of the list: the last
    // route must differ from the first).
    routes: ['/overview', '/alerts', '/progress', '/accordion', '/tabs', '/modal', '/offcanvas', '/breadcrumbs',
      '/pagination', '/selection-controls', '/select', '/rating', '/slider', '/timeline', '/typeahead', '/wizard',
      '/multi-select', '/currency', '/color-picker', '/date-picker', '/time-picker', '/date-range', '/carousel'],
  },
  // new_sali/frontend: a real ngdart application (284 files, a barrel library
  // imported by 203 of them). Without its backend only the public login page renders.
  sali: {
    entry: 'web/main.dart',
    web: 'web',
    ready: "!!document.querySelector('pre-login-page .page-content')",
    component: 'lib/src/modules/auth/pages/pre_login/pre_login_page.dart',
    anchor: 'PreLoginPage(this.authService, this._router) {',
    template: 'lib/src/modules/auth/pages/pre_login/pre_login_page.html',
    // Exported by package:new_sali_frontend/new_sali_frontend.dart.
    apiFile: 'lib/src/shared/services/cache/mem_cache.dart',
    routes: [],
  },
};

function preset() {
  const p = PRESETS[opts.preset];
  if (!p) throw new Error(`--preset must be one of ${Object.keys(PRESETS).join(', ')}`);
  return { name: opts.preset, ...p };
}

// ------------------------------------------------------------------ helpers

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const now = () => performance.now();

function record(kind, data) {
  const row = { kind, tag: TAG, time: new Date().toISOString(), load: os.loadavg().map((x) => +x.toFixed(1)), ...data };
  appendFileSync(join(OUT, 'results.jsonl'), JSON.stringify(row) + '\n');
  console.log(JSON.stringify(row));
  return row;
}

function median(xs) {
  const s = [...xs].sort((a, b) => a - b);
  if (!s.length) return NaN;
  const m = s.length >> 1;
  return s.length % 2 ? s[m] : (s[m - 1] + s[m]) / 2;
}

/// All processes: pid, ppid, cumulative CPU seconds, RSS bytes, command.
function processTable() {
  const out = execFileSync('ps', ['-axo', 'pid=,ppid=,time=,rss=,command='], { encoding: 'utf8', maxBuffer: 64 << 20 });
  const rows = [];
  for (const line of out.split('\n')) {
    const m = line.match(/^\s*(\d+)\s+(\d+)\s+(\S+)\s+(\d+)\s+(.*)$/);
    if (m) rows.push({ pid: +m[1], ppid: +m[2], cpu: cpuSeconds(m[3]), rss: +m[4] * 1024, command: m[5] });
  }
  return rows;
}

/// `ps` TIME: [[dd-]hh:]mm:ss.ss
function cpuSeconds(t) {
  let days = 0;
  if (t.includes('-')) { const [d, r] = t.split('-'); days = +d; t = r; }
  const parts = t.split(':').map(Number);
  let s = 0;
  for (const p of parts) s = s * 60 + p;
  return s + days * 86400;
}

function descendants(table, roots) {
  const set = new Set(roots);
  let grew = true;
  while (grew) {
    grew = false;
    for (const r of table) if (!set.has(r.pid) && set.has(r.ppid)) { set.add(r.pid); grew = true; }
  }
  return [...set];
}

/// Physical footprint (current and lifetime peak) per pid, from `footprint`.
function footprints(pids) {
  const res = new Map();
  if (!pids.length) return res;
  if (process.platform === 'darwin') {
    const args = ['--noCategories', '-f', 'bytes'];
    for (const p of pids) args.push('-p', String(p));
    const r = spawnSync('footprint', args, { encoding: 'utf8', maxBuffer: 16 << 20 });
    let cur = null;
    for (const line of (r.stdout || '').split('\n')) {
      let m = line.match(/\[(\d+)\]: .*Footprint: (\d+) B/);
      if (m) { cur = +m[1]; res.set(cur, { footprint: +m[2], peak: +m[2] }); continue; }
      m = line.match(/phys_footprint_peak: (\d+) B/);
      if (m && cur != null) res.get(cur).peak = +m[1];
    }
  }
  const table = processTable();
  for (const p of pids) {
    const row = table.find((r) => r.pid === p);
    if (!row) continue;
    const e = res.get(p) || { footprint: row.rss, peak: row.rss };
    e.rss = row.rss;
    e.cpu = row.cpu;
    e.command = row.command.slice(0, 120);
    res.set(p, e);
  }
  return res;
}

function sumMemory(pids) {
  const f = footprints(pids);
  let footprint = 0, peak = 0, rss = 0, cpu = 0;
  const per = [];
  for (const [pid, e] of f) {
    footprint += e.footprint; peak += e.peak; rss += e.rss || 0; cpu += e.cpu || 0;
    per.push({ pid, mb: +(e.footprint / 1048576).toFixed(1), peakMb: +(e.peak / 1048576).toFixed(1), cmd: e.command });
  }
  return { footprint, peak, rss, cpu, processes: f.size, per };
}

function killPids(pids, signal = 'SIGTERM') {
  for (const p of pids) { try { process.kill(p, signal); } catch {} }
}

function dirBytes(dir, filter) {
  let n = 0, files = 0;
  const walk = (d) => {
    for (const e of readdirSync(d, { withFileTypes: true })) {
      const p = join(d, e.name);
      if (e.isDirectory()) walk(p);
      else if (!filter || filter(p)) { n += statSync(p).size; files++; }
    }
  };
  if (existsSync(dir)) walk(dir);
  return { bytes: n, files };
}

function freePort(base) {
  // Ports are spread by pid so that two runs of this script do not collide.
  return base + (process.pid % 200);
}

function env(extra = {}) {
  const e = { ...process.env, PATH: `${join(DART_SDK, 'bin')}:${process.env.PATH}`, ...extra };
  if (PUB_CACHE) e.PUB_CACHE = PUB_CACHE;
  return e;
}

// ------------------------------------------------------------------ servers

/// A running development server: one of ours or webdev's.
class Server {
  constructor(tool, dir) {
    this.tool = tool; // 'dartforge' | 'webdev'
    this.dir = dir;
    this.log = [];
    this.child = null;
    this.baseline = null;
    this.port = 0;
  }

  /// Process ids that belong to this server right now.
  pids() {
    const table = processTable();
    if (this.tool === 'dartforge') return descendants(table, [this.child.pid]).filter((p) => table.some((r) => r.pid === p));
    // webdev: the client, the build daemon it starts (detached) and the
    // persistent workers the daemon starts (also detached, cwd in a scratch
    // space): every new `dart` process whose command is one of those.
    const mine = table.filter((r) => !this.baseline.has(r.pid) &&
      (/webdev(\.dart\.snapshot)?\s+serve|build_runner daemon|build\.dart\.(dill|snapshot)|(dartdevc|kernel_worker)(_aot)?\.dart\.snapshot --persistent_worker|frontend_server/.test(r.command) ||
        r.pid === this.child.pid));
    return descendants(table, mine.map((r) => r.pid));
  }

  /// Starts the server; resolves when it is ready to serve the compiled app.
  async start({ port, cacheDir } = {}) {
    this.port = port;
    this.baseline = new Set(processTable().map((r) => r.pid));
    const p = preset();
    const t0 = now();
    if (this.tool === 'dartforge') {
      const out = join(this.dir, '.dartforge-dev-out');
      const extra = cacheDir ? { DARTFORGE_CACHE_DIR: cacheDir } : {};
      this.child = spawn(DARTFORGE, ['serve', p.entry, '-o', out, '--web', p.web, '--packages', '.dart_tool/package_config.json', '--port', String(port)],
        { cwd: this.dir, env: env(extra), stdio: ['ignore', 'pipe', 'pipe'] });
    } else {
      this.child = spawn(DART, ['run', 'webdev', 'serve', `${p.web}:${port}`, '--auto', 'refresh', '--', '--delete-conflicting-outputs'],
        { cwd: this.dir, env: env(), stdio: ['ignore', 'pipe', 'pipe'] });
    }
    const readyRe = this.tool === 'dartforge' ? /serving at http:\/\/127\.0\.0\.1:(\d+)/ : /Succeeded after|Failed after|Build completed/;
    this.readyMs = await new Promise((ok, fail) => {
      let buf = '';
      let served = this.tool === 'dartforge';
      const onData = (d) => {
        const s = d.toString().replace(/\x1b\[2K/g, '').replace(/\r/g, '\n');
        buf += s;
        for (const line of s.split('\n')) if (line.trim() && !/elapsed, \d+\/\d+ actions/.test(line)) this.log.push(line);
        if (/Serving `/.test(buf)) served = true;
        if (/Failed after/.test(buf)) fail(new Error(`${this.tool}: build failed\n${this.log.slice(-30).join('\n')}`));
        if (served && readyRe.test(buf)) { this.child.stdout.off('data', onData); ok(now() - t0); }
      };
      this.child.stdout.on('data', onData);
      this.child.stderr.on('data', (d) => this.log.push(d.toString()));
      this.child.on('exit', (c) => fail(new Error(`${this.tool} exited with ${c}\n${this.log.slice(-30).join('\n')}`)));
    });
    // Keep draining stdout: a full pipe would block the server.
    // A finished recompilation: build_runner's summary line, or the `total`
    // line of our per-phase report.
    this.builds = [];
    const done = this.tool === 'dartforge' ? /^total\s+[\d.]+ ms|^error:/ : /Succeeded after|Failed after/;
    this.child.stdout.on('data', (d) => {
      const t = now();
      for (const line of d.toString().replace(/\x1b\[2K/g, '').split(/[\r\n]+/)) {
        if (!line.trim() || /elapsed, \d+\/\d+ actions/.test(line)) continue;
        this.log.push(line);
        if (done.test(line.trim())) this.builds.push({ t, line: line.trim() });
      }
      if (this.log.length > 5000) this.log.splice(0, 2500);
    });
    this.url = `http://127.0.0.1:${port}/`;
    return this.readyMs;
  }

  async stop() {
    // The server's own log (per-phase timings of every recompilation).
    try { writeFileSync(join(OUT, `server-${this.tool}-${opts.preset}-${Date.now()}.log`), this.log.join('\n')); } catch {}
    const pids = this.pids();
    killPids(pids, 'SIGTERM');
    await sleep(1500);
    killPids(this.pids(), 'SIGKILL');
    // The build daemon outlives its client by design: make sure nothing is left.
    await sleep(300);
  }
}

// ------------------------------------------------------------------ Chrome / CDP

class Browser {
  constructor(name) { this.name = name; }

  async launch() {
    this.profile = mkdtempSync(join(OUT, `chrome-${this.name}-`));
    this.cdpPort = 9500 + Math.floor(Math.random() * 400);
    this.child = spawn(CHROME, ['--headless=new', '--no-first-run', '--no-default-browser-check', '--disable-gpu',
      '--disable-background-timer-throttling', '--disable-renderer-backgrounding', '--disable-backgrounding-occluded-windows',
      '--disable-extensions', `--user-data-dir=${this.profile}`, `--remote-debugging-port=${this.cdpPort}`, 'about:blank'], { stdio: 'ignore' });
    let target;
    for (let i = 0; i < 200 && !target; i++) {
      try { target = (await (await fetch(`http://127.0.0.1:${this.cdpPort}/json/list`)).json()).find((t) => t.type === 'page'); } catch {}
      if (!target) await sleep(100);
    }
    if (!target) throw new Error('Chrome did not start');
    this.ws = new WebSocket(target.webSocketDebuggerUrl);
    await new Promise((ok) => this.ws.addEventListener('open', ok, { once: true }));
    this.seq = 0;
    this.pending = new Map();
    this.listeners = new Set();
    this.console = [];
    this.errors = [];
    this.loads = 0;
    this.navigations = 0;
    this.responses = new Map();
    this.ws.addEventListener('message', ({ data }) => {
      const m = JSON.parse(data);
      if (m.id) { const p = this.pending.get(m.id); this.pending.delete(m.id); p && p(m); return; }
      const t = now();
      if (m.method === 'Runtime.consoleAPICalled') this.console.push({ t, text: m.params.args.map((a) => a.value ?? a.description ?? '').join(' ') });
      else if (m.method === 'Runtime.exceptionThrown') this.errors.push(m.params.exceptionDetails.exception?.description?.split('\n')[0] || m.params.exceptionDetails.text);
      else if (m.method === 'Page.loadEventFired') { this.loads++; this.lastLoad = t; }
      else if (m.method === 'Page.frameNavigated' && !m.params.frame.parentId) { this.navigations++; this.lastNavigation = t; }
      else if (m.method === 'Network.responseReceived') this.responses.set(m.params.requestId, { url: m.params.response.url, type: m.params.type, mime: m.params.response.mimeType, status: m.params.response.status });
      else if (m.method === 'Network.loadingFinished') { const r = this.responses.get(m.params.requestId); if (r) r.encoded = m.params.encodedDataLength; }
      for (const l of this.listeners) l(m, t);
    });
    await this.send('Page.enable');
    await this.send('Runtime.enable');
  }

  send(method, params = {}) {
    return new Promise((ok) => { const id = ++this.seq; this.pending.set(id, ok); this.ws.send(JSON.stringify({ id, method, params })); });
  }

  async eval(expression, awaitPromise = false) {
    const r = await this.send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise });
    return r.result?.result?.value;
  }

  /// Navigates and waits until the preset's readiness expression holds;
  /// returns in-page times (ms since navigation start) and Chrome metrics.
  async loadPage(url, ready, { timeoutMs = 120000 } = {}) {
    await this.send('Performance.enable');
    const navs = this.navigations;
    const t0 = now();
    await this.send('Page.navigate', { url });
    let readyAt = null;
    while (now() - t0 < timeoutMs) {
      if (this.navigations > navs) {
        const v = await this.eval(`(${ready}) ? performance.now() : null`);
        if (typeof v === 'number') { readyAt = v; break; }
      }
      await sleep(20);
    }
    // Let pending work (change detection, timers) settle before reading metrics.
    await sleep(500);
    const after = await this.metrics();
    const nav = await this.eval("(() => { const n = performance.getEntriesByType('navigation')[0]; const p = performance.getEntriesByName('first-contentful-paint')[0]; return n ? { dcl: n.domContentLoadedEventEnd, loadEventMs: n.loadEventEnd, fcp: p ? p.startTime : null } : null; })()");
    // Chrome resets these metrics on navigation: they cover this page's life.
    return {
      readyMs: readyAt, wallMs: now() - t0, ...nav,
      scriptMs: after.ScriptDuration * 1000,
      taskMs: after.TaskDuration * 1000,
      heapMb: after.JSHeapUsedSize / 1048576,
      nodes: after.Nodes,
    };
  }

  async metrics() {
    const r = await this.send('Performance.getMetrics');
    return Object.fromEntries((r.result?.metrics || []).map((m) => [m.name, m.value]));
  }

  async close() {
    try { this.ws.close(); } catch {}
    try { this.child.kill('SIGKILL'); } catch {}
    await sleep(500);
    rmSync(this.profile, { recursive: true, force: true, maxRetries: 5 });
  }
}

/// The JavaScript the page loaded: scripts fetched during one cold load
/// (cache disabled), measured raw and gzip-compressed (level 6) here.
async function servedJs(browser, url, ready) {
  await browser.send('Network.enable');
  await browser.send('Network.setCacheDisabled', { cacheDisabled: true });
  browser.responses.clear();
  await browser.loadPage(url, ready);
  await sleep(1500);
  const scripts = [...browser.responses.values()].filter((r) => r.type === 'Script' && r.status === 200 && r.url.startsWith(url.replace(/\/$/, '')));
  let raw = 0, gz = 0, wire = 0, n = 0;
  for (const s of scripts) {
    const body = Buffer.from(await (await fetch(s.url)).arrayBuffer());
    raw += body.length;
    gz += gzipSync(body, { level: 6 }).length;
    wire += s.encoded || 0;
    n++;
  }
  await browser.send('Network.setCacheDisabled', { cacheDisabled: false });
  await browser.send('Network.disable');
  return { scripts: n, rawBytes: raw, gzipBytes: gz, wireBytes: wire };
}

// ------------------------------------------------------------------ edits

/// The project's edit state. Every edit changes only its own files relative
/// to the previous state: the component's constructor always holds one
/// `print`, the template one marker, the API library one extra function.
class Edits {
  constructor(dir, p) {
    this.dir = dir;
    this.p = p;
    this.files = {
      component: join(dir, p.component),
      template: join(dir, p.template),
      api: resolve(dir, p.apiFile),
    };
    this.orig = Object.fromEntries(Object.entries(this.files).map(([k, f]) => [k, readFileSync(f, 'utf8')]));
    if (!this.orig.component.includes(p.anchor)) throw new Error(`anchor not found in ${p.component}`);
    this.state = { stmt: null, tpl: null, api: null };
  }

  render() {
    const { stmt, tpl, api } = this.state;
    return {
      component: stmt ? this.orig.component.replace(this.p.anchor, `${this.p.anchor}\n    ${stmt}`) : this.orig.component,
      template: tpl ? `${tpl}\n${this.orig.template}` : this.orig.template,
      api: api ? `${this.orig.api}\n${api}\n` : this.orig.api,
    };
  }

  /// Applies an edit; returns the marker the new code shows and how to see it.
  apply(kind, n, tool) {
    const mark = `df-bench-${tool}-${kind}-${n}-${Date.now()}`;
    const before = this.render();
    if (kind === 'body') this.state.stmt = `print('${mark}');`;
    else if (kind === 'api') {
      this.state.api = `/// Benchmark edit: a new public declaration (changes this library's API).\nString benchApi${n}() => '${mark}';`;
      this.state.stmt = `print(benchApi${n}());`;
    } else if (kind === 'template') this.state.tpl = `<span class="df-bench-mark">${mark}</span>`;
    const after = this.render();
    const written = [];
    for (const k of Object.keys(after)) if (after[k] !== before[k]) { writeFileSync(this.files[k], after[k]); written.push(k); }
    return { mark, via: kind === 'template' ? 'dom' : 'console', written };
  }

  restore() {
    for (const [k, f] of Object.entries(this.files)) writeFileSync(f, this.orig[k]);
  }
}

/// Waits until the page shows `mark` (console or DOM) after a reload.
///
/// When the server reported a finished build but the page did not reload
/// within `graceMs` (webdev's live-reload channel can drop during a long
/// build), the page is reloaded by hand and the result is flagged `manual`.
async function waitMark(browser, server, { mark, via }, t0, timeoutMs = 900000, graceMs = 20000, noMarkMs = 90000) {
  const navs = browser.navigations;
  let reloadAt = null, manual = false;
  const result = (o) => {
    const b = server.builds.find((x) => x.t >= t0);
    return { ...o, reloadMs: reloadAt != null ? reloadAt - t0 : null, buildMs: b ? b.t - t0 : null, manual };
  };
  while (now() - t0 < timeoutMs) {
    if (reloadAt == null && browser.navigations > navs) reloadAt = browser.lastNavigation;
    // Reloaded but the new code never showed: record where the page is and stop waiting.
    if (reloadAt != null && now() - reloadAt > noMarkMs) {
      const where = await browser.eval('location.href + " | " + (document.body ? document.body.innerText.slice(0, 120) : "")');
      return result({ ms: null, noMark: true, where, errors: browser.errors.slice(-3) });
    }
    const built = server.builds.find((x) => x.t >= t0);
    if (built && reloadAt == null && !manual && now() - built.t > graceMs) {
      manual = true;
      await browser.send('Page.reload', { ignoreCache: false });
    }
    if (via === 'console') {
      const hit = browser.console.find((c) => c.t >= t0 && c.text.includes(mark));
      if (hit) return result({ ms: hit.t - t0 });
    } else if (browser.navigations > navs) {
      const v = await browser.eval(`document.body ? document.body.innerHTML.includes(${JSON.stringify(mark)}) : false`);
      if (v) return result({ ms: now() - t0 });
    }
    await sleep(via === 'console' ? 10 : 25);
  }
  return result({ ms: null, timeout: true });
}

// ------------------------------------------------------------------ commands

async function cold() {
  const p = preset();
  const dirs = { dartforge: resolve(opts.ours), webdev: resolve(opts.ddc) };
  const variants = (opts.variants || 'dartforge-cold,dartforge-nocache,dartforge-warm,webdev-cold,webdev-warm,build_runner-cold').split(',');
  for (let rep = 0; rep < REPS; rep++) {
    const order = rep % 2 ? [...variants].reverse() : variants;
    for (const v of order) {
      const [tool, mode] = v.split('-');
      if (tool === 'build_runner') { await buildRunnerCold(dirs.webdev, rep); continue; }
      const dir = dirs[tool];
      let cacheDir;
      if (tool === 'dartforge') {
        if (mode !== 'warm') rmSync(join(dir, '.dartforge-dev-out'), { recursive: true, force: true });
        if (mode === 'nocache') cacheDir = mkdtempSync(join(OUT, 'sdkcache-'));
      } else if (mode === 'cold') rmSync(join(dir, '.dart_tool/build'), { recursive: true, force: true });
      const s = new Server(tool, dir);
      const b = new Browser(tool);
      const load = os.loadavg()[0];
      let readyMs, page, mem;
      try {
        readyMs = await s.start({ port: freePort(tool === 'dartforge' ? 8100 : 8400), cacheDir });
        mem = sumMemory(s.pids());
        await b.launch();
        page = await b.loadPage(s.url, p.ready);
      } catch (e) {
        console.error(String(e).slice(0, 2000));
      } finally {
        await b.close();
        await s.stop();
        if (cacheDir) rmSync(cacheDir, { recursive: true, force: true });
      }
      record('cold', {
        preset: p.name, tool, mode, rep, loadAtStart: load, readyMs, firstPageMs: page?.readyMs != null ? readyMs + page.wallMs : null,
        pageReadyMs: page?.readyMs, cpuSeconds: mem?.cpu, footprintMb: mem && mem.footprint / 1048576, peakMb: mem && mem.peak / 1048576,
        processes: mem?.processes, errors: b.errors?.slice(0, 3),
      });
    }
  }
}

async function buildRunnerCold(dir, rep) {
  rmSync(join(dir, '.dart_tool/build'), { recursive: true, force: true });
  const load = os.loadavg()[0];
  const t0 = now();
  // /usr/bin/time -l reports the CPU time and the max RSS of the whole build
  // (the workers are children of the build script and are waited for).
  const r = spawnSync('/usr/bin/time', ['-l', DART, 'run', 'build_runner', 'build', '--delete-conflicting-outputs'],
    { cwd: dir, env: env(), encoding: 'utf8', maxBuffer: 256 << 20 });
  const ms = now() - t0;
  const err = r.stderr || '';
  const user = +(err.match(/([\d.]+) user/)?.[1] || NaN);
  const sys = +(err.match(/([\d.]+) sys/)?.[1] || NaN);
  const ok = /Succeeded after/.test(r.stdout || '');
  record('cold', { preset: preset().name, tool: 'build_runner', mode: 'build', rep, loadAtStart: load, readyMs: ms, cpuSeconds: user + sys, ok, outputs: (r.stdout || '').match(/with (\d+) outputs/)?.[1] });
}

async function edits() {
  const p = preset();
  const kinds = (opts.kinds || 'body,api,template').split(',');
  const tools = ['dartforge', 'webdev'];
  const dirs = { dartforge: resolve(opts.ours), webdev: resolve(opts.ddc) };
  const servers = {}, browsers = {}, eds = {};
  const cleanup = async () => {
    for (const t of tools) { try { eds[t]?.restore(); } catch {} }
    for (const t of tools) { try { await browsers[t]?.close(); } catch {} }
    for (const t of tools) { try { await servers[t]?.stop(); } catch {} }
  };
  process.on('SIGINT', async () => { await cleanup(); process.exit(130); });
  try {
    for (const t of tools) {
      eds[t] = new Edits(dirs[t], p);
      servers[t] = new Server(t, dirs[t]);
      const readyMs = await servers[t].start({ port: freePort(t === 'dartforge' ? 8100 : 8400) });
      const m = sumMemory(servers[t].pids());
      record('start', { preset: p.name, tool: t, readyMs, footprintMb: m.footprint / 1048576, peakMb: m.peak / 1048576, processes: m.processes, per: m.per });
      browsers[t] = new Browser(t);
      await browsers[t].launch();
    }
    // JavaScript served (one cold load each) and page-load times, alternating.
    for (const t of tools) record('size', { preset: p.name, tool: t, ...(await servedJs(browsers[t], servers[t].url, p.ready)), outDir: t === 'dartforge' ? dirBytes(join(dirs[t], '.dartforge-dev-out'), (f) => /\.m?js$/.test(f)) : undefined });
    const loads = Number(opts.loads || 5);
    for (let i = 0; i < loads; i++) {
      for (const t of i % 2 ? [...tools].reverse() : tools) {
        for (const cache of [false, true]) {
          await browsers[t].send('Network.enable');
          await browsers[t].send('Network.setCacheDisabled', { cacheDisabled: !cache });
          const r = await browsers[t].loadPage(servers[t].url, p.ready);
          record('page', { preset: p.name, tool: t, rep: i, cache, ...r, errors: browsers[t].errors.length });
        }
      }
    }
    // Interaction tour: route by route, in-page time until the routed
    // component replaced the previous one, plus the script time Chrome measured.
    if (p.routes.length) {
      for (let i = 0; i < Number(opts.tours || 3); i++) {
        for (const t of i % 2 ? [...tools].reverse() : tools) {
          const b = browsers[t];
          await b.loadPage(servers[t].url, p.ready);
          const before = await b.metrics();
          const r = await b.eval(`(async (routes) => {
            const frame = () => new Promise((ok) => requestAnimationFrame(() => setTimeout(ok, 0)));
            const outlet = () => { const o = document.querySelector('router-outlet'); return o && o.nextElementSibling; };
            const out = [];
            const t0 = performance.now();
            for (const r of routes) {
              const prev = outlet();
              const s = performance.now();
              location.hash = r;
              for (let k = 0; k < 400; k++) { await frame(); const cur = outlet(); if (cur && cur !== prev && cur.isConnected) break; }
              await frame();
              out.push(Math.round(performance.now() - s));
            }
            return { totalMs: performance.now() - t0, perRoute: out };
          })(${JSON.stringify(Array.from({ length: Number(opts.laps || 3) }, () => p.routes).flat())})`, true);
          const after = await b.metrics();
          record('tour', { preset: p.name, tool: t, rep: i, ...r, scriptMs: (after.ScriptDuration - before.ScriptDuration) * 1000, taskMs: (after.TaskDuration - before.TaskDuration) * 1000, heapMb: after.JSHeapUsedSize / 1048576, errors: b.errors.length });
        }
      }
      for (const t of tools) await browsers[t].loadPage(servers[t].url, p.ready);
    }
    // Edit loop: kinds round-robin, tools alternating, memory after every edit.
    const reps = REPS;
    let n = 0;
    for (let rep = 0; rep < reps; rep++) {
      for (const kind of kinds) {
        n++;
        const order = n % 2 ? tools : [...tools].reverse();
        for (const t of order) {
          const b = browsers[t];
          const load = os.loadavg()[0];
          const cpuBefore = sumMemory(servers[t].pids()).cpu;
          const t0 = now();
          const e = eds[t].apply(kind, n, t);
          const w = await waitMark(b, servers[t], e, t0);
          await sleep(Number(opts.settle || 1500));
          const m = sumMemory(servers[t].pids());
          record('edit', { preset: p.name, tool: t, edit: kind, n, loadAtStart: load, ms: w.ms, reloadMs: w.reloadMs, buildMs: w.buildMs, manualReload: w.manual, timeout: !!w.timeout, noMark: w.noMark, where: w.where, pageErrors: w.errors, cpuSeconds: m.cpu - cpuBefore,
            footprintMb: m.footprint / 1048576, peakMb: m.peak / 1048576, rssMb: m.rss / 1048576, processes: m.processes, files: e.written });
          if (w.timeout) console.error(servers[t].log.slice(-30).join('\n'));
        }
      }
    }
    for (const t of tools) {
      const m = sumMemory(servers[t].pids());
      record('memory-final', { preset: p.name, tool: t, footprintMb: m.footprint / 1048576, peakMb: m.peak / 1048576, processes: m.processes, per: m.per });
    }
  } finally {
    await cleanup();
  }
}

/// bench/performance programs under Node: dartdevc (ES modules, linked to the
/// DDC-built dart_sdk.js) against `dartforge compile-js`; the VM is the oracle
/// for the printed results.
async function nodeBench() {
  // A private copy of the programs: the measurement must not see edits made
  // to the tree while it runs.
  const work = join(OUT, 'node-bench');
  const bench = join(work, 'src');
  rmSync(bench, { recursive: true, force: true });
  cpSync(resolve(opts.bench || join(repo, 'bench/performance')), bench, { recursive: true });
  const sdkJs = resolve(opts['dart-sdk-js'] || process.env.DARTFORGE_DART_SDK_JS || join(repo, 'runtime/ddc/dart_sdk.js'));
  const programs = (opts.programs ? opts.programs.split(',') : readdirSync(bench).filter((f) => f.endsWith('.dart') && !['common.dart', 'comum.dart'].includes(f)).map((f) => f.replace(/\.dart$/, '')));
  const parse = (text) => {
    const rounds = {}, results = {};
    for (const line of text.split('\n')) {
      const m = line.match(/^([\w ]+): ([\d ]+) us \| (.*)$/);
      if (m) { rounds[m[1]] = m[2].trim().split(/\s+/).map(Number); results[m[1]] = m[3]; }
    }
    return { rounds, results, other: text.split('\n').filter((l) => l && !/ us \| /.test(l)).join('\n') };
  };
  for (const prog of programs) {
    const src = join(bench, `${prog}.dart`);
    const ddcDir = join(work, prog, 'ddc');
    const dfDir = join(work, prog, 'dartforge');
    mkdirSync(ddcDir, { recursive: true });
    rmSync(dfDir, { recursive: true, force: true });
    // Compile both (timed, but compile time of a single small program is not the point here).
    let t = now();
    const c1 = spawnSync(DART, [join(DART_SDK, 'bin/snapshots/dartdevc.dart.snapshot'), '--modules=es6', '-o', join(ddcDir, `${prog}.js`), `${prog}.dart`], { cwd: bench, encoding: 'utf8' });
    const ddcCompileMs = now() - t;
    if (c1.status !== 0) { record('node', { program: prog, error: `dartdevc: ${c1.stdout}${c1.stderr}`.slice(0, 500) }); continue; }
    let js = readFileSync(join(ddcDir, `${prog}.js`), 'utf8').replace("from 'dart_sdk.js'", `from '${'file://' + sdkJs}'`);
    writeFileSync(join(ddcDir, `${prog}.js`), js);
    const exp = js.split('\n').find((l) => l.startsWith('export {')).replace('export {', '').replace(/[};]/g, '').split(',')[0].trim();
    const name = exp.includes(' as ') ? exp.split(' as ')[1].trim() : exp;
    writeFileSync(join(ddcDir, 'main.mjs'), `import { ${name} as m } from './${prog}.js';\nm.main();\n`);
    t = now();
    const c2 = spawnSync(DARTFORGE, ['compile-js', src, '-o', dfDir], { cwd: bench, encoding: 'utf8', env: env({ DARTFORGE_DART_SDK_JS: sdkJs }) });
    const dfCompileMs = now() - t;
    if (c2.status !== 0) { record('node', { program: prog, error: `dartforge: ${c2.stdout}${c2.stderr}`.slice(0, 500) }); continue; }
    const vm = parse(spawnSync(DART, ['run', src], { cwd: bench, encoding: 'utf8' }).stdout || '');
    for (let rep = 0; rep < REPS; rep++) {
      for (const [tool, dir] of rep % 2 ? [['dartforge', dfDir], ['ddc', ddcDir]] : [['ddc', ddcDir], ['dartforge', dfDir]]) {
        const load = os.loadavg()[0];
        const t0 = now();
        const r = spawnSync('/usr/bin/time', ['-l', process.execPath, 'main.mjs'], { cwd: dir, encoding: 'utf8', maxBuffer: 64 << 20 });
        const wallMs = now() - t0;
        const out = parse(r.stdout || '');
        const err = r.stderr || '';
        const cpu = +(err.match(/([\d.]+) user/)?.[1] || NaN) + +(err.match(/([\d.]+) sys/)?.[1] || NaN);
        const maxRss = +(err.match(/(\d+)\s+maximum resident set size/)?.[1] || NaN);
        const kernels = Object.fromEntries(Object.entries(out.rounds).map(([k, v]) => [k, median(v.slice(1)) / 1000]));
        const sameAsVm = Object.keys(vm.results).length > 0 && Object.keys(vm.results).every((k) => out.results[k] === vm.results[k]) && out.other === vm.other;
        record('node', { program: prog, tool, rep, loadAtStart: load, exit: r.status, wallMs, cpuSeconds: cpu, maxRssMb: maxRss / 1048576, kernelsMs: kernels, results: out.results, sameAsVm,
          compileMs: tool === 'ddc' ? ddcCompileMs : dfCompileMs, diff: sameAsVm ? undefined : { vm: vm.results, got: out.results, vmOther: vm.other.slice(0, 300), other: out.other.slice(0, 300), stderr: err.split('\n').slice(0, 5).join('\n') } });
      }
    }
  }
}

// ------------------------------------------------------------------ report

function report() {
  const rows = readFileSync(join(OUT, 'results.jsonl'), 'utf8').trim().split('\n').map((l) => JSON.parse(l))
    .filter((r) => !opts.tag || r.tag === opts.tag);
  const fmt = (xs, unit = '', digits = 0) => {
    xs = xs.filter((x) => typeof x === 'number' && isFinite(x));
    if (!xs.length) return '—';
    const f = (x) => x.toFixed(digits);
    return xs.length === 1 ? `${f(xs[0])}${unit}` : `${f(median(xs))}${unit} [${f(Math.min(...xs))}–${f(Math.max(...xs))}] n=${xs.length}`;
  };
  const group = (rs, key) => { const g = new Map(); for (const r of rs) { const k = key(r); if (!g.has(k)) g.set(k, []); g.get(k).push(r); } return g; };
  const loads = (rs) => fmt(rs.map((r) => r.load?.[0]), '', 0);
  const lines = [];
  const cold = rows.filter((r) => r.kind === 'cold');
  if (cold.length) {
    lines.push('### Start (ms, median [min–max])', '', '| project | tool | mode | ready | first page rendered | CPU s | footprint MB | load 1 min |', '|---|---|---|---|---|---|---|---|');
    for (const [k, rs] of group(cold, (r) => `${r.preset}|${r.tool}|${r.mode}`)) {
      const [pr, tool, mode] = k.split('|');
      lines.push(`| ${pr} | ${tool} | ${mode} | ${fmt(rs.map((r) => r.readyMs))} | ${fmt(rs.map((r) => r.firstPageMs))} | ${fmt(rs.map((r) => r.cpuSeconds), '', 1)} | ${fmt(rs.map((r) => r.footprintMb))} | ${loads(rs)} |`);
    }
    lines.push('');
  }
  const ed = rows.filter((r) => r.kind === 'edit');
  if (ed.length) {
    lines.push('### Edit → page reloaded with the new code (ms)', '', '| project | edit | tool | edit→new code visible | edit→build done | edit→reload starts | CPU s per edit | manual reloads / timeouts | load 1 min |', '|---|---|---|---|---|---|---|---|---|');
    for (const [k, rs] of group(ed, (r) => `${r.preset}|${r.edit}|${r.tool}`)) {
      const [pr, kind, tool] = k.split('|');
      lines.push(`| ${pr} | ${kind} | ${tool} | ${fmt(rs.map((r) => r.ms))} | ${fmt(rs.map((r) => r.buildMs))} | ${fmt(rs.map((r) => r.reloadMs))} | ${fmt(rs.map((r) => r.cpuSeconds), '', 2)} | ${rs.filter((r) => r.manualReload).length} / ${rs.filter((r) => r.timeout).length} | ${loads(rs)} |`);
    }
    lines.push('', '### Memory across the edit loop (physical footprint, MB)', '', '| project | tool | after start | after edits: median [min–max] | last | sum of lifetime peaks | processes |', '|---|---|---|---|---|---|---|');
    for (const [k, rs] of group(ed, (r) => `${r.preset}|${r.tool}`)) {
      const [pr, tool] = k.split('|');
      const start = rows.filter((r) => r.kind === 'start' && r.preset === pr && r.tool === tool).at(-1);
      const fin = rows.filter((r) => r.kind === 'memory-final' && r.preset === pr && r.tool === tool).at(-1);
      lines.push(`| ${pr} | ${tool} | ${start ? start.footprintMb.toFixed(0) : '—'} | ${fmt(rs.map((r) => r.footprintMb))} | ${rs.at(-1).footprintMb.toFixed(0)} | ${fin ? fin.peakMb.toFixed(0) : '—'} | ${fin?.processes ?? '—'} |`);
    }
    lines.push('');
  }
  const size = rows.filter((r) => r.kind === 'size');
  if (size.length) {
    lines.push('### JavaScript served on a cold page load', '', '| project | tool | scripts | raw MB | gzip MB |', '|---|---|---|---|---|');
    for (const r of size) lines.push(`| ${r.preset} | ${r.tool} | ${r.scripts} | ${(r.rawBytes / 1048576).toFixed(2)} | ${(r.gzipBytes / 1048576).toFixed(2)} |`);
    lines.push('');
  }
  const page = rows.filter((r) => r.kind === 'page');
  if (page.length) {
    lines.push('### Page load in headless Chrome (ms from navigation start)', '', '| project | tool | HTTP cache | app rendered | DOMContentLoaded | script time | JS heap MB | load 1 min |', '|---|---|---|---|---|---|---|---|');
    for (const [k, rs] of group(page, (r) => `${r.preset}|${r.tool}|${r.cache}`)) {
      const [pr, tool, cache] = k.split('|');
      lines.push(`| ${pr} | ${tool} | ${cache === 'true' ? 'on' : 'off'} | ${fmt(rs.map((r) => r.readyMs))} | ${fmt(rs.map((r) => r.dcl))} | ${fmt(rs.map((r) => r.scriptMs))} | ${fmt(rs.map((r) => r.heapMb), '', 1)} | ${loads(rs)} |`);
    }
    lines.push('');
  }
  const tour = rows.filter((r) => r.kind === 'tour');
  if (tour.length) {
    lines.push('### Route tour in Chrome (ms)', '', '| project | tool | total | script time | task time | JS heap MB |', '|---|---|---|---|---|---|');
    for (const [k, rs] of group(tour, (r) => `${r.preset}|${r.tool}`)) {
      const [pr, tool] = k.split('|');
      lines.push(`| ${pr} | ${tool} | ${fmt(rs.map((r) => r.totalMs))} | ${fmt(rs.map((r) => r.scriptMs))} | ${fmt(rs.map((r) => r.taskMs))} | ${fmt(rs.map((r) => r.heapMb), '', 1)} |`);
    }
    lines.push('');
  }
  const nodeRows = rows.filter((r) => r.kind === 'node' && !r.error);
  if (nodeRows.length) {
    lines.push('### bench/performance under Node (kernel ms: median of rounds 2..n, then median over runs)', '', '| program | kernel | DDC | DartForge | DartForge/DDC | same result as DDC | same as VM (DDC / DartForge) |', '|---|---|---|---|---|---|---|');
    for (const [prog, rs] of group(nodeRows, (r) => r.program)) {
      const kernels = new Set(rs.flatMap((r) => Object.keys(r.kernelsMs || {})));
      for (const k of kernels) {
        const d = median(rs.filter((r) => r.tool === 'ddc').map((r) => r.kernelsMs[k]).filter((x) => x != null));
        const f = median(rs.filter((r) => r.tool === 'dartforge').map((r) => r.kernelsMs[k]).filter((x) => x != null));
        // JavaScript semantics (String.hashCode, number printing) legitimately
        // differ from the VM: the reference for DartForge is DDC's output.
        const vm = (t) => rs.filter((r) => r.tool === t).every((r) => r.sameAsVm) ? 'yes' : 'no';
        const ref = rs.find((r) => r.tool === 'ddc')?.results?.[k];
        const same = rs.filter((r) => r.tool === 'dartforge').every((r) => r.results?.[k] === ref) ? 'yes' : 'NO';
        lines.push(`| ${prog} | ${k} | ${d.toFixed(1)} | ${f.toFixed(1)} | ${(f / d).toFixed(2)} | ${same} | ${vm('ddc')} / ${vm('dartforge')} |`);
      }
    }
    lines.push('');
  }
  for (const r of rows.filter((r) => r.kind === 'node' && r.error)) lines.push(`* ${r.program}: ${r.error.split('\n')[0]}`);
  console.log(lines.join('\n'));
}

// ------------------------------------------------------------------ main

const commands = { cold, edits, node: nodeBench, report };
if (!commands[cmd]) {
  console.error('usage: node scripts/compare-dev-ddc.mjs cold|edits|node|report [options] (see the header)');
  process.exit(2);
}
await commands[cmd]();
process.exit(0);

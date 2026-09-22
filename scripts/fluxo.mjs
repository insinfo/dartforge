// Verificador de fluxo real do new_sali/frontend no Edge headless.
//
// Serve o diretório emitido (com fallback de SPA: rotas desconhecidas devolvem
// o index.html, porque o ngdart usa `routerProviders` com `<base href="/">`),
// abre o Edge com `--remote-debugging-port`, e conduz uma sequência de passos
// pelo CDP (Chrome DevTools Protocol) por WebSocket: navegar, procurar
// elementos-chave, clicar, esperar a aplicação reagir. Em cada passo recolhe
// `console.*`, `window.onerror`, `unhandledrejection` e as exceções que o
// runtime do Dart lança, e imprime um relatório por passo.
//
// Uso:
//   node scripts/fluxo.mjs --dir work/fe/out --web C:/.../frontend/web [--porta 8771]
//                          [--json relatorio.json] [--visivel] [--ddc]
//
// Sem dependências: o Node 22+ tem `WebSocket` global.

import { createServer } from 'node:http';
import { spawn } from 'node:child_process';
import { existsSync, readFileSync, statSync, mkdtempSync, writeFileSync, cpSync } from 'node:fs';
import { join, extname, resolve, sep } from 'node:path';
import { tmpdir } from 'node:os';

// --------------------------------------------------------------- argumentos

function args(argv) {
  const o = { dir: 'work/fe/out', web: '', porta: 8771, json: '', visivel: false, rotulo: 'DartForge' };
  for (let i = 2; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--visivel') o.visivel = true;
    else if (a.startsWith('--')) o[a.slice(2)] = argv[++i];
  }
  o.porta = Number(o.porta);
  return o;
}
const op = args(process.argv);
const raiz = resolve(op.dir);

// ------------------------------------------------------------ servidor HTTP

const TIPOS = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.mjs': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
  '.map': 'application/json; charset=utf-8',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.jpeg': 'image/jpeg',
  '.gif': 'image/gif',
  '.ico': 'image/x-icon',
  '.woff': 'font/woff',
  '.woff2': 'font/woff2',
  '.ttf': 'font/ttf',
  '.eot': 'application/vnd.ms-fontobject',
  '.mp4': 'video/mp4',
  '.webm': 'video/webm',
  '.pdf': 'application/pdf',
};

const naoEncontrados = new Set();

function servidor() {
  return createServer((req, res) => {
    const url = new URL(req.url, 'http://127.0.0.1');
    let rel = decodeURIComponent(url.pathname).replace(/^\/+/, '');
    if (rel === '') rel = 'index.html';
    let arquivo = resolve(join(raiz, rel));
    if (!arquivo.startsWith(raiz + sep) && arquivo !== raiz) {
      res.writeHead(403).end('fora da raiz');
      return;
    }
    if (!existsSync(arquivo) || statSync(arquivo).isDirectory()) {
      // Fallback de SPA: só para navegações (sem extensão conhecida).
      if (extname(arquivo) === '') {
        arquivo = join(raiz, 'index.html');
      } else {
        naoEncontrados.add('/' + rel);
        res.writeHead(404).end('não encontrado');
        return;
      }
    }
    res.writeHead(200, { 'Content-Type': TIPOS[extname(arquivo)] || 'application/octet-stream' });
    res.end(readFileSync(arquivo));
  });
}

// ------------------------------------------------------------- index.html

// Injeta o coletor de erros e troca `main.dart.js` pelo módulo emitido.
function prepararIndex() {
  if (!op.web) return;
  const fonte = readFileSync(join(op.web, 'index.html'), 'utf8');
  const coletor = `<script>
window.__erros = [];
window.__reg = function (t) { window.__erros.push(String(t)); };
window.onerror = function (m, s, l, c, e) { window.__reg(String(m) + " @" + s + ":" + l + (e && e.stack ? "\\n" + e.stack : "")); };
window.addEventListener("unhandledrejection", function (ev) { var r = ev.reason; window.__reg("rejeição: " + String(r) + (r && r.stack ? "\\n" + r.stack : "")); });
var __log = console.error; console.error = function () { window.__reg("console.error: " + Array.from(arguments).map(String).join(" ")); __log.apply(console, arguments); };
</script>`;
  let html = fonte.replace(/<script[^>]*src="main\.dart\.js"[^>]*>\s*<\/script>/, '');
  const entrada = op.ddc ? `<script type="module">import { main } from './main.js'; main.main();</script>` : `<script type="module" src="main.mjs"></script>`;
  html = html.replace('</head>', `${coletor}\n${entrada}\n</head>`);
  writeFileSync(join(raiz, 'index.html'), html);
  for (const a of ['assets', 'favicon.ico', 'manifest.json', 'scrollbar.css']) {
    const src = join(op.web, a);
    if (existsSync(src) && !existsSync(join(raiz, a))) {
      cpSync(src, join(raiz, a), { recursive: true });
    }
  }
}

// ------------------------------------------------------------------- CDP

class Cdp {
  constructor(ws) {
    this.ws = ws;
    this.id = 0;
    this.pendentes = new Map();
    this.eventos = [];
    this.hooks = new Map();
    ws.addEventListener('message', (ev) => {
      const m = JSON.parse(ev.data);
      if (m.id !== undefined) {
        const p = this.pendentes.get(m.id);
        this.pendentes.delete(m.id);
        if (p) m.error ? p.rej(new Error(JSON.stringify(m.error))) : p.res(m.result);
      } else {
        this.eventos.push(m);
        const h = this.hooks.get(m.method);
        if (h) h(m.params);
      }
    });
  }
  ao(method, fn) {
    this.hooks.set(method, fn);
  }
  envia(method, params = {}) {
    const id = ++this.id;
    this.ws.send(JSON.stringify({ id, method, params }));
    return new Promise((res, rej) => {
      this.pendentes.set(id, { res, rej });
      setTimeout(() => this.pendentes.has(id) && (this.pendentes.delete(id), rej(new Error(`tempo esgotado: ${method}`))), 30000);
    });
  }
  // Erros do protocolo (console, exceções) desde um marco.
  drenar(marco) {
    const novos = this.eventos.slice(marco);
    const erros = [];
    for (const e of novos) {
      if (e.method === 'Runtime.exceptionThrown') {
        const d = e.params.exceptionDetails;
        const desc = d.exception?.description || d.text;
        erros.push(`exceção: ${desc}`);
      } else if (e.method === 'Runtime.consoleAPICalled' && (e.params.type === 'error' || e.params.type === 'warning')) {
        const txt = e.params.args.map((a) => a.description || a.value).join(' ');
        if (txt) erros.push(`console.${e.params.type}: ${txt}`);
      } else if (e.method === 'Log.entryAdded' && e.params.entry.level === 'error') {
        erros.push(`log: ${e.params.entry.text} ${e.params.entry.url || ''}`);
      }
    }
    return erros;
  }
  async avalia(expr) {
    const r = await this.envia('Runtime.evaluate', { expression: expr, returnByValue: true, awaitPromise: true, userGesture: true });
    if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description || r.exceptionDetails.text);
    return r.result.value;
  }
}

const espera = (ms) => new Promise((r) => setTimeout(r, ms));

async function esperarPor(cdp, expr, limite = 8000) {
  const fim = Date.now() + limite;
  for (;;) {
    let v = false;
    try {
      v = await cdp.avalia(expr);
    } catch {}
    if (v) return true;
    if (Date.now() > fim) return false;
    await espera(200);
  }
}

// ---------------------------------------------------------------- passos

// Resumo do que está montado: componentes ngdart (tags com hífen) e controles.
const RESUMO = `(() => {
  const tags = {};
  for (const el of document.querySelectorAll('*')) {
    const t = el.tagName.toLowerCase();
    if (t.includes('-')) tags[t] = (tags[t] || 0) + 1;
  }
  const texto = (document.body ? document.body.innerText : '').replace(/\\s+/g, ' ').trim();
  return {
    url: location.pathname + location.search + location.hash,
    componentes: Object.keys(tags).sort(),
    botoes: [...document.querySelectorAll('button')].map(b => (b.id ? '#' + b.id + ':' : '') + (b.innerText || b.getAttribute('aria-label') || '').trim()).filter(Boolean).slice(0, 12),
    entradas: [...document.querySelectorAll('input,select,textarea')].map(i => i.tagName.toLowerCase() + (i.type ? '[' + i.type + ']' : '') + (i.name ? '#' + i.name : '')).slice(0, 12),
    links: [...document.querySelectorAll('a[href]')].map(a => a.getAttribute('href')).filter(h => h && !h.startsWith('http')).slice(0, 12),
    alertas: [...document.querySelectorAll('.alert,.invalid-feedback,[role=alert]')].map(e => e.innerText.replace(/\\s+/g, ' ').trim()).filter(Boolean).slice(0, 6),
    tamanhoTexto: texto.length,
    trecho: texto.slice(0, 160),
  };
})()`;

async function passo(cdp, nome, corpo) {
  const marco = cdp.eventos.length;
  const antes = (await cdp.avalia('window.__erros ? window.__erros.length : 0')) || 0;
  let detalhe = {};
  let falha = null;
  try {
    detalhe = (await corpo()) || {};
  } catch (e) {
    falha = String(e.message || e);
  }
  let resumo = {};
  try {
    resumo = await cdp.avalia(RESUMO);
  } catch (e) {
    falha = falha || String(e.message || e);
  }
  const daPagina = (await cdp.avalia(`(window.__erros || []).slice(${antes})`)) || [];
  const doProtocolo = cdp.drenar(marco);
  const todos = [...new Set([...daPagina, ...doProtocolo])];
  // 404 de recurso é do ambiente (já listado em `naoEncontrados`), não da aplicação.
  const ambiente = todos.filter((e) => /Failed to load resource/.test(e));
  const erros = todos.filter((e) => !ambiente.includes(e));
  return { nome, ...detalhe, resumo, erros, ambiente, falha };
}

// ------------------------------------------------------------------ main

async function main() {
  prepararIndex();
  const srv = servidor();
  await new Promise((r) => srv.listen(op.porta, '127.0.0.1', r));
  const base = `http://127.0.0.1:${op.porta}`;

  const edge = ['C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe', 'C:/Program Files/Microsoft/Edge/Application/msedge.exe'].find(existsSync);
  if (!edge) throw new Error('Edge não encontrado');
  const perfil = mkdtempSync(join(tmpdir(), 'dartforge-edge-'));
  const portaCdp = op.porta + 1;
  const navegador = spawn(edge, [
    ...(op.visivel ? [] : ['--headless=new']),
    '--disable-gpu',
    '--no-first-run',
    '--no-default-browser-check',
    '--disable-features=Translate,MediaRouter',
    `--user-data-dir=${perfil}`,
    `--remote-debugging-port=${portaCdp}`,
    'about:blank',
  ], { stdio: 'ignore' });

  const relatorio = { rotulo: op.rotulo, passos: [], naoEncontrados: [] };
  try {
    // Espera o endpoint do CDP.
    let alvo = null;
    for (let i = 0; i < 100 && !alvo; i++) {
      try {
        const lista = await (await fetch(`http://127.0.0.1:${portaCdp}/json/list`)).json();
        alvo = lista.find((t) => t.type === 'page');
      } catch {}
      if (!alvo) await espera(200);
    }
    if (!alvo) throw new Error('CDP não respondeu');

    const ws = new WebSocket(alvo.webSocketDebuggerUrl);
    await new Promise((res, rej) => {
      ws.addEventListener('open', res, { once: true });
      ws.addEventListener('error', rej, { once: true });
    });
    const cdp = new Cdp(ws);
    await cdp.envia('Runtime.enable');
    await cdp.envia('Log.enable');
    await cdp.envia('Page.enable');
    await cdp.envia('Network.enable');

    // Navegações para fora do servidor local (o IdP) são bloqueadas e
    // registradas: o interesse é a reação da aplicação, não o IdP.
    const externas = [];
    await cdp.envia('Fetch.enable', { patterns: [{ urlPattern: '*', requestStage: 'Request' }] });
    cdp.ao('Fetch.requestPaused', (p) => {
      const u = p.request.url;
      if (u.startsWith(base) || u.startsWith('data:') || u.startsWith('blob:')) {
        cdp.envia('Fetch.continueRequest', { requestId: p.requestId }).catch(() => {});
      } else {
        externas.push(u);
        cdp.envia('Fetch.failRequest', { requestId: p.requestId, errorReason: 'BlockedByClient' }).catch(() => {});
      }
    });

    const MONTADO = `!!document.querySelector('my-app router-outlet') && document.querySelector('my-app').querySelectorAll('*').length > 6`;
    const navegar = async (caminho) => {
      await cdp.envia('Page.navigate', { url: base + caminho });
      await espera(300);
      const ok = await esperarPor(cdp, MONTADO, 40000);
      await espera(800);
      return ok;
    };

    // 1. Carga inicial: a aplicação monta o componente raiz e a rota padrão.
    relatorio.passos.push(await passo(cdp, '1. carga inicial (/)', async () => {
      const montou = await navegar('/');
      return { montou, rotaRenderizada: await cdp.avalia(`!!document.querySelector('pre-login-page')`) };
    }));

    // 2. Interação: carrossel Bootstrap (interop JS + dart:html na página).
    relatorio.passos.push(await passo(cdp, '2. clique no carrossel (Slide 3)', async () => {
      const antes = await cdp.avalia(`(() => { const a = document.querySelector('.carousel-item.active'); return a ? [...a.parentElement.children].indexOf(a) : -1; })()`);
      await cdp.avalia(`(() => { const b = document.querySelector('[aria-label="Slide 3"]'); if (!b) throw new Error('botão do carrossel ausente'); b.click(); return true; })()`);
      await espera(1500);
      const depois = await cdp.avalia(`(() => { const a = document.querySelector('.carousel-item.active'); return a ? [...a.parentElement.children].indexOf(a) : -1; })()`);
      return { slideAntes: antes, slideDepois: depois, mudou: antes !== depois };
    }));

    // 3. Reação a um login recusado pelo IdP: `error_code` na query deve
    //    renderizar o alerta (onActivate + queryParameters + *ngIf).
    relatorio.passos.push(await passo(cdp, '3. login recusado (/login?error_code=GOVBR_BRONZE)', async () => {
      await navegar('/login?error_code=GOVBR_BRONZE');
      const achou = await esperarPor(cdp, `!!document.querySelector('.alert-warning')`, 8000);
      const texto = achou ? await cdp.avalia(`document.querySelector('.alert-warning').innerText.replace(/\\s+/g, ' ').trim().slice(0, 120)`) : null;
      return { alertaRenderizado: achou, textoAlerta: texto };
    }));

    // 4. Submeter "Entrar": `doLogin()` pede o redirecionamento ao IdP; a
    //    navegação externa é bloqueada e o URL de autorização é conferido.
    relatorio.passos.push(await passo(cdp, '4. submeter "Entrar" (redirecionamento OIDC)', async () => {
      externas.length = 0;
      await cdp.avalia(`(() => { const b = document.querySelector('#submit'); if (!b) throw new Error('botão #submit ausente'); b.click(); return true; })()`);
      await espera(2000);
      const urls = externas.filter((u) => u.startsWith('http'));
      const autorizacao = urls.find((u) => u.includes('authorize'));
      const r = {
        pedidosExternosBloqueados: urls.length,
        urlAutorizacao: autorizacao ? autorizacao.slice(0, 120) + '…' : null,
        temPkce: !!autorizacao && autorizacao.includes('code_challenge'),
        redirecionouParaOIdp: !!autorizacao,
      };
      // O IdP está fora do ar (pedido bloqueado): volta ao estado conhecido.
      await navegar('/login');
      return r;
    }));

    // 5..8. Rotas públicas e a rota restrita (deve devolver ao login).
    for (const [nome, rota, chave] of [
      ['5. rota /sobre', '/sobre', 'sobre-page'],
      ['6. rota /sessao-expirou', '/sessao-expirou', null],
      ['7. rota restrita /restrito/home (sem sessão)', '/restrito/home', null],
      ['8. volta a /login', '/login', 'pre-login-page'],
    ]) {
      relatorio.passos.push(await passo(cdp, nome, async () => {
        const montou = await navegar(rota);
        const r = { montou };
        if (chave) {
          r.componenteEsperado = chave;
          r.presente = await esperarPor(cdp, `!!document.querySelector('${chave}')`, 8000);
        }
        return r;
      }));
    }

    // 9. Navegação pelo router, sem recarregar (link interno, se houver).
    relatorio.passos.push(await passo(cdp, '9. navegação pelo router (link interno)', async () => {
      const href = await cdp.avalia(`(() => { const a = [...document.querySelectorAll('a[href]')].find(x => { const h = x.getAttribute('href'); return h && h.startsWith('/') && !h.startsWith('//'); }); if (!a) return null; a.click(); return a.getAttribute('href'); })()`);
      await espera(1500);
      return { linkClicado: href, urlApos: await cdp.avalia('location.pathname') };
    }));

    relatorio.naoEncontrados = [...naoEncontrados].sort();
    ws.close();
  } finally {
    navegador.kill();
    srv.close();
  }

  // Relatório.
  const linhas = [];
  let comErro = 0;
  for (const p of relatorio.passos) {
    const r = p.resumo || {};
    linhas.push(`### ${p.nome}`);
    if (p.falha) linhas.push(`  FALHA DO PASSO: ${p.falha}`);
    linhas.push(`  url: ${r.url ?? '?'}  texto: ${r.tamanhoTexto ?? 0} chars`);
    if (r.componentes?.length) linhas.push(`  componentes: ${r.componentes.join(', ')}`);
    if (r.botoes?.length) linhas.push(`  botões: ${r.botoes.join(' | ')}`);
    if (r.entradas?.length) linhas.push(`  entradas: ${r.entradas.join(', ')}`);
    if (r.alertas?.length) linhas.push(`  alertas: ${r.alertas.join(' | ')}`);
    for (const [k, v] of Object.entries(p)) {
      if (['nome', 'resumo', 'erros', 'ambiente', 'falha'].includes(k)) continue;
      linhas.push(`  ${k}: ${JSON.stringify(v)}`);
    }
    if (p.erros.length) {
      comErro++;
      for (const e of p.erros.slice(0, 6)) linhas.push(`  ERRO: ${e.split('\n').slice(0, 4).join(' | ').slice(0, 400)}`);
      if (p.erros.length > 6) linhas.push(`  (+${p.erros.length - 6} erros)`);
    } else {
      linhas.push('  sem erros');
    }
    if (p.ambiente?.length) linhas.push(`  ambiente: ${p.ambiente.length} recurso(s) 404`);
  }
  linhas.push(`\n${relatorio.rotulo}: ${relatorio.passos.length} passos, ${comErro} com erro; 404 do servidor: ${relatorio.naoEncontrados.length}`);
  if (relatorio.naoEncontrados.length) linhas.push(`  404: ${relatorio.naoEncontrados.slice(0, 10).join(' ')}`);
  console.log(linhas.join('\n'));
  if (op.json) writeFileSync(op.json, JSON.stringify(relatorio, null, 2));
}

main().then(
  () => process.exit(0),
  (e) => {
    console.error('fluxo: ' + (e.stack || e.message || e));
    process.exit(1);
  },
);

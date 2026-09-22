// Verifica a recarga automática de ponta a ponta: abre a página no Edge por
// CDP, conta os carregamentos, edita um componente e espera o navegador
// recarregar sozinho.
import { spawn } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';

const URL_BASE = process.argv[2] || 'http://127.0.0.1:8099/';
const ARQUIVO = process.argv[3];
const EDGE = 'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe';
const PORTA_CDP = 9333;

const dormir = (ms) => new Promise((r) => setTimeout(r, ms));

const edge = spawn(EDGE, [
  '--headless=new',
  `--remote-debugging-port=${PORTA_CDP}`,
  '--no-first-run',
  '--user-data-dir=' + process.env.TEMP + '/df-recarga',
  URL_BASE,
]);

async function alvo() {
  for (let i = 0; i < 40; i++) {
    try {
      const r = await fetch(`http://127.0.0.1:${PORTA_CDP}/json/list`);
      const abas = await r.json();
      const p = abas.find((a) => a.type === 'page' && a.webSocketDebuggerUrl);
      if (p) return p;
    } catch {}
    await dormir(500);
  }
  throw new Error('Edge não respondeu');
}

const pagina = await alvo();
const ws = new WebSocket(pagina.webSocketDebuggerUrl);
await new Promise((r) => (ws.onopen = r));
let id = 0;
const pendentes = new Map();
let carregamentos = 0;
ws.onmessage = (ev) => {
  const m = JSON.parse(ev.data);
  if (m.method === 'Page.loadEventFired') carregamentos++;
  if (m.id && pendentes.has(m.id)) pendentes.get(m.id)(m);
};
const cmd = (method, params = {}) =>
  new Promise((res) => {
    const i = ++id;
    pendentes.set(i, res);
    ws.send(JSON.stringify({ id: i, method, params }));
  });

await cmd('Page.enable');
await dormir(4000);
const antes = carregamentos;
const estado = await cmd('Runtime.evaluate', {
  expression: "document.querySelector('my-app') ? 'montou' : 'vazio'",
  returnByValue: true,
});
console.log('estado inicial:', estado.result?.result?.value, '| carregamentos:', antes);

// Edita o componente: o servidor recompila e deve mandar `recarregar`.
const original = readFileSync(ARQUIVO, 'utf8');
writeFileSync(ARQUIVO, `// recarga ${Date.now()}\n` + original);
console.log('editado:', ARQUIVO);

const t0 = Date.now();
let recarregou = false;
for (let i = 0; i < 60; i++) {
  await dormir(500);
  if (carregamentos > antes) {
    recarregou = true;
    break;
  }
}
const ms = Date.now() - t0;
writeFileSync(ARQUIVO, original);

const depois = await cmd('Runtime.evaluate', {
  expression: "document.querySelector('pre-login-page') ? 'montou' : 'vazio'",
  returnByValue: true,
});
console.log(recarregou ? `RECARREGOU em ${ms} ms` : 'NÃO RECARREGOU');
console.log('depois da recarga:', depois.result?.result?.value);
edge.kill();
process.exit(recarregou ? 0 : 1);

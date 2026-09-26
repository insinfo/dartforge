// Juiz da conferência de tipos do emit_js (docs/INFERENCIA-JS-ALINHAMENTO.md):
// cruza as divergências que a conferência grava (DARTFORGE_JS_CONFERIR_TIPOS
// com um caminho .tsv) com a saída do oráculo do analyzer
// (tools/oraculo_tipos/oraculo.dart) e diz, por categoria, quem acerta o tipo
// estático: o emissor, a inferência comum, os dois (nós distintos do
// analyzer no mesmo intervalo) ou nenhum.
//
// Uso:
//   node scripts/conferir-tipos-js.mjs <divergencias.tsv> <oraculo.tsv> [--exemplos N]
//
// Os intervalos da conferência são em bytes UTF-8; os do analyzer, em
// unidades UTF-16: a conversão lê a fonte de cada arquivo.

import { readFileSync } from 'node:fs';

const args = process.argv.slice(2);
if (args.length < 2) {
  console.error('uso: node scripts/conferir-tipos-js.mjs <divergencias.tsv> <oraculo.tsv> [--exemplos N]');
  process.exit(64);
}
const [divPath, oracPath] = args;
const iEx = args.indexOf('--exemplos');
const exemplos = iEx >= 0 ? Number(args[iEx + 1]) : 0;

// Oráculo: `caminho \t offset \t comprimento \t nó \t tipo \t elemento`.
// Um mesmo intervalo pode ter mais de um nó (a instanciação implícita
// `FunctionReference` envolve o identificador).
const oraculo = new Map();
for (const l of readFileSync(oracPath, 'utf8').split('\n')) {
  const c = l.split('\t');
  if (c.length < 6 || c[0].startsWith('#') || c[4] === '-') continue;
  const k = `${c[0]}\t${c[1]}\t${c[2]}`;
  if (!oraculo.has(k)) oraculo.set(k, []);
  oraculo.get(k).push(c[4]);
}

const fontes = new Map();
const utf16 = (caminho, ini, comp) => {
  if (!fontes.has(caminho)) fontes.set(caminho, readFileSync(caminho));
  const b = fontes.get(caminho);
  const u = (x) => x.toString('utf8').length; // String do JS: unidades UTF-16.
  return `${caminho}\t${u(b.subarray(0, ini))}\t${u(b.subarray(ini, ini + comp))}`;
};

// O analyzer mostra `X & B` para a variável de tipo promovida; o emissor, `X`.
const norm = (t) => t.replaceAll(' ', '').replace(/(\w+)&[\w<>?,]+/g, '$1');

const porCategoria = new Map();
const total = new Map();
const amostras = new Map();
for (const l of readFileSync(divPath, 'utf8').split('\n')) {
  if (!l || l.startsWith('#')) continue;
  const c = l.split('\t');
  if (c[2] !== 'tipo') continue;
  const [local, no, , cat, texto, emissor, comum, caminho, ini, comp] = c;
  const nos = oraculo.get(utf16(caminho, Number(ini), Number(comp))) ?? [];
  let v;
  if (nos.length === 0) {
    v = 'sem oráculo';
  } else {
    const rs = new Set(nos.map(norm));
    const a = rs.has(norm(emissor));
    const b = rs.has(norm(comum));
    v = a && b ? 'ambos (nós distintos)' : a ? 'emissor certo' : b ? 'comum certo' : 'nenhum';
  }
  total.set(v, (total.get(v) ?? 0) + 1);
  if (!porCategoria.has(cat)) porCategoria.set(cat, new Map());
  const m = porCategoria.get(cat);
  m.set(v, (m.get(v) ?? 0) + 1);
  const chave = `${cat} / ${v}`;
  if (!amostras.has(chave)) amostras.set(chave, []);
  amostras.get(chave).push(`${local} ${no} \`${texto}\`: emissor=${emissor} comum=${comum} analyzer=${nos.join(' | ') || '?'}`);
}

console.log('veredito:', Object.fromEntries(total));
const cats = [...porCategoria].sort((x, y) => sum(y[1]) - sum(x[1]));
for (const [cat, m] of cats) console.log(`${String(sum(m)).padStart(5)} ${cat}: ${JSON.stringify(Object.fromEntries(m))}`);
if (exemplos > 0) {
  for (const [k, xs] of [...amostras].sort()) {
    console.log(`\n== ${k} (${xs.length})`);
    for (const x of xs.slice(0, exemplos)) console.log(`   ${x}`);
  }
}

function sum(m) {
  let s = 0;
  for (const n of m.values()) s += n;
  return s;
}

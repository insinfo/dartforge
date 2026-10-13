// Comparison between languages (RELATORIO-COMPARATIVO-NATIVO.md): the same
// five kernels in each one; `scripts/compare-languages.py` compiles and measures.
class No {
  esq: No | null; dir: No | null;
  constructor(e: No | null, d: No | null) { this.esq = e; this.dir = d; }
  conta(): number { return 1 + (this.esq ? this.esq.conta() : 0) + (this.dir ? this.dir.conta() : 0); }
}
function arvore(d: number): No { return d === 0 ? new No(null, null) : new No(arvore(d - 1), arvore(d - 1)); }
function arvores(maxD: number): number {
  const longa = arvore(maxD); let total = 0;
  for (let d = 4; d <= maxD; d += 2) { const it = 1 << (maxD - d + 4); for (let i = 0; i < it; i++) total += arvore(d).conta(); }
  return total + longa.conta();
}
function fib(n: number): number { return n < 2 ? n : fib(n - 1) + fib(n - 2); }
function numerico(n: number): number { let s = 0; for (let i = 0; i < n; i++) { s = (s + i * i) % 1000003; } return s; }
function textos(n: number): number {
  const m = new Map<string, number>();
  for (let i = 0; i < n; i++) { const k = "k" + (i % 5000); m.set(k, (m.get(k) ?? 0) + i); }
  let s = 0; m.forEach((v) => { s += v % 1000; }); return s;
}
interface Item { id: number; name: string; tags: string[]; active: boolean; level: number }
function json(n: number): number {
  const dados: Item[] = [];
  for (let i = 0; i < n; i++) dados.push({ id: i, name: "item" + i, tags: ["a" + (i % 7), "b" + (i % 13)], active: i % 2 === 0, level: i % 100 });
  const lidos: Item[] = JSON.parse(JSON.stringify(dados));
  let s = 0;
  for (const m of lidos) {
    let ativo = 0;
    if (m.active) ativo = 1;
    s += m.id + m.name.length + m.tags.length + ativo + m.level;
  }
  return s;
}
function medir(name: string, f: () => number) {
  let r = 0; const t: number[] = [];
  for (let i = 0; i < 4; i++) { const t0 = Date.now(); r = f(); t.push(Date.now() - t0); }
  console.log(name + ": " + t.join(" ") + " ms | " + r);
}
medir("fib", () => fib(32));
medir("trees", () => arvores(14));
medir("numeric", () => numerico(30000000));
medir("strings", () => textos(1000000));
medir("json", () => json(50000));

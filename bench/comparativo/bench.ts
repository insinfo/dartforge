// Comparativo entre linguagens (RELATORIO-COMPARATIVO-NATIVO.md): os mesmos
// quatro núcleos em cada uma; `scripts/comparar-linguagens.py` compila e mede.
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
function medir(nome: string, f: () => number) {
  let r = 0; const t: number[] = [];
  for (let i = 0; i < 4; i++) { const t0 = Date.now(); r = f(); t.push(Date.now() - t0); }
  console.log(nome + ": " + t.join(" ") + " ms | " + r);
}
medir("fib", () => fib(32));
medir("arvores", () => arvores(14));
medir("numerico", () => numerico(30000000));
medir("textos", () => textos(1000000));

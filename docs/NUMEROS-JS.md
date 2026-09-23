# Inteiros e SIMD no backend JavaScript — o contrato e o que pode vir

**Estado: registro de desenho; nada iniciado.** Material trazido pelo
proprietário em 2026-09-23 (Kotlin/JS, Scala.js, Fable, GopherJS,
AssemblyScript, WebAssembly SIMD). Complementa `docs/SIMD-NATIVO.md`.

## 1. O contrato que já vale — e por que não muda por padrão

O backend JS emite no contrato do DDC e liga contra o `dart_sdk.js` oficial
(`docs/EMISSAO-DDC.md`); o oráculo é o `dartdevc`/`dart2js`. No Dart oficial
para a web, `int` e `double` são o `number` do JavaScript: inteiros exatos só
até 2^53, operadores bit a bit em 32 bits, e diferenças documentadas em relação
à VM (as 7 divergências web × VM declaradas no ESTADO §2.1 são disso).
**Reproduzir esse comportamento é o correto**: a regra de equivalência
semântica do PLANO manda comportar igual à ferramenta oficial daquele alvo.
Trocar `int` por `BigInt` ou por duas metades de 32 bits por padrão faria o
nosso JS divergir do DDC em `identical`, mapas, `toString`, interop e
desempenho — não é uma otimização, é outro contrato.

## 2. O que pode vir, com o contrato preservado

* **Modo opcional de inteiros exatos de 64 bits no JS** (desligado por
  padrão; flag explícita, p.ex. `--int64-exato`), para quem precisa da
  semântica da VM na web. Duas representações a medir, não a escolher por
  regra: `BigInt` com `BigInt.asIntN(64, …)` em cada operação, ou duas metades
  de 32 bits com substituição escalar (o `RuntimeLong` do Scala.js é a
  referência de como evitar alocação por operação). Exige tratar comparações,
  operações mistas `int`/`double`, chaves de mapa, `toString`, conversões e a
  fronteira com interop — não basta trocar literais. Oráculo: a VM, não o DDC.
* **Operações com largura explícita na IR** (`AddI32Wrapping`,
  `MulU32Wrapping` → `Math.imul`, `TruncateToI16`, deslocamento lógico de 32
  bits…) quando algo precisar de largura fixa — `dart:typed_data` e o modo de
  64 bits acima. `(a * b) | 0` é **errado** para produto de 32 bits (perde bits
  antes de truncar); o certo é `Math.imul`.
* **SIMD no JS por substituição escalar** (perfil de produção): `Float32x4`
  no JS é, no `dart_sdk.js`, quatro componentes somados um a um. O que o nosso
  otimizador pode fazer, sem mudar semântica, é eliminar os objetos
  intermediários (`(a + b) * c` → quatro cadeias de componentes), **mantendo
  cada `Math.fround`** — tirar um arredondamento intermediário muda o
  resultado. É uma otimização do `emit_js_producao`, medida contra o
  `dart2js -O4`, depois do mundo fechado.
* **Wasm como alvo** (futuro): SIMD de verdade na web só existe em WebAssembly
  (`v128`, `f32x4.add`); SIMD.js foi abandonado. O Dart oficial já tem
  `dart2wasm` (WasmGC). Se um dia houver backend Wasm nosso, o laço SIMD fica
  inteiro dentro do módulo — `v128` não atravessa a fronteira JS/Wasm (a chamada
  gera `TypeError`), então nada de ir e voltar por operação.

## 3. O que NÃO fazer

* Mudar a representação de `int` no perfil padrão.
* Tratar `Float32Array`/typed arrays como SIMD — são armazenamento, não
  operação vetorial.
* Tirar `Math.fround` intermediário ou reassociar reduções de ponto flutuante.

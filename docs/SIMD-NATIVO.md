# SIMD no backend nativo — contrato de desenho

**Estado (2026-09-29): em uso.** `Float32x4`/`Int32x4`/`Float64x2` sem caixa
em locais, operadores, listas SIMD e gravação indexada (§5); a API de
`Int32x4` do Dart 3.14 por uma extensão marcada (§6); medidas contra o Dart
AOT 3.6.2 nos benchmarks do dgfx e no rasterizador (§7). Falta: parâmetros e
retornos sem caixa, e o que §7.3 lista. Contrato registrado em 2026-09-23 a
partir de material trazido pelo proprietário; o que é fato relatado de
terceiros está marcado como tal.

## 1. A lição, numa frase

No Dart oficial, o SIMD já perdeu para o código escalar **não** porque SIMD seja
inadequado ao Dart, mas porque o compilador **não manteve o caminho vetorial de
ponta a ponta**: basta uma operação do laço (um operador de máscara, um acesso à
lista) cair numa chamada genérica com encaixotamento para o custo ao redor
superar o ganho das quatro operações juntas.

## 2. Evidência (relatada; não medida por nós)

* **Issue 53662** (investigada por mraleph): no AOT, `Int32x4.operator+` e
  `Int32x4.operator&` não eram inlinados, enquanto `Float32x4` sim. Algoritmos
  em `Float32x4` usam `Int32x4` para comparações e máscaras, então o laço
  inteiro caía no caminho lento. A mesma investigação achou um defeito separado
  de **correção** em comparações com NaN (corrigido depois).
* **Commit "Support SIMD arrays in TypedDataSpecializer"** (Egorov): leitura e
  escrita de `Int32x4List`/`Float32x4List`/`Float64x2List` precisaram de
  especialização própria — operador rápido não basta se o acesso à lista não é.
* **Issue 61087 "SIMD surprises"** (2025-07-09), microbenchmark de adição:
  `Int32x4` 49 ms no JIT e **2.784 ms no AOT**, contra 140/149 ms da variante
  escalar — 18,7× mais lento no AOT. Depois disso a correção 497000 (maioria
  dos operadores `Int32x4` no AOT) foi integrada, com ganho relatado de >2× numa
  biblioteca de bits. Números anteriores à correção não representam o SDK atual.
* **Implementação da VM**: `CallSpecializer`, `GraphIntrinsifier`,
  `SimdOpInstr`; representações `kUnboxedFloat32x4`/`kUnboxedFloat64x2`/
  `kUnboxedInt32x4`; conversões explícitas de/para objeto; no ARM,
  `kFloat32x4Add` emite `vaddqs`. Ver `references/dart-sdk/runtime/vm/compiler`.
* **JS (DDC/dart2js)**: `Float32x4 + ` é implementado **componente a
  componente** no runtime JS — o nosso `emit_js` herda isso do `dart_sdk.js`,
  o que é correto pelo contrato do DDC; não há SIMD a ganhar lá sem mudar o
  runtime.
* **Wasm**: em 3.12.1 relatado como estruturas gerenciadas + escalares; a
  branch main do SDK já usa `WasmV128`/`WasmF32x4` para `+ - * /`, mas
  `min`/`max`/`clamp` continuam por componente por causa de NaN e zero com
  sinal. Não confirmado em SDK estável.

## 3. Contrato para o DartForge

1. **Representação**: `Float32x4` ↔ `<4 x float>`, `Float64x2` ↔
   `<2 x double>`, `Int32x4` ↔ `<4 x i32>` como **tipos de representação** da
   HIR, ao lado de `I64`/`F64`/`I1`/`Ref` (contrato R de
   `docs/NATIVO-PLANO.md`). Dentro de uma função o vetor é valor SSA, nunca
   objeto; vira objeto (`Box`) só na fronteira que exige referência
   (`Object`/`dynamic`/genérico, campo `Ref`, coleção não tipada), pelo mesmo
   `coerce` único do contrato R.
2. **Caminho completo, não instrução isolada**: aritmética, comparações,
   máscaras (`Int32x4` e seus operadores lógicos), `select`/`shuffle`,
   construtores, getters de componente **e** leitura/escrita em
   `Float32x4List`/`Int32x4List`/`Float64x2List` (visão sobre `ByteBuffer`,
   respeitando deslocamento e alinhamento) — todos intrinsecados. O critério de
   aceite é por **laço**, não por operação: nenhum laço do corpus SIMD pode ter
   chamada, encaixotamento ou alocação no corpo (verificado no IR emitido).
3. **Semântica antes de velocidade**: sem `fast-math`; `min`/`max`/`clamp`
   com a semântica do Dart para NaN e zero com sinal (composição de comparação
   + `select` quando a instrução nativa diverge); sem supor que duas visões do
   mesmo buffer não se sobrepõem; componentes de `Float32x4` são `float` de
   32 bits e os getters devolvem `double` (arredondamento, infinito e NaN como
   na VM). Oráculo: a VM, byte a byte, inclusive NaN.
4. **Fronteiras de chamada**: convenção interna que passe vetores sem
   encaixotar entre funções nossas (o mesmo raciocínio dos escalares
   desencaixotados); só `dynamic`/genéricos encaixotam.
5. **Autovetorização é complementar**: os passes do LLVM (loop e SLP
   vectorizer) podem ajudar laços escalares, mas não substituem o SIMD
   explícito; reduções em ponto flutuante não são reassociadas (muda o
   resultado).
6. **ARC não resolve isto**: trocar o gerenciador de memória não conserta uma
   operação que caiu em chamada genérica (`docs/EXPERIMENTO-ARC.md`).

## 4. Como provar

* **Corpus SIMD** (`corpus/js/` ou `corpus/nativo/`): soma de listas,
  produto escalar com acumulador vetorial, máscara por comparação +
  `select`, `min`/`max` com NaN e ±0, visão sobre `ByteBuffer` com
  deslocamento — saída igual à da VM (inclusive NaN).
* **Inspeção do IR**: teste que exige, nos laços do corpus SIMD, apenas
  operações sobre `<4 x float>`/`<4 x i32>`/`<2 x double>` e nenhuma chamada
  ao runtime.
* **Medição honesta** (metodologia de `docs/PESQUISA-LLVM-DART-AOT.md` §7):
  JIT e AOT separados, dados preparados fora da medição, resultado consumido,
  comparação contra a variante escalar e contra `dart compile exe`, com
  mediana de ≥ 10 execuções. Um SIMD mais lento que o escalar é defeito.

## 5. Estado medido (2026-09-26)

O primeiro degrau do contrato é o **acesso aos elementos**: sem ele, nenhum
laço sobre lista tipada escapa da chamada genérica, SIMD ou não.

* **Listas tipadas numéricas** (`Int8List` … `Float64List`), com esse tipo
  estático: `[]`, `[]=`, compostos e `length` são carga/gravação direta
  (`lower/tipados.rs`, `CargaNativa`/`GravacaoNativa` da HIR). O comprimento
  e o endereço vêm de `dartforge_typed_len`/`dartforge_typed_ptr`, funções
  puras do handle (`memory(none) speculatable`) que o LLVM tira dos laços;
  índice fora dos limites, visão não modificável e `Uint8ClampedList` caem
  no `typed_data_patch.dart`, com os erros da VM (`corpus/nativo/20`).
* **`List<E>`**: o runtime dá o cabeçalho da lista
  (`dartforge_lista_cabecalho`, `heap::CabecalhoDeLista`: endereço fixo
  enquanto a lista vive, `memory(none) speculatable`, fora dos laços), e o
  comprimento e o endereço dos elementos são lidos dele em linha; na
  escrita, um bit do cabeçalho diz que a lista é modificável e o `E` aceita
  o escalar (conferido uma vez por `dartforge_lista_len_gravavel`); o
  código gerado lê e grava o elemento em linha pela ABI de `TaggedValue`
  (`#[repr(C)]`, 16 bytes: `bits` no 0, `is_ref` no 8, a tag no 9,
  conferidos em tempo de compilação no runtime). A leitura confere a tag e
  cai na caixa se não bate; a gravação direta é só para `int`/`double`.
  Classe do usuário que implementa `List`, listas não modificáveis e
  covariância ficam com o despacho (`corpus/nativo/21`).
* **Validade do endereço**: o vetor da lista pode ser realocado (crescer,
  `length =`, `_setData`), sempre por chamada sem atributo. O endereço é
  obtido no bloco do acesso, e o LLVM só o reaproveita entre pontos sem
  chamada assim; o dono continua enraizado (o slot do valor SSA só é
  sobrescrito ao fim do quadro). `corpus/nativo/22` cobre crescimento por
  outra referência, realocação no meio de uma expressão e de um composto,
  callback que encurta a lista, coleta entre o endereço e o uso e
  gravações alternadas entre o acesso direto, o SDK e `dynamic` — iguais à
  VM também com `--optimize` e GC stress.
* **Medido e descartado**: o runtime em bitcode na LTO de produção
  (`-Clinker-plugin-lto`, mesmo LLVM do rustc) deu 41 → 38 ms em
  `List<int>[]`, com +1,5 min de build e uma terceira variante do runtime:
  o inliner não expande os helpers (personalidade de exceção, `RefCell`,
  caminhos de pânico). O que resta nos ~40 ms são as chamadas de
  comprimento e endereço dentro do laço, que o LLVM não tira porque os
  caminhos frios do mesmo laço (despacho, caixa) são chamadas que podem
  realocar a lista. O próximo passo é na HIR: provar a estabilidade da
  lista no laço (nenhuma chamada que possa realocá-la) e obter comprimento
  e endereço uma vez, com o laço original como alternativa.
* **Raízes do GC**: o quadro de cada função fica no stack dela (pilha-sombra)
  e cada raiz é um `store`. Antes eram uma chamada ao runtime por valor
  `Ref` e um vetor alocado por ativação, e essas chamadas impediam o LLVM de
  tirar qualquer coisa dos laços.

4 milhões de acessos, AOT com `--optimize`, Linux x86-64, uma execução
aquecida (`dart compile exe` 3.6.2 como referência):

| Laço | VM AOT | antes | agora |
| --- | ---: | ---: | ---: |
| `Int32List[]` | 3 ms | 1584 ms | 1 ms |
| `Float32List[]` | 3 ms | 1753 ms | 2–3 ms |
| `Uint8List[]=` | 4 ms | 2738 ms | 2–4 ms |
| `List<int>[]` | 5 ms | 1623 ms | 40–50 ms |

### Valores SIMD sem caixa

`Float32x4`, `Int32x4` e `Float64x2` com esse tipo estático (não anulável)
são vetores do LLVM na HIR (`Type::V4F32`/`V4I32`/`V2F64`,
`Instruction::Simd`): `lower/simd.rs` reconhece operadores, getters,
construtores, `shuffle`/`shuffleMix` com máscara constante (literal ou as
constantes `xyzw`), `select`, `signMask`, `withX`…, flags, as conversões de
bits e de largura, e `llvm/simd.rs` emite as instruções de vetor (`fadd <4
x float>`, `shufflevector`, `fcmp` + `select`), sem intrínseca de alvo nem
`fast-math`.

* **Locais** guardam o vetor (`repr_do_local`); o capturado por closure, o
  `late` e os de funções assíncronas continuam na caixa. A caixa só aparece
  numa fronteira (argumento, retorno, campo, `dynamic`): os 16 bytes vão ao
  runtime (`dartforge_simd_caixa`), e o desencaixe é uma carga no endereço
  de `dartforge_typed_ptr` (a caixa é imutável).
* **Listas SIMD** (`Float32x4List`, `Int32x4List`, `Float64x2List`) entram
  no caminho direto das listas tipadas: o elemento é carregado e gravado
  como vetor.
* **Comando descartado**: `e;` e as atualizações do `for` não encaixotam o
  valor final (`lower_expr_descartada`) — sem isso, `acc = acc + v` alocava
  uma caixa por volta.
* **`min`/`max`/`clamp`** por API: os de `Float32x4`/`Float64x2` seguem o
  resultado da plataforma da VM (`Utils::Minimum`/`Maximum`: `fcmp olt`/
  `ogt` + `select`, com NaN e ±0 dando o segundo operando), conferidos
  contra a VM em `corpus/nativo/24` (também com `--optimize`, GC stress e
  JIT). O `clamp` muda com a arquitetura, como no `simd128.cc`: no arm64 é
  o `vminf`/`vmaxf` da VM (com NaN fica o primeiro operando, e na
  igualdade o `min` dá `-0` e o `max` dá `+0`), então `clamp` de NaN é NaN
  no arm64 e o limite superior no x64 (`clamp_pista` no runtime, o ramo
  `aarch64` de `OpSimd::Clamp` em `llvm/simd.rs`). O `min`/`max` de `dart:math` (que propaga NaN) e o `clamp` de `num`
  (pelo `compareTo`) não passam por aqui.

| Laço (`simd/bench.dart`, 819 200 voltas) | VM AOT | caixa | sem caixa |
| --- | ---: | ---: | ---: |
| `Float32x4` `acc + a[i] * b[i] + um` | 6 ms | 414 ms | 2 ms |
| `Int32x4` `acc + (a[i] & m)` | 6 ms | 254 ms | 0–1 ms |

* **Gravação indexada sem caixa** (2026-09-29): `v4[i] = expr` numa
  `Int32x4List`/`Float32x4List`/`Float64x2List` com valor SIMD do mesmo
  tipo grava o vetor direto (`lower/atribuicao.rs`); só o caminho lento
  (índice fora, visão não modificável: o `[]=` do SDK) o põe na caixa.
  Antes, toda gravação chamava `dartforge_simd_caixa` — uma alocação por
  volta, que sozinha fazia o B2D v1 SIMD do dgfx 9× mais lento que o
  escalar. O local SIMD gravado por uma função local direta (pelo endereço
  do local de quem chama) grava 16 bytes (`llvm/mod.rs`, `Store`); antes o
  IR era recusado pelo Clang. `corpus/nativo/67`.

## 6. A API de `Int32x4` do Dart 3.14

**Fonte** (conferida em 2026-09-29): `sdk/lib/typed_data/typed_data.dart` e
`sdk/lib/_internal/vm/lib/typed_data_patch.dart` do `main` do
`dart-lang/sdk` (`tools/VERSION` 3.14.0), commits de 2026-08-18 a
2026-09-19 (`[typed_data] Add Int32x4 …`), e o SDK
`3.14.0-248.0.dev` (2026-09-19, que já tem `andNot` e `min`/`max`). Todos
`@Since("3.14")`, só em `Int32x4` (`Float32x4`/`Float64x2` não mudaram):

```dart
factory Int32x4.splat(int value);        factory Int32x4.zero();
Int32x4 operator ~();                    Int32x4 andNot(Int32x4 other);
Int32x4 operator -();                    Int32x4 abs();
Int32x4 operator <<(int shiftAmount);    Int32x4 operator >>(int shiftAmount);
Int32x4 equal(Int32x4 other);            Int32x4 notEqual(Int32x4 other);
Int32x4 lessThan(Int32x4 other);         Int32x4 lessThanOrEqual(Int32x4 other);
Int32x4 greaterThan(Int32x4 other);      Int32x4 greaterThanOrEqual(Int32x4 other);
Int32x4 min(Int32x4 other);              Int32x4 max(Int32x4 other);
bool get anyTrue;                        bool get allTrue;
```

Em revisão, não integrado: `Int32x4 operator *(Int32x4 other)` (CL 551260,
32 bits baixos do produto). **Não existe** proposta publicada de conversão
numérica `Int32x4` ↔ `Float32x4` (nem CL aberta): não há assinatura oficial
para seguir, e não foi implementada. Semântica (do patch da VM): tudo em 32
bits com volta (`-(-2^31)` e `abs(-2^31)` dão `-2^31`); deslocamento por
`shiftAmount & 31` (negativos e ≥ 32 também), `>>` aritmético; comparações
com sinal dão -1/0; `anyTrue` = alguma pista ≠ 0, `allTrue` = todas ≠ 0.

**Como fica disponível sem quebrar a paridade com o 3.6.2** (decisão):
`dart:typed_data` continua a do 3.6.2 (D1 de `docs/VERSOES-LINGUAGEM.md`). A
API vem numa **extensão comum** de Dart 3.6,
`pacotes/dartforge_simd/lib/int32x4_3_14.dart` (`extension Int32x4Api314 on
Int32x4`), com as assinaturas acima e os corpos do patch da VM, marcada com
`@pragma('dartforge:simd-api', '3.14')`:

* na VM 3.6.2 (o oráculo do corpus), os corpos rodam em Dart — é o que
  garante que o resultado do DartForge é conferido contra a semântica
  oficial, byte a byte;
* no nativo, `lower/simd.rs` (`api_314`, `receita_api_314`) reconhece a
  chamada a um membro de extensão sobre `Int32x4` cuja declaração tem o
  pragma e emite a instrução (`OpSimd::CmpInt`, `AndNot`, `Not`,
  `Desloca`, `Algum`, `Todos`, e `Mul`/`Min`/`Max`/`Neg`/`Abs` no ramo
  inteiro de `llvm/simd.rs`: `icmp` + `sext`, `shl`/`ashr` por um splat de
  `s & 31`, `mul`, `llvm.abs.v4i32` com `is_int_min_poison` falso, `icmp` +
  `bitcast` para `i4` nas reduções) — nenhuma intrínseca de alvo;
* num SDK 3.14, os membros da classe têm precedência sobre os da extensão;
  a extensão só completa o `*`.

Os construtores `Int32x4.splat`/`zero` não cabem numa extensão do 3.6:
`Int32x4(v, v, v, v)` e `Int32x4(0, 0, 0, 0)` já são intrinsecados. Um
programa que não importa a extensão não muda em nada. Conferido em
`corpus/nativo/69` (a extensão copiada; extremos, `s` negativo e ≥ 32,
laço sobre `Int32x4List`, caminhos em caixa) contra a VM 3.6.2 — e, fora
do harness, igual também no `dart run` do 3.14.0-248.0.dev.

## 7. O rasterizador do dgfx (2026-09-29)

Critério do proprietário: cada variante SIMD do rasterizador mais rápida
que a escalar compilada pelo próprio DartForge (e que o Dart AOT). Medido
numa cópia de `benchmark/rasterization_benchmark.dart` só com os pares
escalar/SIMD (B2D v1, B2D v2 Imm/Batch, SKIA; o PNG trocado por um resumo
FNV do framebuffer, igual nos três compiladores), `--sem-iso`: com
isolates o executável do DartForge sai com 127 (um `Isolate.run` com
closure que captura uma visão tipada já falha sozinho — fora do SIMD).
Windows x64, `--optimize`, 3 rodadas intercaladas, ms por iteração
(mínimo–máximo). "antes" é o binário do início do dia; "depois", o com as
correções abaixo.

| variante | Dart AOT 3.6.2 | Dart AOT 3.14.0-248.0.dev | DartForge antes | DartForge depois |
| --- | ---: | ---: | ---: | ---: |
| B2D v1 escalar | 6,68–7,09 | 6,67–7,01 | 28,55–35,72 | 5,96–6,06 |
| B2D v1 SIMD | 203,32–268,42 | 21,39–21,91 | 290,71–436,58 | 5,68–6,48 |
| B2D v2 Imm escalar | 2,43–3,99 | 2,08–2,15 | 31,63–32,44 | 1,18–1,32 |
| B2D v2 Imm SIMD | 65,46–86,08 | 3,64–3,80 | 111,91–127,12 | 2,59–2,65 |
| B2D v2 Batch escalar | 2,34–5,35 | 2,02–2,13 | 33,80–35,15 | 1,15–1,22 |
| B2D v2 Batch SIMD | 21,37–23,83 | 2,35–2,45 | 54,98–60,64 | 1,38–1,47 |
| SKIA escalar | 2,63–4,08 | 1,86–1,93 | 36,65–41,40 | 3,41–3,55 |
| SKIA SIMD | 7,90–12,90 | 1,70–1,74 | 87,31–104,35 | 12,16–12,48 |

SIMD ÷ escalar (medianas): B2D v1 0,96× (SIMD ganha), B2D v2 Imm 2,16×,
B2D v2 Batch 1,18×, SKIA 3,60×; no Dart AOT 3.6.2: 31,8×, 19,4×, 6,5×,
3,0×.

### 7.1 Gargalos achados no IR e corrigidos

1. **Gravação SIMD encaixotada** (§5): uma alocação por `v4[i] = …`;
   B2D v1 SIMD 375 → 44 ms (antes das demais).
2. **Leitura de campo `late`**: cada leitura era `dartforge_late_field_initialized`
   (busca num `HashSet` do runtime) e o `late final x = …` chamava o
   getter; agora, em campo de tipo interface não anulável guardado como
   referência, "não nulo" já prova "inicializado" (o campo novo é zero, e
   nesse tipo nunca se grava `null`), e o runtime só é consultado com o
   campo nulo (`lower/membros.rs`). Tipos anuláveis, `int`/`double`/`bool`
   e o ciclo de inicialização seguem pelo runtime (`corpus/nativo/67`).
3. **Cabeçalho de lista tipada fora dos laços**: `dartforge_typed_cabecalho`
   devolve `dereferenceable(16) ptr` (nunca nulo), e as duas cargas dele
   passam a ser especuláveis — o LLVM as tira dos laços com a chamada (antes
   só a chamada saía; as cargas ficavam, porque o laço tem saídas antes
   delas). A lista relida a cada acesso — de campo, da via rápida de
   `late`, de global — passa pelo cache de cabeçalho de `cabecalho_tipado`.
4. **`fillRange` de lista tipada**: o do SDK gravava elemento a elemento
   pelo despacho (~30 ns cada); o `clear()` do rasterizador (3 listas de
   512×512) custava 26 ms por iteração, 70 % do tempo do B2D. Agora, com o
   tipo estático de lista tipada numérica (menos `Uint8ClampedList`) e
   argumentos `int`/elemento não anuláveis, o runtime preenche de uma vez
   (`dartforge_typed_fill_int`/`_double`); faixa inválida, lista vazia e
   visão não modificável voltam ao `fillRange` do SDK.

### 7.2 Benchmarks pequenos (`dgfx/benchmark/simd`)

μs por `exercise()` (10 chamadas de `run()`, o valor do `benchmark_harness`
2.4.0, copiado sem pacote), mínimo–máximo de 3 rodadas; a rodada 1 de
coverage/span/solid teve `rustc` de outros agentes rodando. "3.6" são as
variantes que compilam no SDK 3.6.2 (escalar, extração de faixa, e as
adaptações `andNot36`/`floatBits36` com `Int32x4.bool` e `shiftSimd36`/
`floatSimd36` com deslocamento por faixa); "3.14" são as da API nova, pela
extensão (§6) no Dart 3.6.2 e no DartForge, e pelos membros da classe no
3.14.0-248.0.dev. Resumos da saída iguais em todas as colunas.

| benchmark | VM 3.6.2 | DF antes | DF depois | VM 3.6.2 (ext.) | VM 3.14-dev | DF (ext.) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| CoverageScan.scalar | 807–1544 | 3009–7308 | 2710–5877 | 899–1908 | 878–1128 | 2941–5779 |
| CoverageScan.laneExtract | 20358–35234 | 5506–5704 | 3905–6421 | 20744–22079 | 969–1364 | 3568–14906 |
| CoverageScan.scanSimd | — | — | — | 151583–162415 | 4203–6801 | 4171–4986 |
| SpanClassify.scalar | 306–407 | 6128–15392 | 1484–1943 | 304–492 | 303–358 | 1518–1625 |
| SpanClassify.laneExtract | 708–900 | 35842–38700 | 7697–22821 | 574–930 | 581–632 | 6985–8048 |
| SpanClassify.compareSimd | — | — | — | 47380–56510 | 1107–1121 | 2269–2509 |
| MaskSelect.scalar | 375–1136 | 1255–3209 | 1175–2116 | 354–1029 | 367–533 | 1198–1485 |
| MaskSelect.andNot(36) | 26038–28355 | 8865–10153 | 2559–6945 | 35012–40380 | 1116–1202 | 2002–7836 |
| MaskSelect.floatBits(36) | 15071–15603 | 9444–10766 | 2011–2230 | 15419–16821 | 614–705 | 2168–3333 |
| SolidBlend.scalar | 457–642 | 1072–1767 | 1070–3697 | 441–2039 | 435–1510 | 1065–2271 |
| SolidBlend.shiftSimd(36) | 190084–249403 | 198012–311002 | 191776–471263 | 185080–455218 | 4411–6806 | 103209–127089 |
| SolidBlend.floatSimd(36) | 101938–130814 | 176405–190736 | 128115–280625 | 101034–291535 | 7430–9228 | 87689–103194 |
| SolidBlend.mulSimd (`*`) | — | — | — | 171798–404126 | 4394–4961 | 2055–2286 |
| Int32x4Op.load+store | 64–115 | 6509–12509 | 552–990 | 65–177 | 64–104 | 356–1012 |
| Int32x4Op.add | 3137–10814 | 7057–14302 | 825–1155 | 2942–3499 | 147–559 | 778–1088 |
| Int32x4Op.andNot | — | — | — | 13867–17671 | 453–571 | 770–1091 |
| Int32x4Op.shiftLeft | — | — | — | 8582–12577 | 361–474 | 619–909 |
| Int32x4Op.abs | — | — | — | 8872–18566 | 404–466 | 647–857 |
| Int32x4Op.min | — | — | — | 21403–27618 | 478–535 | 974–1188 |
| Int32x4Op.lessThan | — | — | — | 12147–17688 | 471–702 | 814–1047 |
| Int32x4Op.equal.allTrue | — | — | — | 10517–21052 | 105–131 | 718–945 |
| Int32x4Op.withX | 2823–5480 | 7605–12291 | 632–764 | 2702–4625 | 300–334 | 661–803 |
| Int32x4Op.lanes->Float32x4 | 6365–11872 | 7782–11197 | 552–729 | 5820–7760 | 907–1018 | 360–821 |

No IR (`opt -O2` do `--emit-ir`) os laços das variantes da API nova não têm
chamada, caixa nem alocação no caminho de toda volta; os `Int32x4Op` do
DartForge ficam no custo do acesso às listas (globais), não da operação.

### 7.3 O que ainda perde, e por quê

* **SKIA SIMD 3,6× o escalar** — algoritmo do benchmark e compilador: o
  `_blitAccumulatedSIMD` cria uma `List<int>.generate` por linha (no
  DartForge ~1,2 µs cada, contra ~20 ns na VM) e extrai as pistas gravando
  o vetor num `Int32x4List` de rascunho e relendo quatro `int` pelos campos
  (`_simdLaneScratch`, `_simdLaneInts`), enquanto o escalar só lê o
  acumulador; a leitura do campo a cada acesso não sai do laço (sem TBAA,
  a gravação na lista pode, para o LLVM, mudar o campo).
* **B2D v2 Imm 2,2× e Batch 1,2×** — algoritmo: o escalar
  (`_resolveMaskedScalar`) só visita as linhas marcadas em `activeMask`; o
  SIMD varre o tile inteiro e ainda faz o blend por pixel em escalar.
* **SolidBlend.shiftSimd/floatSimd** — compilador: `mul`, `toF` e `toI` são
  funções que recebem e devolvem `Int32x4`/`Float32x4` — a caixa na
  fronteira da chamada (parâmetros e retornos SIMD sem caixa não existem);
  com o `*` da API nova (`mulSimd`) o mesmo kernel cai de ~110 000 para
  ~2 200 μs.
* **SpanClassify/CoverageScan/MaskSelect SIMD ≥ escalar no DartForge** —
  compilador: listas em globais e campos `late` relidas a cada acesso, e o
  `.length` de lista tipada relida chama `dartforge_typed_len` a cada
  volta (o LLVM especula a chamada `speculatable` do ramo da visão para
  trocar o desvio por `select`); o escalar do DartForge já é 2–5× o da VM
  nesses kernels pelo mesmo motivo.

Falta: tirar comprimento e endereço de `List<E>` dos laços com prova de
estabilidade na HIR; parâmetros e retornos SIMD sem caixa (entrada tipada
das funções); não especular `dartforge_typed_len`/`_ptr` fora do ramo da
visão; TBAA (campo × elemento de lista tipada) para tirar a releitura de
campos dos laços.

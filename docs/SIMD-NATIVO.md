# SIMD no backend nativo — contrato de desenho

**Estado: não iniciado.** Entra depois que `dart:typed_data` for compilado da
fonte pelo backend nativo (rodada 2, passo P5 em diante: o SDK da fonte,
`Smi`, strings UTF-16). Registrado em 2026-09-23 a partir de material trazido
pelo proprietário; o que é fato relatado de terceiros está marcado como tal.

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
* **`List<E>`**: o runtime dá o comprimento e o endereço dos elementos
  (`dartforge_lista_len_rapido`/`dartforge_lista_dados`, que leem só o
  cabeçalho do vetor e os conjuntos de listas especiais —
  `memory(inaccessiblemem: read)`, sem tocar o contador do `RefCell`); o
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

Falta: tirar comprimento e endereço de `List<E>` dos laços com prova de
estabilidade na HIR; a análise de vivacidade das raízes; os valores `Float32x4`/`Int32x4`/`Float64x2` como vetores
sem caixa na HIR (item 1 do contrato), com a leitura das listas SIMD pelo
mesmo caminho direto; e `min`/`max` com NaN e zero com sinal decididos contra
a VM (issue dart-lang/sdk#63962).

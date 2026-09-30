# Tamanho do executável e desempenho contra o Dart AOT

Plano consolidado de duas pesquisas de 2026-09-29 sobre `E:\references` (dart-sdk, julia,
swift, dartino-llvm, dart-sdk-llvm-mraleph, linzj-llvm-project, mun). Números medidos no
Windows 11 (i3-1215U, máquina ruidosa, ±15–30%); o que é estimativa está marcado.

## 1. Tamanho

### Medida (bytes)

| Programa | DartForge `aot --optimize` | Dart 3.6.2 `compile exe` | Snapshot AOT do Dart |
|---|---:|---:|---:|
| hello (`print`) | 12 211 200 | 5 797 376 | 798 816 |
| `bench/http/servidor.dart` | 13 676 544 | 6 108 160 | 1 093 464 |

O `compile exe` do Dart é o `dartaotruntime.exe` (5 015 040) mais o snapshot.

No hello, o `.text` tem 9,5 MB:

- **Código Dart: 8,2 MB (86%)**, quase todo do SDK. O programa do usuário ocupa menos de 80 KB.
- Rust: 1,17 MB. C (ring, zlib): 0,14 MB.

Por forma de entrada, o código Dart se divide em:

| Forma | MB |
|---|---:|
| corpos | 2,80 |
| `$c` | 2,02 |
| `$tearm` | 1,45 |
| `$g` | 1,04 |
| `$tc` | 0,45 |

No `.rdata`:

- **`@df.area`: 0,97 MB.** São os hashes de slot, que só servem à recarga.
- **Tabelas `df.mt.*`:** 0,47 MB.
- **`.xdata`:** 0,44 MB.

**Causa raiz:** o `/OPT:REF` não poda o SDK.

- A entrada chama os `df.registrar.<lib>` das 15 bibliotecas do SDK.
- Toda alocação referencia a `df.mt.<Classe>$d`, que lista todo membro da classe em todas as
  formas de entrada. Então uma classe instanciada mantém vivos todos os seus métodos.
- Um analisador de alcance sobre o IR confirmou isso: o modelo atual alcança 8 207 168 dos
  8 207 376 bytes. Mantendo viva só a entrada cujo seletor aparece em algum `df.seletor` vivo, o
  código Dart cai **80% no hello (1,6 MB) e 72% no servidor**. É estimativa: o executável podado
  não foi gerado.

### Ganhos rápidos (medidos por religação)

| Mudança | Onde | hello | servidor |
|---|---|---:|---:|
| Runtime com `panic=abort` + `opt-level="s"` | `emit_native/build.rs` (`precompilar_runtime`) | −648 KB | −679 KB |
| `@df.area` reduzido a `[chave, n]` fora da recarga | `llvm/mod.rs` (`emitir_descritor_da_area`), `gc_raizes.rs` | −968 KB | −968 KB |
| `/opt:safeicf` (o `==` de tear-off é preservado) | `ligador_windows.rs`; `--icf=safe` no Linux | −224 KB | −258 KB |
| Atributo `optsize` no SDK de produção | `sdk_modulo.rs` | −307 KB | −376 KB |
| **Tudo junto** | | **10,0 MB (−18%)** | **11,3 MB (−17%)** |

Cuidados:

- **`panic=abort`:** um pânico numa thread de isolado passa a abortar o processo. Conferir
  `isolados.rs`.
- **`opt-level="s"` e `optsize`:** passam pelos benchmarks antes de entrar.
- **Descartado:** o `/opt:icf` completo quebra a identidade de tear-off.

### Estrutural: tabelas de métodos montadas na ligação

É o equivalente, no DartForge, destas peças das referências:

- `table_selector_assigner.dart` e `precompiler.cc` (`DropFunctions`) do Dart;
- a eliminação de funções virtuais do Swift (`GenMeta.cpp`, `DeadFunctionElimination.cpp`);
- o `--trim` da Julia.

Como funciona:

- O cache de bitcode do SDK continua. Os módulos do SDK passam a só declarar as `df.mt.*$d` e os
  registros dos CIDs do runtime.
- Cada biblioteca grava um resumo com funções, referências e seletores.
- Na ligação, um ponto fixo por seletor (RTA) gera as tabelas no módulo do programa só com as
  entradas vivas.
- A LTO e o `/OPT:REF` removem o resto.

Esforço e retorno:

- **Onde mexer:** `llvm/seletores.rs`, `lower/sdk_fonte.rs`, `df.registrar.programa` em
  `llvm/mod.rs` e os resumos em `sdk_modulo.rs`.
- **Risco:** chamadas por nome em tempo de execução (`Function.apply`, `noSuchMethod`,
  `hash_do_nome` em `seletores.rs`/`closures.rs`, mirrors). Pedem raízes conservadoras, como as
  do `crates/mundo`.
- **Esforço:** 1–2 semanas.
- **Projeção (estimativa):** hello de 3,3–3,8 MB e servidor de 4,5–5 MB, abaixo do Dart AOT. A
  religação de produção também fica bem mais rápida; hoje leva de 30 a 86 s.

Depois disso:

- **Formas de entrada sob demanda:** `$tearm` só se o getter do método for usado, e `$c`/`$tc`
  só para seletores chamados de forma dinâmica.
- **Deduplicação de stubs idênticos:** o equivalente do `DedupInstructions` do Dart.

## 2. Desempenho

### Medida

| Carga | DartForge | Dart AOT |
|---|---:|---:|
| Servidor HTTP, CPU/req, 1 conexão | 180 µs | 102 µs |
| JSON (`bench/desempenho/json.dart`) | 1,3–7× o Dart AOT | 1× |

No servidor, cerca de 50 µs por requisição são E/S que os dois pagam. Detalhes nas seções
§9.10 e §10 de `NATIVO-PLANO.md`.

Perfil do JSON (tempo *self*):

| Categoria | % |
|---|---:|
| código Dart | 20 |
| consulta ao slot | 14 |
| coleta (21 inclusivo) | 14 |
| classe do valor (`value_class`, `is_subclass`) | 11,5 |
| `malloc` | 11 |
| alocação no heap | 8 |
| listas do runtime | 6 |
| RTI | 4,6 |

### Causa raiz

Os objetos das classes do programa já seguem o desenho da VM do Dart:

- cabeçalho com o `cid`;
- alocação em linha por *bump pointer* (`assembler_x64.cc` `TryAllocateObject`);
- barreira de escrita em linha;
- raízes por vivacidade, no modelo da Julia.

Mas `String`, `List`, `Map`, `Set`, closures, ambientes, células, caixas de `double`/`int` e
listas tipadas moram em `Heap::slots: Vec<Option<Value>>`, com o conteúdo no `malloc`. Cada
acesso faz TLS, `RefCell`, índice e `match`. Cada objeto novo é um `malloc`, cada morto é um
`drop` com `free`. A coleta menor custa pelo que foi alocado, não pelo que sobreviveu. Juntos,
esses custos somam cerca de 55–60% do JSON e 17–20% da CPU do servidor.

### Propostas, por ordem

**Rápidas** (dias a 2 semanas cada, independentes; ganhos estimados):

1. **Classe por slot num vetor denso.** Os bits de fixa/imutável ficam no próprio vetor, e somem
   os `HashSet` `fixas`/`imutaveis`. O `df.classe` lê a classe em linha, e o `is C` sem
   argumentos de tipo vira consulta a uma tabela de bits `@df.sub.<C>[cid]`.
   - Referência: `LoadClassId` em `assembler_x64.cc`, faixas de cid em `class_finalizer.cc` e os
     type testing stubs.
   - Ganho: JSON −8–11%, HTTP −2–3%.
2. **Coleta menor sem recalcular `estimated_bytes` nem filtrar as tabelas laterais a cada
   coleta,** e revisão de `CONTAGEM_JOVEM`. Ganho: JSON −5–10%.
3. **RTI com cache de 2 vias,** caminho rápido em linha para `is`/`as` genérico (comparar o
   metadado do cabeçalho) e `rti_definir` com id pré-internado. Ganho: −2–4%.
4. **Contexto do isolado sem chamada.**
   - No AOT, uma `thread_local(initialexec)` do módulo.
   - Depois, `%ctx` como parâmetro oculto, como o `THR` da VM e o `llvm-ptls.cpp` da Julia.
   - Ganho: −1–2%.
5. **TBAA e laços de listas sem guardas,** que habilitam a vetorização do código escalar. Em
   andamento (frente de vetorização). A referência é o `LoadOptimizer` em
   `redundancy_elimination.cc`.

**Médias:**

6. **Tabela de despacho global** `[cid + deslocamento do seletor]` no módulo do programa. Casa com
   a proposta estrutural de tamanho e depende da proposta 1.
   - Referência: `dispatch_table_generator.cc` e `DispatchTableCallInstr` do Dart.
7. **Análise de escape na HIR** para caixas, closures, ambientes e listas pequenas que não
   escapam.
   - Referências: `llvm-alloc-opt.cpp` da Julia e `DeadObjectElimination.cpp` do Swift.

**Estrutural** (semanas por fase): valores do runtime no espaço de objetos, como os
`UntaggedOneByteString`, `UntaggedArray` e `UntaggedGrowableObjectArray` de
`dart-sdk/runtime/vm/raw_object.h`.

- **Fase B, strings:** `_OneByteString`/`_TwoByteString` com `length`/`hash` no corpo e unidades
  em linha. Os `int.toString` e `_JsonStringParser` da VM passam a servir como estão.
- **Fase C:** caixas de `double`/`Mint`, células, ambientes e closures, pela alocação em linha que
  já existe.
- **Fase D:** listas expansíveis com vetor de base no espaço e elementos de 8 B, mais mapas e
  conjuntos.
- **Ganho estimado:** JSON 2–3× mais rápido e HTTP −15–20% de CPU/req. A coleta que custa pelo
  alocado se resolve junto; a coleta continua sem mover objetos, como na Julia.

### O que não fazer, com evidência

- **Trocar a pilha-sombra por *statepoints*:**
  - o fork do linzj gastou cerca de 32 dos 122 commits corrigindo *stack maps*;
  - o mraleph registra que o *statepoint* derrama tudo no chamador;
  - o Dartino registra perda de otimização com os `gc.relocate`;
  - com um coletor que não move, o ganho seria só as gravações de slot.
- **LTO do runtime Rust com o programa:** medida em `SIMD-NATIVO.md` §7.2, 41 → 38 ms por
  +1,5 min de build. Os *helpers* quentes continuam como IR `alwaysinline` (`df.*`).
- **O modelo de alocação do Swift:** ARC com `malloc` não é modelo para um runtime com coletor.

## 3. Ordem proposta

1. **Ganhos rápidos de tamanho:** `panic=abort`, `@df.area` enxuto e `safeicf`. `opt-level="s"`
   e `optsize` só depois de medir.
2. **Propostas 1 a 4 de desempenho,** baratas e independentes.
3. **Tabelas de métodos na ligação** (tamanho), junto da tabela de despacho global (proposta 6).
4. **Fases B → C → D** dos valores no espaço de objetos.

As ferramentas de medida (`alcance.py`, `quebra.py`, religação por variantes, amostrador com
pilhas) ficaram no scratchpad da sessão de 2026-09-29. Vale trazer para `tools/` as que forem
reusadas.

## 4. O que aproveitar do scriptc (Vercel Labs)

Conferido em `E:\references\scriptc` (revisão `0d9d946`), que compila TypeScript para C/LLVM com
runtime nativo próprio em C, contagem de referências e ilha QuickJS opcional (`--dynamic`). O
que já está neste plano não se repete aqui. O que ele acrescenta:

1. **Teste de ausência de código** (`tests/harness/deadstrip.test.ts`). Confere que corpo, wrapper
   assíncrono, instância genérica e entrada de vtable de algo inalcançável **não aparecem** no
   artefato, e que os mesmos construtos, quando alcançados, dão os mesmos diagnósticos.
   - Adaptar para o DartForge junto com as tabelas montadas na ligação. O caso é uma classe
     **instanciada** com métodos nunca chamados: o executável não pode conter nem o corpo nem os
     `$c`/`$tc`/`$g`/`$tearm` deles (conferido pelo mapa do ligador ou por `llvm-nm`), e o programa
     roda igual à VM.
   - Sem esse caso, o teste só provaria a eliminação de classes inteiras, que já existe.
2. **Marcador de ABI na ligação** (`packages/compiler/src/backend/runtime-abi.ts`). O objeto do
   programa deixa `scr_runtime_abi_v4` indefinido, e só o runtime compatível o define: a mistura
   falha na ligação, não em tempo de execução.
   - Aqui os hashes do cache já protegem o caminho normal. O marcador (ex.:
     `dartforge_runtime_abi_v<N>`) cobre ligações manuais e distribuições misturadas. Vale
     sobretudo **antes** das fases B/C/D, que mudam layouts.
   - Custo: horas.
3. **Plano do runtime por capacidades** (`packages/runtime-pack-common/runtime-pack-matrix.mjs`:
   unidades opcionais com predicados, e zlib, mbedTLS e QuickJS só quando a capacidade é
   efetiva).
   - Aqui o runtime é uma staticlib única por fragmentos (`crates/runtime/build.rs`), e o
     `/OPT:REF`/`--gc-sections` já descarta o código Rust não referenciado.
   - A pesquisa de tamanho mostrou que rustls, ring e zlib entram no hello só porque o código de
     `dart:io` do SDK está vivo pelas tabelas de métodos. Eles saem sozinhos com as tabelas
     montadas na ligação.
   - Então a seleção explícita por capacidade tem **prioridade baixa** como ganho de tamanho. Vale
     como relatório verificável: quais capacidades o executável liga e por quê.
4. **Relatório de "por que isto ficou no executável".** O `crates/mundo` já registra a causa de
   alcance (`Causa`: raiz, função, variável, classe). Estender isso ao SDK nativo e às tabelas
   responde a perguntas como "quem puxou este método?" e "por que TLS entrou no hello?".
   - Primeiro passo: trazer para `tools/` o `alcance.py` e o `quebra.py` da pesquisa.
5. **Inventário de compatibilidade ligado a evidência.** O scriptc separa suporte comprovado,
   parcial, recusa explícita e não revisado, e não trata "não está no registro" como prova de
   ausência. Aplicável por eixo (AOT, JIT, JS dev, JS produção, plataforma) sobre o diferencial
   que já existe.

**O que não copiar:**

- **A ilha QuickJS** (o equivalente seria embutir a VM).
- **Contagem de referências com coleta de ciclos no lugar do GC:** o perfil aponta representação e
  indireções, não o modelo de coleta.
- **A restrição de métodos genéricos fora da vtable:** o scriptc só especializa chamadas
  resolvidas estaticamente e recusa as demais, o que fere a compatibilidade com o Dart.
  Especialização aqui só como caminho adicional, com a entrada genérica sempre presente e um
  orçamento de versões.
- **Os arrays do runtime** (`packages/runtime/src/scr_array.c`, slots de 8 B com a espécie dos
  elementos no contêiner): a ideia de representação confirma a fase D, mas o código atende
  semântica de JS (buracos, propriedades) e usa `realloc`.

**Comparação de tamanho.** O hello de ~320 KB divulgado pelo scriptc é macOS, ligado à
libSystem, com runtime de escopo menor. Não é comparável aos números do Windows deste
documento.
